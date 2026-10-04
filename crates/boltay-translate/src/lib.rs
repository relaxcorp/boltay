mod english;
mod latin;
mod prepare;
mod spm;
mod vocab;

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue, SessionOutputs};
use ort::value::{Tensor, TensorRef};
use serde::{Deserialize, Serialize};

pub use crate::prepare::prepare;
use crate::prepare::prepare_detailed;
use crate::vocab::Vocab;

const GENERATION: &str = "generation_config.json";
/// Web export: quantized encoder and a merged decoder that switches on `use_cache_branch`.
const MERGED: [&str; 2] = [
    "encoder_model_quantized.onnx",
    "decoder_model_merged_quantized.onnx",
];
/// Optimum export: a decoder for the first step and another one taking past keys and values.
const SPLIT: [&str; 3] = [
    "encoder_model.onnx",
    "decoder_model.onnx",
    "decoder_with_past_model.onnx",
];

/// Greedy decoding of this model sometimes loops on a phrase ("I don't know, I don't know")
/// on colloquial input. A banned repeated trigram alone gets dodged with a look-alike token
/// ("don ́t"), a mild penalty on tokens already used stops the loop. Stronger penalties or a
/// bigram ban start changing ordinary translations.
const NO_REPEAT_NGRAM: usize = 3;
const REPETITION_PENALTY: f32 = 1.05;

/// A translated word whose least likely piece the decoder picked with a lower probability
/// than this is marked as possibly wrong.
const LOW_CONFIDENCE: f32 = 0.15;
/// A source word the tokenizer cuts into this many pieces or more is one the model has
/// rarely seen whole. Russian is cut finer: its endings are pieces of their own.
const RARE_PIECES_EN: usize = 3;
const RARE_PIECES_RU: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    RuEn,
    EnRu,
}

