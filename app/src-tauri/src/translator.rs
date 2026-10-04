use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use boltay_core::{translation_model, Detailed, Direction, Model, SpanKind, Store, Translator};
use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};

use crate::settings::Settings;
use crate::{engine, focus, overlay, Shared};

pub const LABEL: &str = "translator";
/// The plate is 440 wide, the rest is room for its shadow.
const WIDTH: f64 = 470.0;
const HEIGHT: f64 = 260.0;
/// Gap between the selection or the pointer and the plate.
const GAP: f64 = 14.0;
/// The hotkey fires as its key goes down; Ctrl+C sent while the user's fingers still come
/// off Shift or Alt would arrive as another shortcut.
const SETTLE: Duration = Duration::from_millis(120);
/// A hidden plate keeps its window and models this long, then lets them go: a webview
/// costs a hundred megabytes, a translation model three hundred.
const KEEP: Duration = Duration::from_secs(10 * 60);

/// What the plate was opened on. The page asks for it when it loads, then gets every
/// next one as an event.
#[derive(Debug, Clone, Serialize)]
pub struct Opened {
    seq: u64,
    /// The selected text, `None` when nothing was selected: the plate shows a field.
    text: Option<String>,
}

static OPENED: Mutex<Option<Opened>> = Mutex::new(None);
static SEQ: AtomicU64 = AtomicU64::new(0);
/// The window the selection came from: "Insert" goes back there.
static TARGET: Mutex<Option<focus::Target>> = Mutex::new(None);
/// The text cursor or pointer the plate opened at, to keep it there as it grows.
static ANCHOR: Mutex<Option<(f64, f64, f64)>> = Mutex::new(None);
static ESCAPE: AtomicBool = AtomicBool::new(false);
/// The user dragged the plate away: it stays there while its height follows the text.
static MOVED: AtomicBool = AtomicBool::new(false);
static HIDDEN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Loaded models, at most one per direction.
static CACHE: Mutex<Vec<(&'static str, Arc<Translator>)>> = Mutex::new(Vec::new());

/// The translator hotkey: opens the plate on the selection, or closes an open one.
pub fn toggle(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        if visible(&app) {
            hide(&app);
        } else {
            let target = focus::current();
            let text = target.and_then(|_| selection(&app));
            show(&app, target, text);
        }
    });
}

/// From the tray: no selection to take, a field to type into.
pub fn open_empty(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || show(&app, None, None));
}

/// Esc closes the plate while it is up, wherever the focus is.
pub fn escape(app: &AppHandle) -> bool {
    if !visible(app) {
        return false;
    }
    // Called from the hotkey handler, which holds the lock that unregistering Esc in
    // `hide` waits for: on this thread the whole app would hang.
    let app = app.clone();
    thread::spawn(move || hide(&app));
    true
}

pub fn opened() -> Option<Opened> {
    OPENED.lock().unwrap().clone()
}

fn selection(app: &AppHandle) -> Option<String> {
    thread::sleep(SETTLE);
    let shared = app.state::<Shared>();
    let hotkey = shared.settings().translator_hotkey;
    match shared.paster.copy_selection(&hotkey) {
        Ok(text) => text.map(|t| t.trim().to_string()),
        Err(e) => {
            log::warn!("cannot copy the selection: {e:#}");
            None
        }
    }
}

fn visible(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

fn show(app: &AppHandle, target: Option<focus::Target>, text: Option<String>) {
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => match create(app) {
            Ok(window) => window,
            Err(e) => {
                log::error!("translator window: {e:#}");
                return;
            }
        },
    };
    let typing = text.is_none();
    *TARGET.lock().unwrap() = target.filter(|t| !t.is_ours());
    *ANCHOR.lock().unwrap() = overlay::caret_or_pointer(&window);
    *HIDDEN_AT.lock().unwrap() = None;
    MOVED.store(false, Ordering::Relaxed);
    let opened = Opened {
        seq: SEQ.fetch_add(1, Ordering::Relaxed) + 1,
        text,
    };
    *OPENED.lock().unwrap() = Some(opened.clone());
    let _ = window.emit_to(LABEL, "translator-open", &opened);
    place(&window, HEIGHT);
    // Over the app the text came from, without taking its focus: the user may only read.
    // With nothing selected there is a field to type into, and it needs the keyboard.
    if typing {
        keyboard(&window, true);
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Clicks on the plate leave the focus, and so the selection, in the app.
        keyboard(&window, false);
        overlay::show_inactive(&window);
        // A plate created just now may have taken the focus with its webview.
        if let Some(target) = *TARGET.lock().unwrap() {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(150));
                focus::reclaim(target);
            });
        }
    }
    grab_escape(app, true);
    watch_focus(app, opened.seq, typing);
}

