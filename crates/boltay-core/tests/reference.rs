use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{env, fs};

use boltay_core::{
    audio, EngineOptions, Gigaam, Language, Parakeet, Recognizer, Result, Transcriber, Vad,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

// Expected texts come from scripts/reference.py, models from scripts/fetch-models.sh.
// Without models the tests pass vacuously so a fresh checkout still builds green.
fn models() -> Option<PathBuf> {
    let dir = env::var_os("BOLTAY_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"));
    let complete = ["gigaam-v3-e2e-rnnt", "parakeet-v3", "silero_vad.onnx"]
        .iter()
        .all(|name| dir.join(name).exists());
    if complete {
        Some(dir)
    } else {
        eprintln!("no models in {}, skipping", dir.display());
        None
    }
}

fn gigaam(models: &Path) -> Gigaam {
    Gigaam::load(&models.join("gigaam-v3-e2e-rnnt"), EngineOptions::default()).unwrap()
}

fn parakeet(models: &Path) -> Parakeet {
    Parakeet::load(&models.join("parakeet-v3"), EngineOptions::default()).unwrap()
}

fn vad(models: &Path) -> Vad {
    Vad::load(&models.join("silero_vad.onnx")).unwrap()
}

fn load(name: &str) -> Vec<f32> {
    audio::load(&fixture(name)).unwrap()
}

// Exact match holds only on the machine and runtime the reference was taken with: the int8
// encoder flips a word or a capital letter on borderline audio between CPUs (AVX2 vs VNNI).
// Elsewhere the texts are compared after normalization with a character error budget.
const MAX_CER: f64 = 0.02;

fn strict() -> bool {
    env::var_os("BOLTAY_STRICT_REFERENCE").is_some_and(|v| v == "1")
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diag + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

fn cer(text: &str, reference: &str) -> f64 {
    let text: Vec<char> = normalize(text).chars().collect();
    let reference: Vec<char> = normalize(reference).chars().collect();
    edit_distance(&text, &reference) as f64 / reference.len().max(1) as f64
}

/// Describes the mismatch, `None` if the text is close enough to the reference.
fn compare(text: &str, reference: &str) -> Option<String> {
    let mismatch = if strict() {
        text != reference
    } else {
        cer(text, reference) > MAX_CER
    };
    mismatch.then(|| {
        format!(
            "CER {:.3}\n  got:      {text}\n  expected: {reference}",
            cer(text, reference)
        )
    })
}

#[test]
fn metric_ignores_case_and_punctuation() {
    assert_eq!(
        cer("У Лукоморья дуб зелёный.", "у лукоморья, дуб  зелёный"),
        0.0
    );
    assert_eq!(cer("кот", "кит"), 1.0 / 3.0);
    assert_eq!(cer("", "abcd"), 1.0);
    assert_eq!(edit_distance(&['a', 'b'], &[]), 2);
}

fn check_reference(engine: &dyn Transcriber, reference: &str) {
    let reference = fs::read_to_string(fixture(reference)).unwrap();
    let mut mismatches = Vec::new();
    for line in reference.lines() {
        let (name, expected) = line.split_once('\t').unwrap();
        let text = engine.transcribe(&load(name)).unwrap();
        if let Some(diff) = compare(&text, expected) {
            mismatches.push(format!("{name}: {diff}"));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn gigaam_matches_onnx_asr() {
    let Some(models) = models() else { return };
    check_reference(&gigaam(&models), "reference_gigaam.tsv");
}

#[test]
fn parakeet_matches_onnx_asr() {
    let Some(models) = models() else { return };
    check_reference(&parakeet(&models), "reference_parakeet.tsv");
}

#[test]
fn english_does_not_break_gigaam() {
    let Some(models) = models() else { return };
    let text = gigaam(&models).transcribe(&load("jfk.wav")).unwrap();
    assert!(!text.trim().is_empty());
}

#[test]
fn long_recording_matches_onnx_asr_with_vad() {
    let Some(models) = models() else { return };
    let recognizer = Recognizer::new(Box::new(gigaam(&models)), Some(vad(&models)));

    let files = [
        "gigaam_example.wav",
        "tts_ru_1.wav",
        "tts_ru_2.wav",
        "tts_ru_3.wav",
        "tts_ru_4.wav",
    ];
    let gap = vec![0.0; audio::SAMPLE_RATE as usize];
    let mut samples = Vec::new();
    for (i, name) in files.iter().enumerate() {
        if i > 0 {
            samples.extend_from_slice(&gap);
        }
        samples.extend(load(name));
    }

    let expected = fs::read_to_string(fixture("reference_long.txt")).unwrap();
    let text = recognizer.recognize(&samples).unwrap();
    if let Some(diff) = compare(&text, expected.trim_end()) {
        panic!("{diff}");
    }
}

struct Counting(Arc<AtomicUsize>);

impl Transcriber for Counting {
    fn transcribe(&self, _: &[f32]) -> Result<String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok("text".into())
    }

    fn language(&self) -> Language {
        Language::Russian
    }
}

#[test]
fn silence_never_reaches_the_engine() {
    let Some(models) = models() else { return };
    let calls = Arc::new(AtomicUsize::new(0));
    let recognizer = Recognizer::new(Box::new(Counting(calls.clone())), Some(vad(&models)));

    // Digital silence and a quiet hum, as from an idle microphone.
    let hum: Vec<f32> = (0..3 * audio::SAMPLE_RATE)
        .map(|n| 0.002 * (n as f32 * 0.02).sin())
        .collect();
    for samples in [vec![0.0; 48_000], hum, Vec::new()] {
        assert_eq!(recognizer.recognize(&samples).unwrap(), "");
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    assert_eq!(recognizer.recognize(&load("tts_ru_1.wav")).unwrap(), "text");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn vad_trims_surrounding_silence() {
    let Some(models) = models() else { return };
    let speech = load("tts_ru_3.wav");
    let mut samples = vec![0.0; 2 * audio::SAMPLE_RATE as usize];
    samples.extend_from_slice(&speech);
    samples.extend(vec![0.0; 3 * audio::SAMPLE_RATE as usize]);

    let segments = vad(&models).segments(&samples).unwrap();
    let first = segments.first().unwrap();
    let last = segments.last().unwrap();
    let speech_range = 32_000..32_000 + speech.len();
    assert!(first.start >= speech_range.start - 1_000, "{segments:?}");
    assert!(last.end <= speech_range.end + 1_000, "{segments:?}");
}

#[test]
fn tiny_inputs_give_empty_or_short_text() {
    let Some(models) = models() else { return };
    let speech = load("tts_ru_1.wav");
    let engines: [Box<dyn Transcriber>; 2] =
        [Box::new(gigaam(&models)), Box::new(parakeet(&models))];
    for engine in &engines {
        for len in [0, 1, 159, 160, 319, 320, 321, 480, 1_000, 2_000] {
            let text = engine.transcribe(&speech[..len]).unwrap();
            assert!(text.chars().count() < 10, "{len}: {text}");
        }
    }
}

#[test]
fn engine_is_shared_between_threads() {
    let Some(models) = models() else { return };
    let engine = Arc::new(gigaam(&models));
    let samples = Arc::new(load("tts_ru_2.wav"));
    let expected = engine.transcribe(&samples).unwrap();

    let handles: Vec<_> = (0..3)
        .map(|_| {
            let (engine, samples) = (engine.clone(), samples.clone());
            std::thread::spawn(move || engine.transcribe(&samples).unwrap())
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), expected);
    }
}
