use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::audio::{self, Capture};
use crate::Shared;

/// "Check by voice" in the text settings: one phrase from the microphone, shown as the
/// model heard it and as it would be pasted. Nothing is pasted or kept in the history.
#[derive(Default)]
pub struct VoiceCheck(Mutex<Option<Capture>>);

/// The longest phrase worth checking.
const MAX_SECS: u32 = 30;

#[derive(Debug, Clone, Serialize)]
pub struct Heard {
    pub raw: String,
    pub text: String,
}

pub fn start(app: &AppHandle) -> Result<()> {
    let shared = app.state::<Shared>();
    let settings = shared.settings();
    let capture = Capture::start(settings.microphone.as_deref(), MAX_SECS)?;
    *shared.voice_check.0.lock().unwrap() = Some(capture);
    Ok(())
}

pub fn stop(app: &AppHandle) -> Result<Heard> {
    let shared = app.state::<Shared>();
    let capture = shared
        .voice_check
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow!("nothing is being recorded"))?;
    let mut samples = capture.finish()?;
    let settings = shared.settings();
    audio::amplify(&mut samples, settings.mic_gain_db);
    let root = shared.models_dir(&settings);
    let dictation = shared
        .engines
        .get(settings.speech, &root, settings.graph_cache)?;
    {
        let mut dictation = dictation.write().unwrap();
        dictation.text = settings.text.clone();
        dictation.paragraph_pause = settings.paragraph_pause();
    }
    let dictation = dictation.read().unwrap();
    if let Some(vad) = dictation.recognizer.vad() {
        vad.set_threshold(settings.vad_threshold);
    }
    let raw = dictation.recognize(&samples)?;
    let text = dictation.finish(&raw, false)?;
    Ok(Heard { raw, text })
}

pub fn cancel(app: &AppHandle) {
    app.state::<Shared>().voice_check.0.lock().unwrap().take();
}