/// The plate goes once the user moves on to another window. A click on the plate does not
/// count, nor, while it only shows a translation, the app the text came from.
fn watch_focus(app: &AppHandle, seq: u64, typing: bool) {
    let app = app.clone();
    let target = *TARGET.lock().unwrap();
    thread::spawn(move || {
        // The field takes the focus a moment after the plate shows up.
        let mut had_focus = !typing;
        loop {
            thread::sleep(Duration::from_millis(250));
            if SEQ.load(Ordering::Relaxed) != seq || !visible(&app) {
                return;
            }
            let Some(now) = focus::current() else {
                continue;
            };
            if now.is_ours() {
                had_focus = true;
            } else if had_focus && (typing || Some(now) != target) {
                hide(&app);
                return;
            }
        }
    });
}

/// "Reply in English" needs a field to type into.
pub fn take_keyboard(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        keyboard(&window, true);
        let _ = window.set_focus();
    }
}

/// Whether the plate can take the focus. Without it, buttons still take clicks and the app
/// below keeps its selection for "Replace". Windows only: X11 window managers focus on
/// click regardless.
fn keyboard(window: &tauri::WebviewWindow, on: bool) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
        };
        let Ok(hwnd) = window.hwnd() else { return };
        let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let style = if on {
                style & !(WS_EX_NOACTIVATE as isize)
            } else {
                style | WS_EX_NOACTIVATE as isize
            };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        }
    }
    #[cfg(not(windows))]
    let _ = (window, on);
}

pub fn hide(app: &AppHandle) {
    grab_escape(app, false);
    if let Some(window) = app.get_webview_window(LABEL) {
        // Both: the plate shows through Tauri with a field to type in and around it
        // otherwise, and each hide only knows its own show.
        let _ = window.hide();
        overlay::hide_inactive(&window);
        let _ = window.emit_to(LABEL, "translator-hidden", ());
    }
    #[cfg(target_os = "macos")]
    give_back_focus(app);
    *HIDDEN_AT.lock().unwrap() = Some(Instant::now());
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(KEEP);
        let idle = HIDDEN_AT
            .lock()
            .unwrap()
            .is_some_and(|at| at.elapsed() >= KEEP);
        if idle && !visible(&app) {
            CACHE.lock().unwrap().clear();
            if let Some(window) = app.get_webview_window(LABEL) {
                let _ = window.destroy();
            }
            log::info!(
                "translator unloaded after {} min hidden",
                KEEP.as_secs() / 60
            );
        }
    });
}

/// An accessory app stays active once its last window is gone, and the keys the user types
/// next would go nowhere: back to the app the text came from, or to whatever is below.
#[cfg(target_os = "macos")]
fn give_back_focus(app: &AppHandle) {
    let settings_open = app
        .get_webview_window(crate::SETTINGS_WINDOW)
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    if settings_open || !focus::current().is_some_and(|t| t.is_ours()) {
        return;
    }
    let target = *TARGET.lock().unwrap();
    let app = app.clone();
    thread::spawn(move || {
        if !target.is_some_and(focus::restore) {
            let _ = app.hide();
        }
    });
}

fn create(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    let window =
        tauri::WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("translator.html".into()))
            .title("Boltay")
            .inner_size(WIDTH, HEIGHT)
            // GTK keeps a window that may not be resized at its first size, and the plate
            // grows with its text. Without decorations there is nothing to drag anyway.
            .resizable(cfg!(target_os = "linux"))
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .focused(false)
            .accept_first_mouse(true)
            .visible(false)
            .build()?;
    overlay::everywhere(&window);
    Ok(window)
}

