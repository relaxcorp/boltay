use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use boltay_core::{Dictation, FileJob, FileLanguage, Language, Transcriber};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::Speech;
use crate::Shared;

/// What a file is accepted as: whatever `boltay_core::audio` decodes.
pub const EXTENSIONS: &[&str] = &["ogg", "oga", "opus", "wav", "mp3", "flac", "m4a", "aac"];

/// The file being transcribed or the last one, as the Transcription section shows it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Job {
    pub file: Option<String>,
    pub running: bool,
    /// From 0 to 1.
    pub progress: f32,
    pub result: Option<Transcribed>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcribed {
    pub text: String,
    /// `ru` or `en`.
    pub language: &'static str,
    pub seconds: f32,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);
static CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

pub fn job() -> Job {
    JOB.lock().unwrap().clone().unwrap_or_default()
}

pub fn cancel() {
    if let Some(flag) = CANCEL.lock().unwrap().as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Lets the user pick a file, then transcribes it. `false` if they closed the dialog.
pub fn pick(app: &AppHandle) -> Result<bool> {
    use tauri_plugin_dialog::DialogExt;
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Audio", EXTENSIONS)
        .blocking_pick_file()
    else {
        return Ok(false);
    };
    start(app, path.into_path()?)?;
    Ok(true)
}

/// Starts transcribing a file in the background, unless one is already in progress.
pub fn start(app: &AppHandle, path: PathBuf) -> Result<()> {
    let known = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
    if !known {
        bail!("not an audio file: {}", path.display());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut job = JOB.lock().unwrap();
        if job.as_ref().is_some_and(|j| j.running) {
            bail!("another file is being transcribed");
        }
        *job = Some(Job {
            file: path.file_name().map(|n| n.to_string_lossy().into_owned()),
            running: true,
            ..Job::default()
        });
        *CANCEL.lock().unwrap() = Some(cancel.clone());
    }
    changed(app);
    let app = app.clone();
    thread::Builder::new()
        .name("transcribe".into())
        .spawn(move || {
            let result = run(&app, &path, &cancel);
            {
                let mut job = JOB.lock().unwrap();
                let job = job.get_or_insert_with(Job::default);
                job.running = false;
                match result {
                    Ok(done) => job.result = Some(done),
                    Err(_) if cancel.load(Ordering::Relaxed) => {}
                    Err(e) => {
                        log::error!("transcription failed: {e:#}");
                        job.error = Some(format!("{e:#}"));
                    }
                }
            }
            changed(&app);
            // The dictation model may have been swapped for the other language.
            crate::preload(&app);
        })?;
    Ok(())
}

fn changed(app: &AppHandle) {
    let _ = app.emit_to(crate::SETTINGS_WINDOW, "transcribe-changed", job());
}

fn run(app: &AppHandle, path: &Path, cancel: &AtomicBool) -> Result<Transcribed> {
    let shared = app.state::<Shared>();
    let settings = shared.settings();
    let root = shared.models_dir(&settings);
    let store = boltay_core::Store::new(&root);
    let vad = store.vad()?;
    let started = Instant::now();
    // The engines the dictation keeps loaded, so a file does not cost a second copy.
    let mut engine = |language: Language| -> boltay_core::Result<Arc<dyn Transcriber>> {
        let speech = match language {
            Language::Russian => Speech::Russian,
            Language::Other => Speech::Other,
        };
        let dictation = shared
            .engines
            .get(speech, &root, settings.graph_cache)
            .map_err(|e| boltay_core::Error::Model(format!("{e:#}")))?;
        Ok(Arc::new(Loaded(dictation, language)))
    };
    let mut last = Instant::now();
    let mut progress = |share: f32| {
        if let Some(job) = JOB.lock().unwrap().as_mut() {
            job.progress = share;
        }
        if last.elapsed() >= Duration::from_millis(200) || share >= 1.0 {
            last = Instant::now();
            let _ = app.emit_to(crate::SETTINGS_WINDOW, "transcribe-progress", share);
        }
    };
    let transcript = boltay_core::transcribe_file(
        path,
        FileLanguage::Auto,
        FileJob {
            vad: Some(&vad),
            engine: &mut engine,
            paragraph: settings.paragraph_pause(),
            progress: &mut progress,
            cancel,
        },
    )?;
    let russian = transcript.language == Language::Russian;
    // Someone else's voice message: spoken commands are words they said, not commands, and
    // a snippet trigger is not theirs to fire.
    let mut config = settings.text.clone();
    config.commands.enabled = false;
    config.snippets.enabled = false;
    if !russian {
        config.brands.enabled = false;
        config.ambiguous.enabled = false;
    }
    config.cleanup.trailing_space = false;
    let text = boltay_text::process(&config, &transcript.text);
    let rate = boltay_core::audio::SAMPLE_RATE as f32;
    let seconds = transcript
        .segments
        .last()
        .map_or(0.0, |s| s.end as f32 / rate);
    log::info!(
        "transcribed {:.0} s of audio in {} ms",
        seconds,
        started.elapsed().as_millis()
    );
    Ok(Transcribed {
        text,
        language: if russian { "ru" } else { "en" },
        seconds,
    })
}

/// A loaded dictation engine, shared with the dictation.
struct Loaded(Arc<RwLock<Dictation>>, Language);

impl Transcriber for Loaded {
    fn transcribe(&self, samples: &[f32]) -> boltay_core::Result<String> {
        self.0
            .read()
            .unwrap()
            .recognizer
            .engine()
            .transcribe(samples)
    }

    fn language(&self) -> Language {
        self.1
    }
}
