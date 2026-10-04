use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::audio::{self, Capture};
use crate::history::Entry;
use crate::i18n::Text;
use crate::overlay::{self, Copied, ErrorKind, Level, State};
use crate::session::{Action, Event, Key, Session};
use crate::settings::{HotkeyMode, Settings, Speech};
use crate::sounds::{self, Cue};
use crate::{engine, focus};
use crate::{hotkey, Shared};

const TICK: Duration = Duration::from_millis(50);
const IDLE_CHECK: Duration = Duration::from_secs(30);
const OVERLAY_KEEP: Duration = Duration::from_secs(60);
/// A single-key hotkey may turn out to be half of right Ctrl+C. The recording starts on the
/// press, but the sound and the overlay wait this long, so a shortcut passes unnoticed.
/// Typing a capital, right Shift is often held for more than 200 ms before the letter.
const REVEAL_DELAY: Duration = Duration::from_millis(350);

pub enum Msg {
    Event(Event),
    /// Settings were saved: pick up the new hotkey mode.
    Reload,
}

/// Runs the recording on its own thread, one event at a time, so presses and releases
/// that come in quick succession are handled in order and never block the UI thread.
pub fn spawn(app: AppHandle) -> Sender<Msg> {
    let (tx, rx) = mpsc::channel();
    let own = tx.clone();
    thread::Builder::new()
        .name("dictation".into())
        .spawn(move || Controller::new(app, own).run(rx))
        .expect("cannot start the dictation thread");
    tx
}

enum Failure {
    NoTranslationModel,
    Microphone(String),
}

struct Controller {
    app: AppHandle,
    tx: Sender<Msg>,
    session: Session,
    capture: Option<Capture>,
    /// The recording in progress came from the translation hotkey.
    translate: bool,
    /// Where the recording in progress is to be pasted.
    target: Option<focus::Target>,
    started: Instant,
    /// When a quiet start shows itself, unless it is dropped before that.
    reveal_at: Option<Instant>,
    /// The recording that could not start, waiting to be told about.
    failure: Option<Failure>,
    last_tick: Instant,
    last_idle_check: Instant,
}