/// Esc reaches the plate through the system hotkey API while another app has the focus.
fn grab_escape(app: &AppHandle, grab: bool) {
    if ESCAPE.swap(grab, Ordering::Relaxed) == grab {
        return;
    }
    let esc = Shortcut::new(None, Code::Escape);
    let shortcuts = app.global_shortcut();
    // A recording may hold Esc already; then it stays registered either way.
    let _ = if grab {
        shortcuts.register(esc)
    } else {
        shortcuts.unregister(esc)
    };
}

/// Under the selection when there is room, above it otherwise, on the screen it is on.
fn place(window: &tauri::WebviewWindow, height: f64) {
    let anchor = *ANCHOR.lock().unwrap();
    let monitor = anchor
        .and_then(|(x, y, _)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let bottom = top + area.size.height as f64;
    let (x, y) = match anchor {
        Some((x, caret_top, caret_bottom)) => {
            let below = caret_bottom + GAP * scale;
            let y = if below + height * scale <= bottom {
                below
            } else {
                caret_top - (GAP + height) * scale
            };
            (x - 24.0 * scale, y)
        }
        None => (
            left + (area.size.width as f64 - WIDTH * scale) / 2.0,
            top + 80.0 * scale,
        ),
    };
    let _ = window.set_position(on_screen(&monitor, x, y, height));
}

/// The plate of `height` at `x`, `y`, moved as little as needed to fit the work area.
fn on_screen(monitor: &tauri::Monitor, x: f64, y: f64, height: f64) -> PhysicalPosition<i32> {
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (left, top) = (area.position.x as f64, area.position.y as f64);
    let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);
    let x = x.clamp(left, (right - WIDTH * scale).max(left));
    let y = y.clamp(top, (bottom - height * scale).max(top));
    PhysicalPosition::new(x.round() as i32, y.round() as i32)
}

/// The page tells how tall the plate turned out.
pub fn fit(app: &AppHandle, height: f64) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let height = height.clamp(120.0, 720.0);
        let _ = window.set_size(LogicalSize::new(WIDTH, height));
        if !MOVED.load(Ordering::Relaxed) {
            place(&window, height);
            return;
        }
        // Where the user left it, pulled up if the taller plate would run off the screen.
        if let (Ok(at), Ok(Some(monitor))) = (window.outer_position(), window.current_monitor()) {
            let fitted = on_screen(&monitor, at.x as f64, at.y as f64, height);
            if fitted != at {
                let _ = window.set_position(fitted);
            }
        }
    }
}

pub fn moved() {
    MOVED.store(true, Ordering::Relaxed);
}

/// Replaces the selection in the app it came from, or types the reply where the cursor is.
/// The app usually still has the focus; after a reply was typed on the plate it gets it back.
pub fn insert(app: &AppHandle, text: &str) -> Result<()> {
    hide(app);
    let target = *TARGET.lock().unwrap();
    if let Some(target) = target {
        if !focus::restore(target) {
            anyhow::bail!("the window the text came from is gone");
        }
    }
    let shared = app.state::<Shared>();
    let settings = shared.settings();
    shared
        .paster
        .paste(text, settings.paste, "", settings.restore_clipboard, false)
}

