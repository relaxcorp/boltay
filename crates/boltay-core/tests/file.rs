use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{env, fs};

use boltay_core::{
    audio, EngineOptions, Error, FileJob, FileLanguage, Language, Result, Store, Transcriber,
    Transcript,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

// Without models the tests pass vacuously so a fresh checkout still builds green.
fn store() -> Option<Store> {
    let dir = env::var_os("BOLTAY_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"));
    let complete = ["gigaam-v3-e2e-rnnt", "parakeet-v3", "silero_vad.onnx"]
        .iter()
        .all(|name| dir.join(name).exists());
    if complete {
        Some(Store::new(dir))
    } else {
        eprintln!("no models in {}, skipping", dir.display());
        None
    }
}

struct Run {
    transcript: Result<Transcript>,
    /// Engines in the order they were asked for.
    asked: Vec<Language>,
    progress: Vec<f32>,
}

fn run(store: &Store, file: &str, language: FileLanguage, cancel: bool) -> Run {
    let vad = store.vad().unwrap();
    let mut asked = Vec::new();
    let mut engine = |language: Language| -> Result<Arc<dyn Transcriber>> {
        asked.push(language);
        Ok(Arc::from(store.engine(language, EngineOptions::default())?))
    };
    let mut progress = Vec::new();
    let transcript = boltay_core::transcribe_file(
        &fixture(file),
        language,
        FileJob {
            vad: Some(&vad),
            engine: &mut engine,
            paragraph: None,
            progress: &mut |p| progress.push(p),
            cancel: &AtomicBool::new(cancel),
        },
    );
    Run {
        transcript,
        asked,
        progress,
    }
}

#[test]
fn russian_voice_message_goes_to_gigaam() {
    let Some(store) = store() else { return };
    let run = run(&store, "voice_ru_1.ogg", FileLanguage::Auto, false);
    let transcript = run.transcript.unwrap();
    assert_eq!(transcript.language, Language::Russian);
    assert_eq!(run.asked, [Language::Other, Language::Russian]);
    assert!(
        transcript.text.contains("ужин без меня"),
        "{}",
        transcript.text
    );
    assert_eq!(run.progress.last(), Some(&1.0));
    assert!(
        run.progress.windows(2).all(|w| w[0] <= w[1]),
        "{:?}",
        run.progress
    );
}

#[test]
fn english_stays_with_parakeet() {
    let Some(store) = store() else { return };
    let run = run(&store, "jfk.opus", FileLanguage::Auto, false);
    let transcript = run.transcript.unwrap();
    assert_eq!(transcript.language, Language::Other);
    assert_eq!(run.asked, [Language::Other, Language::Other]);
    assert!(
        transcript
            .text
            .contains("ask not what your country can do for you"),
        "{}",
        transcript.text
    );
}

#[test]
fn opus_gives_the_same_text_as_wav() {
    let Some(store) = store() else { return };
    let opus = run(&store, "voice_ru_1.ogg", FileLanguage::Russian, false);
    let wav = run(&store, "tts_ru_1.wav", FileLanguage::Russian, false);
    assert_eq!(opus.asked, [Language::Russian]);
    assert_eq!(opus.transcript.unwrap().text, wav.transcript.unwrap().text);
}

#[test]
fn segments_cover_the_speech() {
    let Some(store) = store() else { return };
    let transcript = run(&store, "jfk.wav", FileLanguage::English, false)
        .transcript
        .unwrap();
    let samples = audio::load(&fixture("jfk.wav")).unwrap().len();
    assert!(!transcript.segments.is_empty());
    for s in &transcript.segments {
        assert!(
            s.start < s.end && s.end <= samples,
            "{}..{}",
            s.start,
            s.end
        );
    }
}

#[test]
fn cancelled_before_the_first_piece() {
    let Some(store) = store() else { return };
    let run = run(&store, "jfk.wav", FileLanguage::English, true);
    assert!(matches!(run.transcript, Err(Error::Cancelled)));
}

#[test]
fn silence_gives_nothing() {
    let Some(store) = store() else { return };
    let dir = env::temp_dir().join(format!("boltay-silence-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("silence.wav");
    write_silence(&path, 32_000);
    let vad = store.vad().unwrap();
    let mut engine = |language: Language| -> Result<Arc<dyn Transcriber>> {
        Ok(Arc::from(store.engine(language, EngineOptions::default())?))
    };
    let transcript = boltay_core::transcribe_file(
        &path,
        FileLanguage::Auto,
        FileJob {
            vad: Some(&vad),
            engine: &mut engine,
            paragraph: None,
            progress: &mut |_| {},
            cancel: &AtomicBool::new(false),
        },
    )
    .unwrap();
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(transcript.text, "");
    assert!(transcript.segments.is_empty());
}

/// 16 kHz mono 16-bit silence.
fn write_silence(path: &Path, samples: u32) {
    let data = samples * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    wav.resize(wav.len() + data as usize, 0);
    fs::write(path, wav).unwrap();
}