impl Controller {
    fn new(app: AppHandle, tx: Sender<Msg>) -> Self {
        let mode = app_state(&app).settings().mode;
        Self {
            app,
            tx,
            session: Session::new(mode),
            capture: None,
            translate: false,
            target: None,
            started: Instant::now(),
            reveal_at: None,
            failure: None,
            last_tick: Instant::now(),
            last_idle_check: Instant::now(),
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        self.reload_session();
        loop {
            match rx.recv_timeout(TICK) {
                Ok(Msg::Event(event)) => self.handle(event),
                Ok(Msg::Reload) => self.reload_session(),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            // Events in a steady stream must not hold the level meter and the limit back.
            if self.last_tick.elapsed() >= TICK {
                self.last_tick = Instant::now();
                self.tick();
            }
        }
    }

    fn reload_session(&mut self) {
        let settings = app_state(&self.app).settings();
        self.session.set_mode(settings.mode);
        self.session.set_latch(settings.latch);
    }

    fn handle(&mut self, event: Event) {
        let action = self.session.handle(event, Instant::now());
        match action {
            Action::Start(key) => self.start(key),
            Action::Stop => self.stop(),
            Action::Discard => self.discard(),
            Action::None => {}
        }
        if event == Event::Finished {
            // The mode may have changed while a recording was in flight.
            self.reload_session();
            crate::refresh_status(&self.app);
        }
    }

    fn start(&mut self, key: Key) {
        let translate = key == Key::Translate;
        // Before the overlay shows up: it may take focus as it is created.
        self.target = focus::current();
        let shared = app_state(&self.app);
        let settings = shared.settings();
        self.started = Instant::now();
        self.translate = translate;
        let quiet =
            settings.mode == HotkeyMode::Hold && shared.hotkeys.lock().unwrap().is_solo(key);
        let model = translation_model(&self.app, &settings);
        let root = shared.models_dir(&settings);
        if translate && !engine::model_ready(&root, model) {
            // The translation model is downloaded on first use, not with the app.
            self.failure = Some(Failure::NoTranslationModel);
        } else if let Err(reason) = crate::permissions::ready_to_record() {
            // Without the permission macOS hands over silence: this dictation is lost
            // either way, the permissions come up instead.
            crate::show_permissions(&self.app);
            self.failure = Some(Failure::Microphone(reason));
        } else {
            // A little over the limit: the dictation thread stops the recording on its own tick.
            match Capture::start(
                settings.microphone.as_deref(),
                settings.max_recording_secs + 1,
            ) {
                Ok(capture) => self.capture = Some(capture),
                Err(e) => self.failure = Some(Failure::Microphone(format!("{e:#}"))),
            }
        }
        if quiet {
            // Even a failure waits: right Shift+A must not report a broken microphone.
            self.reveal_at = Some(self.started + REVEAL_DELAY);
        } else {
            self.reveal(settings.sounds);
        }
        if self.failure.is_some() {
            return;
        }

        // Load the models while the user speaks: the first dictation after launch, or the
        // first translation, would otherwise wait for them.
        let app = self.app.clone();
        thread::spawn(move || {
            let state = app_state(&app);
            let settings = state.settings();
            let root = state.models_dir(&settings);
            if let Ok(dictation) =
                state
                    .engines
                    .get(speech(&settings, translate), &root, settings.graph_cache)
            {
                if translate {
                    let _ = state.engines.ensure_translator(
                        &dictation,
                        &root,
                        translation_model(&app, &settings),
                    );
                }
            }
        });
    }

    /// The recording makes itself known: start sound, overlay, tray. Or, if it could not
    /// start, why.
    fn reveal(&mut self, sound: bool) {
        self.reveal_at = None;
        app_state(&self.app).claim_overlay();
        if let Some(failure) = self.failure.take() {
            self.fail(failure);
            return;
        }
        hotkey::grab_escape(&self.app, true);
        let settings = app_state(&self.app).settings();
        if sound && settings.sounds {
            sounds::play(settings.sound_set, Cue::Start);
        }
        overlay::show(
            &self.app,
            State::Listening {
                translate: self.translate,
            },
        );
        self.set_tray(Text::Recording);
        if let Some(target) = self.target {
            // A fresh WebView2 can grab focus a moment after the window is shown.
            thread::spawn(move || {
                for delay in [0, 200, 600] {
                    thread::sleep(Duration::from_millis(delay));
                    focus::reclaim(target);
                }
            });
        }
    }

    fn fail(&mut self, failure: Failure) {
        // Back to idle whether the recording was still on or already stopped.
        self.session.handle(Event::Cancel, Instant::now());
        self.session.handle(Event::Finished, Instant::now());
        match failure {
            Failure::NoTranslationModel => {
                let id = translation_model(&self.app, &app_state(&self.app).settings()).id;
                overlay::show(&self.app, State::Fetching { id });
                if let Err(e) = crate::downloads::start(&self.app, id) {
                    overlay::update(
                        &self.app,
                        State::Error {
                            key: ErrorKind::Model,
                            detail: format!("{e:#}"),
                        },
                    );
                    self.hide_later(Duration::from_secs(5));
                }
            }
            Failure::Microphone(detail) => {
                log::error!("microphone: {detail}");
                overlay::show(
                    &self.app,
                    State::Error {
                        key: ErrorKind::Microphone,
                        detail,
                    },
                );
                self.hide_later(Duration::from_secs(4));
            }
        }
    }

    fn stop(&mut self) {
        if self.reveal_at.is_some() {
            // Stopped before the overlay came up: it still shows the recognition.
            self.reveal(false);
        }
        hotkey::grab_escape(&self.app, false);
        let Some(capture) = self.capture.take() else {
            return;
        };
        let duration = self.started.elapsed();
        let settings = app_state(&self.app).settings();
        if settings.sounds {
            sounds::play(settings.sound_set, Cue::Stop);
        }
        overlay::update(&self.app, State::Recognizing);
        self.set_tray(Text::Recognizing);

        let app = self.app.clone();
        let tx = self.tx.clone();
        let translate = self.translate;
        let target = self.target.take();
        thread::spawn(move || {
            let began = Instant::now();
            let state = transcribe(&app, capture, duration, translate, target);
            // Never the text itself: the log is for failures, not for what was said.
            let audio = duration.as_secs_f32();
            let took = began.elapsed().as_millis();
            match &state {
                State::Error { key, detail } => log::error!("{key:?}: {detail}"),
                State::Unsent { reason, .. } => log::warn!("not pasted: {reason}"),
                State::Empty => log::info!("nothing recognized in {audio:.1} s of audio"),
                State::Done if translate => {
                    log::info!("translated {audio:.1} s of audio in {took} ms")
                }
                State::Done => log::info!("recognized {audio:.1} s of audio in {took} ms"),
                _ => {}
            }
            overlay::update(&app, state.clone());
            let linger = match state {
                State::Unsent { .. } => Duration::from_secs(12),
                State::Error { .. } => Duration::from_secs(5),
                State::Empty => Duration::from_millis(900),
                _ => Duration::from_millis(500),
            };
            let _ = tx.send(Msg::Event(Event::Finished));
            hide_after(&app, linger);
        });
    }

    fn discard(&mut self) {
        hotkey::grab_escape(&self.app, false);
        self.capture = None;
        self.failure = None;
        // Never shown: the overlay may still hold the previous result, leave it be.
        if self.reveal_at.take().is_some() {
            return;
        }
        overlay::hide(&self.app);
        self.hide_later(Duration::ZERO);
        crate::refresh_status(&self.app);
    }

    fn tick(&mut self) {
        if self.reveal_at.is_some_and(|at| Instant::now() >= at) {
            self.reveal(true);
        }
        if let Some(capture) = self.capture.as_ref().filter(|_| self.reveal_at.is_none()) {
            let elapsed = self.started.elapsed();
            overlay::level(
                &self.app,
                Level {
                    level: capture.level(),
                    elapsed_ms: elapsed.as_millis() as u64,
                },
            );
            let limit = Duration::from_secs(u64::from(
                app_state(&self.app).settings().max_recording_secs,
            ));
            if elapsed >= limit {
                self.handle(Event::Limit);
            }
        }
        if self.last_idle_check.elapsed() >= IDLE_CHECK {
            self.last_idle_check = Instant::now();
            if let Some(minutes) = app_state(&self.app).settings().unload_after_minutes {
                app_state(&self.app)
                    .engines
                    .unload_if_idle(Duration::from_secs(u64::from(minutes) * 60));
            }
        }
    }

    fn hide_later(&self, after: Duration) {
        let app = self.app.clone();
        thread::spawn(move || hide_after(&app, after));
    }

    fn set_tray(&self, status: Text) {
        crate::set_tray_status(&self.app, status);
    }
}

/// Hides the overlay unless a new recording has taken it over in the meantime, and frees
/// it a minute later: dictations in a row reuse it, an idle app does not pay for it.
pub fn hide_after(app: &AppHandle, after: Duration) {
    let shared = app_state(app);
    let generation = shared.overlay_generation();
    thread::sleep(after);
    if shared.overlay_generation() != generation {
        return;
    }
    overlay::hide(app);
    thread::sleep(OVERLAY_KEEP);
    if shared.overlay_generation() == generation {
        overlay::retire(app);
    }
}

/// Recognizes, runs the text pipeline, saves the result, then pastes it. The order matters:
/// the text is on disk before anything that can fail or hang touches the other app.
fn transcribe(
    app: &AppHandle,
    capture: Capture,
    duration: Duration,
    translate: bool,
    target: Option<focus::Target>,
) -> State {
    let shared = app_state(app);
    let settings = shared.settings();
    let mut samples = match capture.finish() {
        Ok(samples) => samples,
        Err(e) => {
            return State::Error {
                key: ErrorKind::Microphone,
                detail: format!("{e:#}"),
            }
        }
    };
    let root = shared.models_dir(&settings);
    let ready = shared
        .engines
        .is_loaded(speech(&settings, translate), &root)
        && (!translate
            || shared
                .engines
                .has_translator(translation_model(app, &settings)));
    if !ready {
        overlay::update(app, State::Loading);
    }
    let model_error = |e: anyhow::Error| {
        crate::set_tray_status(app, Text::NoModel);
        State::Error {
            key: ErrorKind::Model,
            detail: format!("{e:#}"),
        }
    };
    let dictation =
        match shared
            .engines
            .get(speech(&settings, translate), &root, settings.graph_cache)
        {
            Ok(d) => d,
            Err(e) => return model_error(e),
        };
    if translate {
        if let Err(e) =
            shared
                .engines
                .ensure_translator(&dictation, &root, translation_model(app, &settings))
        {
            return model_error(e);
        }
    }
    {
        let mut dictation = dictation.write().unwrap();
        dictation.text = settings.text.clone();
        dictation.text.snippets.variables = variables(&shared, &settings);
        dictation.paragraph_pause = settings.paragraph_pause();
    }
    let dictation = dictation.read().unwrap();
    audio::amplify(&mut samples, settings.mic_gain_db);
    if let Some(vad) = dictation.recognizer.vad() {
        vad.set_threshold(settings.vad_threshold);
    }
    let recognition_error = |e: boltay_core::Error| State::Error {
        key: ErrorKind::Recognition,
        detail: e.to_string(),
    };
    let raw = match dictation.recognize(&samples) {
        Ok(raw) => raw,
        Err(e) => return recognition_error(e),
    };
    let text = match dictation.finish(&raw, translate) {
        Ok(text) => text,
        Err(e) => return recognition_error(e),
    };
    drop(dictation);
    // A dictated "абзац" on its own is nothing but line breaks, and still worth pasting.
    if text.trim_matches(' ').is_empty() {
        return State::Empty;
    }

    shared.remember(&text);
    if settings.history.enabled {
        let entry = Entry {
            at: now_ms(),
            text: text.clone(),
            raw,
            duration_ms: duration.as_millis() as u64,
        };
        if let Err(e) = shared.history.append(&entry) {
            log::error!("history: {e:#}");
        }
        crate::history_changed(app);
    }

    if let Err(reason) = crate::permissions::ready_to_paste() {
        if let Err(e) = shared.paster.copy(&text) {
            log::error!("{e:#}");
        }
        // The user is about to press Cmd+V in their app: an open settings window turns to
        // the permissions, a closed one shows them next time, nothing takes the focus.
        let _ = app.emit_to(crate::SETTINGS_WINDOW, "permissions", ());
        return State::Unsent {
            text,
            reason,
            copied: Some(Copied::NoAccess),
        };
    }
    // Closed while the user was speaking: the text must not land wherever focus went.
    if target.is_some_and(|t| !focus::restore(t)) {
        return State::Unsent {
            text,
            reason: "the window the dictation was for is gone or does not take focus".into(),
            copied: None,
        };
    }
    // Keys sent to an elevated window are dropped without a word, so the paste would
    // look fine and do nothing. Ctrl+V pressed by the user does reach it.
    if target.is_some_and(focus::is_above_us) {
        if let Err(e) = shared.paster.copy(&text) {
            log::error!("{e:#}");
        }
        return State::Unsent {
            text,
            reason: "the window runs as administrator".into(),
            copied: Some(Copied::Elevated),
        };
    }
    match shared.paster.paste(
        &text,
        settings.paste,
        &settings.hotkey,
        settings.restore_clipboard,
        settings.paragraph_messages,
    ) {
        Ok(()) => State::Done,
        Err(e) => State::Unsent {
            text,
            reason: format!("{e:#}"),
            copied: None,
        },
    }
}

/// What the placeholders in snippets stand for right now.
fn variables(shared: &Shared, settings: &Settings) -> boltay_text::Variables {
    let now = chrono::Local::now();
    let snippets = &settings.text.snippets;
    let wants_clipboard = snippets.enabled
        && snippets
            .entries
            .iter()
            .any(|s| boltay_text::uses_clipboard(&s.text));
    boltay_text::Variables {
        date: now.format("%d.%m.%Y").to_string(),
        time: now.format("%H:%M").to_string(),
        clipboard: wants_clipboard
            .then(|| shared.paster.read())
            .flatten()
            .unwrap_or_default(),
    }
}

fn app_state(app: &AppHandle) -> tauri::State<'_, Shared> {
    app.state::<Shared>()
}

/// Russian into English: the big model once it is on disk, the base one meanwhile.
fn translation_model(app: &AppHandle, settings: &Settings) -> &'static boltay_core::Model {
    match crate::translator::choose(app, settings, boltay_core::Direction::RuEn) {
        crate::translator::Choice::Use(model)
        | crate::translator::Choice::Missing { wanted: model } => model,
    }
}

/// Translation is from Russian: its hotkey always takes GigaAM, whatever language is picked
/// for plain dictation.
fn speech(settings: &Settings, translate: bool) -> Speech {
    if translate {
        Speech::Russian
    } else {
        settings.speech
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