/// The model to translate with in one direction. With quality translation on, the big
/// one is used once it is on disk; until then it downloads in the background and the
/// base one stands in if it is there.
pub enum Choice {
    Use(&'static Model),
    /// Nothing to translate with yet; `wanted` is what to download.
    Missing {
        wanted: &'static Model,
    },
}

pub fn choose(app: &AppHandle, settings: &Settings, direction: Direction) -> Choice {
    let shared = app.state::<Shared>();
    let root = shared.models_dir(settings);
    let ready = |m: &Model| engine::model_ready(&root, m);
    let base = translation_model(direction, false);
    let big = translation_model(direction, true);
    if !settings.quality_translation {
        return if ready(base) {
            Choice::Use(base)
        } else {
            Choice::Missing { wanted: base }
        };
    }
    if ready(big) {
        return Choice::Use(big);
    }
    if let Err(e) = crate::downloads::start(app, big.id) {
        log::error!("cannot start downloading {}: {e:#}", big.id);
    }
    if ready(base) {
        Choice::Use(base)
    } else {
        Choice::Missing { wanted: big }
    }
}

fn load(app: &AppHandle, model: &'static Model) -> Result<Arc<Translator>> {
    let mut cache = CACHE.lock().unwrap();
    if let Some((_, t)) = cache.iter().find(|(id, _)| *id == model.id) {
        return Ok(t.clone());
    }
    let shared = app.state::<Shared>();
    let root = shared.models_dir(&shared.settings());
    let started = Instant::now();
    let translator = Arc::new(
        Store::new(root)
            .translator(model)
            .context("cannot load the translation model")?,
    );
    log::info!(
        "{} loaded in {} ms",
        model.id,
        started.elapsed().as_millis()
    );
    // The other model of the same direction goes: both would not fit in memory.
    cache.retain(|(_, t)| t.direction() != translator.direction());
    cache.push((model.id, translator.clone()));
    Ok(translator)
}

/// A run of text with how it is marked.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Piece {
    text: String,
    mark: Option<SpanKind>,
    note: Option<String>,
}

/// A spelled-out abbreviation or slang word, as a chip under the translation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chip {
    term: String,
    meaning: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Outcome {
    Done {
        direction: Direction,
        translation: Vec<Piece>,
        original: Vec<Piece>,
        chips: Vec<Chip>,
        /// The big model downloading while the base one translates.
        upgrading: Option<&'static str>,
        /// Only the start of a longer text was translated.
        cut: bool,
    },
    /// No model for this direction: the user picks which one to download.
    NeedModel {
        direction: Direction,
        base: u64,
        big: u64,
    },
    Fetching {
        direction: Direction,
        id: &'static str,
    },
    Empty,
}

pub fn translate(app: &AppHandle, text: &str, direction: Option<Direction>) -> Result<Outcome> {
    let (text, cut) = within_limit(text);
    let Some(direction) = direction.or_else(|| Direction::detect(text)) else {
        return Ok(Outcome::Empty);
    };
    let settings = app.state::<Shared>().settings();
    let model = match choose(app, &settings, direction) {
        Choice::Use(model) => model,
        Choice::Missing { wanted } => {
            let downloading = app.state::<Shared>().downloads.active();
            return Ok(if downloading.contains(&wanted.id) {
                Outcome::Fetching {
                    direction,
                    id: wanted.id,
                }
            } else {
                Outcome::NeedModel {
                    direction,
                    base: translation_model(direction, false).size(),
                    big: translation_model(direction, true).size(),
                }
            });
        }
    };
    let big = translation_model(direction, true);
    let upgrading = (settings.quality_translation && model.id != big.id).then_some(big.id);
    let detailed = load(app, model)?.translate_detailed(text)?;
    Ok(outcome(text, detailed, upgrading, cut))
}

/// Half a minute of translating or so: a whole document selected by mistake would hold the
/// processor for an hour, and each new request would start another one alongside.
const MAX_CHARS: usize = 5_000;

/// The text up to `MAX_CHARS`, ending with the last sentence or line that fits, and whether
/// anything was left out.
fn within_limit(text: &str) -> (&str, bool) {
    let Some((end, _)) = text.char_indices().nth(MAX_CHARS) else {
        return (text, false);
    };
    let head = &text[..end];
    let sentence = head
        .char_indices()
        .rev()
        .find(|(_, c)| matches!(c, '.' | '!' | '?' | '…' | '\n'))
        .map(|(i, c)| i + c.len_utf8())
        .filter(|&i| i > end / 2);
    let cut = sentence
        .or_else(|| head.rfind(char::is_whitespace))
        .unwrap_or(end);
    (head[..cut].trim_end(), true)
}