impl Direction {
    /// Cyrillic text goes into English, Latin into Russian, by which letters there are more
    /// of. `None` for text without letters of either.
    pub fn detect(text: &str) -> Option<Self> {
        let (mut cyrillic, mut latin) = (0usize, 0usize);
        for c in text.chars() {
            if c.is_ascii_alphabetic() {
                latin += 1;
            } else if matches!(c, '\u{400}'..='\u{4ff}') {
                cyrillic += 1;
            }
        }
        match (cyrillic, latin) {
            (0, 0) => None,
            (c, l) if c >= l => Some(Self::RuEn),
            _ => Some(Self::EnRu),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    /// The decoder was unsure about this word of the translation.
    LowConfidence,
    /// A word of the original the model has rarely seen whole.
    Rare,
    /// An abbreviation spelled out before translation.
    Abbreviation,
    /// A slang word replaced with a plain one before translation.
    Slang,
}

/// A marked place, in bytes of the translation, of the original, or of both.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Span {
    pub kind: SpanKind,
    pub translation: Option<Range<usize>>,
    pub original: Option<Range<usize>>,
    /// What a spelled-out abbreviation or slang word means, in the reader's language.
    pub note: Option<String>,
}

/// Result of [`Translator::translate_detailed`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Detailed {
    pub text: String,
    pub direction: Direction,
    /// Ordered by kind of mark, then by position.
    pub spans: Vec<Span>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{}: {source}", path.display())]
    Config {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("tokenizer: {0}")]
    Tokenizer(String),
    #[error(transparent)]
    Onnx(#[from] ort::Error),
}

impl From<ort::Error<ort::session::builder::SessionBuilder>> for Error {
    fn from(e: ort::Error<ort::session::builder::SessionBuilder>) -> Self {
        Self::Onnx(e.into())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Deserialize)]
struct Generation {
    decoder_start_token_id: i64,
    eos_token_id: i64,
    max_length: usize,
    #[serde(default)]
    bad_words_ids: Vec<Vec<i64>>,
}

/// Opus-MT Marian model. Greedy decoding, one sentence at a time.
pub struct Translator {
    direction: Direction,
    /// Target language token a multilingual model needs in front of the source.
    target: Option<i64>,
    vocab: Vocab,
    generation: Generation,
    sessions: Mutex<Sessions>,
}

struct Sessions {
    encoder: Session,
    decoder: Decoder,
}

enum Decoder {
    Merged(Session),
    Split { first: Session, with_past: Session },
}

impl Translator {
    /// Loads a model directory in either export layout.
    pub fn load(dir: &Path, direction: Direction) -> Result<Self> {
        let path = dir.join(GENERATION);
        let json = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let generation =
            serde_json::from_str(&json).map_err(|source| Error::Config { path, source })?;
        // The web export keeps the shared embedding in int8 behind a DequantizeLinear, which
        // ORT leaves unfolded while QDQ handling is on and then redoes on every step: 22 of
        // 27 ms per token. Folding it at load takes about 250 MB more memory.
        let sessions = if dir.join(MERGED[1]).exists() {
            Sessions {
                encoder: session(&dir.join(MERGED[0]), false)?,
                decoder: Decoder::Merged(session(&dir.join(MERGED[1]), true)?),
            }
        } else {
            Sessions {
                encoder: session(&dir.join(SPLIT[0]), false)?,
                decoder: Decoder::Split {
                    first: session(&dir.join(SPLIT[1]), false)?,
                    with_past: session(&dir.join(SPLIT[2]), false)?,
                },
            }
        };
        let vocab = Vocab::load(dir)?;
        let target = match direction {
            Direction::EnRu => vocab.token(">>rus<<"),
            Direction::RuEn => None,
        };
        Ok(Self {
            direction,
            target,
            vocab,
            generation,
            sessions: Mutex::new(sessions),
        })
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Translates sentence by sentence, keeping line breaks.
    pub fn translate(&self, text: &str) -> Result<String> {
        let mut lines = Vec::new();
        for line in text.split('\n') {
            let mut out = Vec::new();
            for sentence in sentences(line) {
                let (ids, _) = self.encode(sentence)?;
                let decoded = self.vocab.decode(&self.tokens(&ids)?.0)?;
                out.push(self.finish(sentence, decoded));
            }
            lines.push(out.join(" "));
        }
        Ok(lines.join("\n"))
    }

    /// Spells out chat abbreviations and slang, translates, and marks the words the model
    /// was unsure about, the rare words of the original and what was spelled out.
    pub fn translate_detailed(&self, text: &str) -> Result<Detailed> {
        let prepared = prepare_detailed(text, self.direction);
        let mut out = String::new();
        let mut doubts = Vec::new();
        let mut rare = Vec::new();
        let mut line_start = 0;
        for (i, line) in prepared.text.split('\n').enumerate() {
            if i > 0 {
                out.push('\n');
            }
            for (j, sentence) in sentences(line).into_iter().enumerate() {
                if j > 0 {
                    out.push(' ');
                }
                let at = line_start + (sentence.as_ptr() as usize - line.as_ptr() as usize);
                let (ids, words) = self.encode(sentence)?;
                for word in words.into_iter().filter(|w| self.is_rare(sentence, w)) {
                    let range = at + word.range.start..at + word.range.end;
                    if let (Some(start), Some(end)) =
                        (prepared.original(range.start), prepared.original(range.end))
                    {
                        rare.push(start..end);
                    }
                }
                let (tokens, probs) = self.tokens(&ids)?;
                let translated = self.finish(sentence, self.vocab.decode(&tokens)?);
                for (range, p) in self.word_confidence(&translated, &tokens, &probs) {
                    if p < LOW_CONFIDENCE {
                        doubts.push(out.len() + range.start..out.len() + range.end);
                    }
                }
                out.push_str(&translated);
            }
            line_start += line.len() + 1;
        }

        let mut spans: Vec<Span> = doubts
            .into_iter()
            .map(|r| Span {
                kind: SpanKind::LowConfidence,
                translation: Some(r),
                original: None,
                note: None,
            })
            .collect();
        spans.extend(rare.into_iter().map(|r| Span {
            kind: SpanKind::Rare,
            translation: None,
            original: Some(r),
            note: None,
        }));
        spans.extend(prepared.terms.into_iter().map(|t| Span {
            kind: t.kind,
            translation: None,
            original: Some(t.original),
            note: Some(t.meaning),
        }));
        Ok(Detailed {
            text: out,
            direction: self.direction,
            spans,
        })
    }

    /// The base model writes technical names in Cyrillic, readers expect them as is.
    fn finish(&self, source: &str, translated: String) -> String {
        match self.direction {
            Direction::EnRu => latin::restore(source, &translated),
            Direction::RuEn => translated,
        }
    }

    fn encode(&self, sentence: &str) -> Result<(Vec<i64>, Vec<vocab::Word>)> {
        let (mut ids, words) = self.vocab.encode_words(sentence)?;
        if let Some(target) = self.target {
            ids.insert(0, target);
        }
        Ok((ids, words))
    }

    /// A word of letters the tokenizer had to cut into many pieces. Numbers, codes and
    /// names with digits are spelled piece by piece anyway and say nothing; a capitalized
    /// word counts only if its lowercase form is cut too, `Sorry` opening a sentence is no
    /// rarer than `sorry`.
    fn is_rare(&self, sentence: &str, word: &vocab::Word) -> bool {
        let pieces = match self.direction {
            Direction::EnRu => RARE_PIECES_EN,
            Direction::RuEn => RARE_PIECES_RU,
        };
        let text = &sentence[word.range.clone()];
        if word.pieces < pieces
            || text.chars().count() < 4
            || !text.chars().all(|c| c.is_alphabetic() || c == '-')
        {
            return false;
        }
        if !text.starts_with(char::is_uppercase) {
            return true;
        }
        let lower = text.to_lowercase();
        self.vocab
            .encode_words(&lower)
            .is_ok_and(|(_, words)| words.iter().map(|w| w.pieces).sum::<usize>() >= pieces)
    }

    /// Words of a translated sentence with the probability of their least likely piece.
    /// Words are found in the decoded text one after another, punctuation around them left
    /// out; a word the decoder spelled differently is skipped.
    fn word_confidence(
        &self,
        text: &str,
        tokens: &[i64],
        probs: &[f32],
    ) -> Vec<(Range<usize>, f32)> {
        let mut words: Vec<(String, f32)> = Vec::new();
        for (&token, &p) in tokens.iter().zip(probs) {
            let Some(piece) = self.vocab.piece(token) else {
                continue;
            };
            let starts = piece.starts_with('\u{2581}');
            let piece = piece.trim_start_matches('\u{2581}');
            // Punctuation does not make a word doubtful. A bare word boundary does: it is
            // where the decoder picked which word comes next.
            let p = if piece.is_empty() || piece.chars().any(char::is_alphanumeric) {
                p
            } else {
                1.0
            };
            match words.last_mut() {
                Some((word, min)) if !starts => {
                    word.push_str(piece);
                    *min = min.min(p);
                }
                _ => words.push((piece.to_string(), p)),
            }
        }
        let mut out = Vec::new();
        let mut cursor = 0;
        for (word, p) in words {
            let word = word.trim_matches(|c: char| !c.is_alphanumeric());
            if word.is_empty() {
                continue;
            }
            if let Some(found) = text[cursor..].find(word) {
                let start = cursor + found;
                cursor = start + word.len();
                out.push((start..cursor, p));
            }
        }
        out
    }

    /// Output tokens of one sentence and the probability the decoder gave each.
    fn tokens(&self, ids: &[i64]) -> Result<(Vec<i64>, Vec<f32>)> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        self.generate(&mut sessions, ids)
    }

