use std::path::{Path, PathBuf};
use std::{env, fs};

use boltay_translate::{Direction, SpanKind, Translator};

const BASE: &str = "opus-mt-ru-en";
const BIG: &str = "opus-mt-tc-big-zle-en";
const EN_RU: &str = "opus-mt-en-ru";
const EN_RU_BIG: &str = "opus-mt-tc-big-en-zle";

// Expected translations come from scripts/translation_reference.mjs (transformers.js) for the
// base model and scripts/translation_reference.py (MarianTokenizer) for the big and the
// English-Russian ones, the big English-Russian one with `>>rus<<` in front.
// Without the model the tests pass vacuously so a fresh checkout still builds green.
fn model(name: &str) -> Option<PathBuf> {
    let dir = env::var_os("BOLTAY_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"))
        .join(name);
    if dir.join("generation_config.json").is_file() {
        Some(dir)
    } else {
        eprintln!("no model in {}, skipping", dir.display());
        None
    }
}

// Exact match holds only with the runtime the reference ran on: onnxruntime 1.30 and 1.28
// pick a different article in one phrase. By default the character error rate over the
// whole set must stay under 2%; per phrase a single word already costs several percent.
const MAX_CER: f64 = 0.02;

fn normalize(text: &str) -> Vec<char> {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect()
}

/// Edit distance and reference length, both in characters after normalization.
fn errors(text: &str, reference: &str) -> (usize, usize) {
    let (a, b) = (normalize(text), normalize(reference));
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
    (row[b.len()], b.len())
}

#[test]
fn error_rate_ignores_case_and_punctuation() {
    assert_eq!(errors("Hey! How are you?", "hey, how are you"), (0, 15));
    assert_eq!(errors("a cat", "the cat"), (3, 7));
}

fn check_reference(name: &str, direction: Direction, fixture: &str) {
    let Some(dir) = model(name) else { return };
    let strict = env::var_os("BOLTAY_STRICT_REFERENCE").is_some_and(|v| v == "1");
    let translator = Translator::load(&dir, direction).unwrap();
    let reference = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture),
    )
    .unwrap();

    let mut mismatches = Vec::new();
    let (mut edits, mut chars) = (0, 0);
    for line in reference.lines() {
        let (source, expected) = line.split_once('\t').unwrap();
        let text = translator.translate(source).unwrap();
        let (e, n) = errors(&text, expected);
        edits += e;
        chars += n;
        if text != expected {
            mismatches.push(format!(
                "{source}\n  got:      {text}\n  expected: {expected}"
            ));
        }
    }
    let cer = edits as f64 / chars.max(1) as f64;
    let failed = if strict {
        !mismatches.is_empty()
    } else {
        cer > MAX_CER
    };
    assert!(!failed, "CER {cer:.4}\n{}", mismatches.join("\n"));
}

#[test]
fn base_matches_transformers_js() {
    check_reference(BASE, Direction::RuEn, "reference.tsv");
}

#[test]
fn big_matches_marian_tokenizer() {
    check_reference(BIG, Direction::RuEn, "reference_big.tsv");
}

#[test]
fn en_ru_matches_marian_tokenizer() {
    check_reference(EN_RU, Direction::EnRu, "reference_en_ru.tsv");
}

#[test]
fn en_ru_big_matches_marian_tokenizer() {
    check_reference(EN_RU_BIG, Direction::EnRu, "reference_en_ru_big.tsv");
}

#[test]
fn does_not_loop_on_colloquial_input() {
    let Some(dir) = model(BASE) else { return };
    let text = Translator::load(&dir, Direction::RuEn)
        .unwrap()
        .translate("Я не знаю, капец какой-то, переделай этот модуль.")
        .unwrap();
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .collect();
    let trigrams: Vec<_> = words.windows(3).collect();
    for (i, t) in trigrams.iter().enumerate() {
        assert!(!trigrams[i + 1..].contains(t), "{text}");
    }
}