fn outcome(
    original: &str,
    detailed: Detailed,
    upgrading: Option<&'static str>,
    cut: bool,
) -> Outcome {
    let mut on_translation = Vec::new();
    let mut on_original = Vec::new();
    let mut chips: Vec<Chip> = Vec::new();
    for span in &detailed.spans {
        if let Some(range) = &span.translation {
            on_translation.push((range.clone(), span.kind, span.note.clone()));
        }
        if let Some(range) = &span.original {
            on_original.push((range.clone(), span.kind, span.note.clone()));
            if let Some(note) = &span.note {
                let chip = Chip {
                    term: original[range.clone()].to_string(),
                    meaning: note.clone(),
                };
                if !chips
                    .iter()
                    .any(|c| c.term.eq_ignore_ascii_case(&chip.term))
                {
                    chips.push(chip);
                }
            }
        }
    }
    Outcome::Done {
        direction: detailed.direction,
        translation: pieces(&detailed.text, on_translation),
        original: pieces(original, on_original),
        chips,
        upgrading,
        cut,
    }
}

/// Cuts a text into marked and plain runs. A mark that overlaps an earlier one is dropped.
fn pieces(text: &str, mut marks: Vec<(Range<usize>, SpanKind, Option<String>)>) -> Vec<Piece> {
    marks.sort_by_key(|(r, ..)| r.start);
    let mut out = Vec::new();
    let mut at = 0;
    for (range, kind, note) in marks {
        if range.start < at || range.end > text.len() || range.is_empty() {
            continue;
        }
        if range.start > at {
            out.push(Piece {
                text: text[at..range.start].to_string(),
                mark: None,
                note: None,
            });
        }
        out.push(Piece {
            text: text[range.clone()].to_string(),
            mark: Some(kind),
            note,
        });
        at = range.end;
    }
    if at < text.len() {
        out.push(Piece {
            text: text[at..].to_string(),
            mark: None,
            note: None,
        });
    }
    out
}

/// The plate's model buttons: the base model, or the big one, which also turns quality
/// translation on.
pub fn fetch(app: &AppHandle, direction: Direction, quality: bool) -> Result<()> {
    if quality {
        let shared = app.state::<Shared>();
        let mut settings = shared.settings();
        if !settings.quality_translation {
            settings.quality_translation = true;
            crate::commands::apply(app, settings)?;
        }
    }
    let model = translation_model(direction, quality);
    crate::downloads::start(app, model.id)?;
    let _ = app.emit_to(LABEL, "translator-fetching", model.id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(text: &str, mark: Option<SpanKind>) -> Piece {
        Piece {
            text: text.into(),
            mark,
            note: None,
        }
    }

    #[test]
    fn long_texts_end_on_a_sentence() {
        let short = "Привет. Как дела?";
        assert_eq!(within_limit(short), (short, false));
        let sentence = "Встреча переносится на четверг. ";
        let long = sentence.repeat(MAX_CHARS / sentence.chars().count() + 5);
        let (head, cut) = within_limit(&long);
        assert!(cut);
        assert!(head.ends_with("четверг."), "{}", &head[head.len() - 20..]);
        assert!(head.chars().count() <= MAX_CHARS);
        // No sentence end anywhere: the last whole word.
        let words = "слово ".repeat(MAX_CHARS);
        let (head, cut) = within_limit(&words);
        assert!(cut && head.ends_with("слово"));
    }

    #[test]
    fn pieces_cover_the_text() {
        let text = "Не знаю, почему бревна";
        let start = text.find("бревна").unwrap();
        let marks = vec![(start..text.len(), SpanKind::LowConfidence, None)];
        assert_eq!(
            pieces(text, marks),
            [
                piece("Не знаю, почему ", None),
                piece("бревна", Some(SpanKind::LowConfidence)),
            ]
        );
        assert_eq!(pieces("abc", Vec::new()), [piece("abc", None)]);
        assert!(pieces("", Vec::new()).is_empty());
    }

    #[test]
    fn overlapping_and_broken_marks_are_dropped() {
        let marks = vec![
            (0..4, SpanKind::Rare, None),
            (2..6, SpanKind::Slang, None),
            (8..40, SpanKind::Slang, None),
        ];
        assert_eq!(
            pieces("abcdefgh", marks),
            [piece("abcd", Some(SpanKind::Rare)), piece("efgh", None)]
        );
    }
}