    fn generate(&self, sessions: &mut Sessions, ids: &[i64]) -> Result<(Vec<i64>, Vec<f32>)> {
        let len = ids.len();
        let mask = vec![1i64; len];
        let (width, hidden) = {
            let outputs = sessions.encoder.run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(([1, len], ids))?,
                "attention_mask" => TensorRef::from_array_view(([1, len], &mask[..]))?,
            ])?;
            let (shape, hidden) = outputs["last_hidden_state"].try_extract_tensor::<f32>()?;
            (shape[2] as usize, hidden.to_vec())
        };

        let gen = &self.generation;
        let bad_words: Vec<usize> = gen
            .bad_words_ids
            .iter()
            .filter(|w| w.len() == 1)
            .map(|w| w[0] as usize)
            .collect();
        let mut cache = match &sessions.decoder {
            Decoder::Merged(session) => KvCache::empty(session),
            Decoder::Split { .. } => KvCache::default(),
        };
        // Includes the start token, as the n-gram check in transformers does.
        let mut sequence = vec![gen.decoder_start_token_id];
        let mut probs = Vec::new();
        // The start token counts towards max_length, as in transformers.
        for step in 1..gen.max_length {
            let session = match &mut sessions.decoder {
                Decoder::Merged(session) => session,
                Decoder::Split { first, .. } if step == 1 => first,
                Decoder::Split { with_past, .. } => with_past,
            };
            let takes = |name: &str| session.inputs().iter().any(|i| i.name() == name);
            let mut inputs: Vec<(String, SessionInputValue)> = vec![
                (
                    "encoder_attention_mask".into(),
                    TensorRef::from_array_view(([1, len], &mask[..]))?.into(),
                ),
                (
                    "input_ids".into(),
                    Tensor::from_array(([1usize, 1], vec![sequence[sequence.len() - 1]]))?.into(),
                ),
            ];
            if takes("encoder_hidden_states") {
                inputs.push((
                    "encoder_hidden_states".into(),
                    TensorRef::from_array_view(([1, len, width], &hidden[..]))?.into(),
                ));
            }
            if takes("use_cache_branch") {
                inputs.push((
                    "use_cache_branch".into(),
                    Tensor::from_array(([1usize], vec![step > 1]))?.into(),
                ));
            }
            if takes("past_key_values.0.decoder.key") {
                cache.push_inputs(&mut inputs)?;
            }
            let outputs = session.run(inputs)?;

            let (_, logits) = outputs["logits"].try_extract_tensor::<f32>()?;
            let (next, p) = if step + 1 == gen.max_length {
                (gen.eos_token_id, 1.0)
            } else {
                let mut logits = logits.to_vec();
                penalize_repeats(&mut logits, &sequence[1..], REPETITION_PENALTY);
                let mut banned = bad_words.clone();
                banned.extend(repeated_ngram_ends(&sequence, NO_REPEAT_NGRAM));
                let next = best(&logits, &banned);
                (next as i64, probability(&logits, &banned, next))
            };
            cache.update(&outputs, step == 1)?;
            if next == gen.eos_token_id {
                break;
            }
            sequence.push(next);
            probs.push(p);
        }
        Ok((sequence.split_off(1), probs))
    }
}