#[test]
fn keeps_lines_and_empty_input() {
    let Some(dir) = model(BASE) else { return };
    let translator = Translator::load(&dir, Direction::RuEn).unwrap();
    assert_eq!(translator.translate("").unwrap(), "");
    let text = translator.translate("Привет!\nПока.").unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn shared_between_threads() {
    let Some(dir) = model(BASE) else { return };
    let translator = std::sync::Arc::new(Translator::load(&dir, Direction::RuEn).unwrap());
    let expected = translator.translate("Где здесь ближайшая аптека?").unwrap();
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let t = translator.clone();
            std::thread::spawn(move || t.translate("Где здесь ближайшая аптека?").unwrap())
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), expected);
    }
}

#[test]
fn detailed_marks_point_into_the_texts() {
    let Some(dir) = model(EN_RU) else { return };
    let translator = Translator::load(&dir, Direction::EnRu).unwrap();
    let text = "Hey, send me the creds ASAP.\nIDK why the Kubernetes pod keeps crashing.";
    let result = translator.translate_detailed(text).unwrap();
    assert_eq!(result.direction, Direction::EnRu);
    assert_eq!(result.text.lines().count(), 2, "{}", result.text);
    for span in &result.spans {
        if let Some(r) = &span.translation {
            assert!(result
                .text
                .get(r.clone())
                .is_some_and(|w| !w.trim().is_empty()));
        }
        if let Some(r) = &span.original {
            assert!(text.get(r.clone()).is_some_and(|w| !w.trim().is_empty()));
        }
    }
    let marked = |kind: SpanKind| -> Vec<&str> {
        result
            .spans
            .iter()
            .filter(|s| s.kind == kind)
            .filter_map(|s| s.original.clone().map(|r| &text[r]))
            .collect()
    };
    assert_eq!(marked(SpanKind::Abbreviation), ["creds", "ASAP", "IDK"]);
    assert_eq!(marked(SpanKind::Rare), ["Kubernetes"]);
    let asap = result
        .spans
        .iter()
        .find(|s| s.original.clone().map(|r| &text[r]) == Some("ASAP"))
        .unwrap();
    assert_eq!(asap.note.as_deref(), Some("как можно скорее"));
}

#[test]
fn detailed_flags_doubtful_words() {
    let Some(dir) = model(EN_RU) else { return };
    let translator = Translator::load(&dir, Direction::EnRu).unwrap();
    // Users "coming on board" is the model's guess at onboarding, and it says it is unsure.
    let result = translator
        .translate_detailed("Please review the onboarding flow and leave comments by EOD.")
        .unwrap();
    let doubtful: Vec<&str> = result
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::LowConfidence)
        .map(|s| &result.text[s.translation.clone().unwrap()])
        .collect();
    assert!(
        doubtful.iter().any(|w| w.contains("включ")),
        "{}: {doubtful:?}",
        result.text
    );
    let plain = translator
        .translate_detailed("Thank you very much for your help.")
        .unwrap();
    assert!(plain.spans.is_empty(), "{plain:?}");
}

#[test]
fn detailed_into_english_spells_out_russian_slang() {
    let Some(dir) = model(BASE) else { return };
    let translator = Translator::load(&dir, Direction::RuEn).unwrap();
    let text = "Спс, ТЗ норм.";
    let result = translator.translate_detailed(text).unwrap();
    let terms: Vec<(&str, SpanKind)> = result
        .spans
        .iter()
        .filter(|s| s.note.is_some())
        .map(|s| (&text[s.original.clone().unwrap()], s.kind))
        .collect();
    assert_eq!(
        terms,
        [
            ("Спс", SpanKind::Abbreviation),
            ("ТЗ", SpanKind::Abbreviation),
            ("норм", SpanKind::Slang)
        ]
    );
    assert_eq!(
        result.text,
        translator
            .translate(&boltay_translate::prepare(text))
            .unwrap()
    );
}