/// Makes tokens already generated less likely, as `repetition_penalty` in transformers:
/// positive logits are divided by the penalty, negative ones multiplied.
fn penalize_repeats(logits: &mut [f32], generated: &[i64], penalty: f32) {
    let mut seen = generated.to_vec();
    seen.sort_unstable();
    seen.dedup();
    for token in seen {
        if let Some(v) = logits.get_mut(token as usize) {
            *v = if *v > 0.0 { *v / penalty } else { *v * penalty };
        }
    }
}

/// Tokens that would repeat an n-gram already in `sequence`: the ones that followed an
/// earlier occurrence of its last n-1 tokens. Same as `no_repeat_ngram_size` in transformers.
fn repeated_ngram_ends(sequence: &[i64], n: usize) -> Vec<usize> {
    if sequence.len() + 1 < n {
        return Vec::new();
    }
    let prefix = &sequence[sequence.len() + 1 - n..];
    sequence
        .windows(n)
        .filter(|w| &w[..n - 1] == prefix)
        .map(|w| w[n - 1] as usize)
        .collect()
}

/// Argmax over the vocabulary with banned tokens skipped, first maximum wins.
fn best(logits: &[f32], banned: &[usize]) -> usize {
    let mut best = None::<(usize, f32)>;
    for (i, &v) in logits.iter().enumerate() {
        if banned.contains(&i) {
            continue;
        }
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map_or(0, |(i, _)| i)
}

/// Softmax probability of `token` among the tokens that were allowed.
fn probability(logits: &[f32], banned: &[usize], token: usize) -> f32 {
    let max = logits[token];
    let sum: f32 = logits
        .iter()
        .enumerate()
        .filter(|(i, _)| !banned.contains(i))
        .map(|(_, &v)| (v - max).exp())
        .sum();
    1.0 / sum
}

/// Past keys and values, by name without the `past_key_values.` prefix. Attention over the
/// encoder output is computed on the first step only: the merged decoder returns placeholders
/// for it later on, the split one does not return it at all.
#[derive(Default)]
struct KvCache {
    entries: Vec<(String, Vec<i64>, Vec<f32>)>,
}

impl KvCache {
    /// Zero-length entries shaped after the session inputs, for a first step that still
    /// has to be fed some past.
    fn empty(session: &Session) -> Self {
        let entries = session
            .inputs()
            .iter()
            .filter_map(|input| {
                let name = input.name().strip_prefix("past_key_values.")?;
                let mut shape = input.dtype().tensor_shape()?.to_vec();
                shape[0] = 1;
                shape[2] = 0;
                Some((name.to_string(), shape, Vec::new()))
            })
            .collect();
        Self { entries }
    }

    fn push_inputs(&self, inputs: &mut Vec<(String, SessionInputValue<'_>)>) -> Result<()> {
        for (name, shape, data) in &self.entries {
            let value = Tensor::from_array((shape.clone(), data.clone()))?;
            inputs.push((format!("past_key_values.{name}"), value.into()));
        }
        Ok(())
    }

    fn update(&mut self, outputs: &SessionOutputs<'_>, first: bool) -> Result<()> {
        for (key, value) in outputs.iter() {
            let Some(name) = key.strip_prefix("present.") else {
                continue;
            };
            if name.contains("encoder") && !first {
                continue;
            }
            let (shape, data) = value.try_extract_tensor::<f32>()?;
            let entry = (name.to_string(), shape.to_vec(), data.to_vec());
            match self.entries.iter_mut().find(|e| e.0 == name) {
                Some(slot) => *slot = entry,
                None => self.entries.push(entry),
            }
        }
        Ok(())
    }
}

fn session(path: &Path, fold_weights: bool) -> Result<Session> {
    let mut builder = Session::builder()?;
    if fold_weights {
        builder = builder.with_config_entry("session.disable_quant_qdq", "1")?;
    }
    Ok(builder
        .with_optimization_level(GraphOptimizationLevel::All)?
        .with_intra_threads(num_cpus::get_physical())?
        .commit_from_file(path)?)
}

/// Splits after sentence-ending punctuation followed by whitespace. The model is trained on
/// single sentences and drops content from long multi-sentence inputs.
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '.' | '!' | '?' | '…')
            && chars.peek().is_some_and(|&(_, n)| n.is_whitespace())
        {
            let end = i + c.len_utf8();
            out.push(text[start..end].trim());
            start = end;
        }
    }
    out.push(text[start..].trim());
    out.retain(|s| !s.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        assert_eq!(
            sentences("Привет! Как дела? Всё хорошо... Пока"),
            ["Привет!", "Как дела?", "Всё хорошо...", "Пока"]
        );
        assert_eq!(sentences("Версия 2.5 вышла."), ["Версия 2.5 вышла."]);
        assert_eq!(sentences("  "), Vec::<&str>::new());
        assert_eq!(sentences("Раз.  Два."), ["Раз.", "Два."]);
    }

    #[test]
    fn repeated_trigrams_are_banned() {
        // "a b c a b" -> "c" would repeat "a b c".
        assert_eq!(repeated_ngram_ends(&[1, 2, 3, 1, 2], 3), [3]);
        assert_eq!(repeated_ngram_ends(&[1, 2, 3, 1, 2, 5, 1, 2], 3), [3, 5]);
        assert!(repeated_ngram_ends(&[1, 2, 3, 4], 3).is_empty());
        assert!(repeated_ngram_ends(&[1], 3).is_empty());
        assert!(repeated_ngram_ends(&[1, 1], 3).is_empty());
        assert_eq!(repeated_ngram_ends(&[1, 1, 1], 3), [1]);
    }

    #[test]
    fn penalty_pushes_used_tokens_down() {
        let mut logits = [2.0, -1.0, 3.0];
        // A token repeated in the output is penalized once, as in transformers.
        penalize_repeats(&mut logits, &[0, 1, 1], 2.0);
        assert_eq!(logits, [1.0, -2.0, 3.0]);
    }

    #[test]
    fn direction_follows_the_letters() {
        assert_eq!(
            Direction::detect("Привет, как дела?"),
            Some(Direction::RuEn)
        );
        assert_eq!(
            Direction::detect("Hey, how are you?"),
            Some(Direction::EnRu)
        );
        // A Russian sentence with a brand name in it is still Russian.
        assert_eq!(
            Direction::detect("Скинь ссылку на GitHub"),
            Some(Direction::RuEn)
        );
        assert_eq!(
            Direction::detect("Check the Яндекс dashboard please"),
            Some(Direction::EnRu)
        );
        assert_eq!(Direction::detect("12:30 — ok?"), Some(Direction::EnRu));
        assert_eq!(Direction::detect("12:30 — 15 000"), None);
        assert_eq!(Direction::detect(""), None);
    }

    #[test]
    fn probability_is_softmax_over_allowed_tokens() {
        let p = probability(&[1.0, 1.0, 5.0], &[2], 0);
        assert!((p - 0.5).abs() < 1e-6, "{p}");
        let p = probability(&[2.0, 0.0], &[], 0);
        assert!((p - 0.880_797).abs() < 1e-5, "{p}");
    }

    #[test]
    fn banned_tokens_are_skipped() {
        assert_eq!(best(&[0.1, 0.9, 0.5], &[1]), 2);
        assert_eq!(best(&[0.5, 0.5], &[]), 0);
    }
}
