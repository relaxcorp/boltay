use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::Path;

use tokenizers::models::unigram::Unigram;
use tokenizers::normalizers::{Precompiled, Replace, Sequence, Strip};
use tokenizers::pre_tokenizers::metaspace::{Metaspace, PrependScheme};
use tokenizers::{Encoding, Tokenizer};

use crate::spm::SpmModel;
use crate::{Error, Result};

const TOKENIZER: &str = "tokenizer.json";
const SOURCE_SPM: &str = "source.spm";
const VOCAB: &str = "vocab.json";

/// A word of the source text without punctuation around it: its bytes and how many pieces
/// the tokenizer cut it into.
pub struct Word {
    pub range: Range<usize>,
    pub pieces: usize,
}

/// Text to token ids and back, in whichever form the model export ships it.
pub enum Vocab {
    /// A `tokenizer.json` of the web export.
    Json(Tokenizer),
    /// SentencePiece pieces of the source language mapped through the Marian vocabulary,
    /// as `MarianTokenizer` does it.
    Marian {
        source: Tokenizer,
        ids: HashMap<String, i64>,
        pieces: Vec<String>,
        unk: i64,
        eos: i64,
        special: Vec<i64>,
    },
}

impl Vocab {
    /// The SentencePiece model of the source language when the export has one, as
    /// `MarianTokenizer` reads it, otherwise `tokenizer.json`.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(SOURCE_SPM);
        if !path.exists() {
            return Ok(Self::Json(load_json(&dir.join(TOKENIZER))?));
        }
        let spm = SpmModel::parse(&read(&path)?)?;
        let source =
            unigram(&spm).map_err(|e| Error::Tokenizer(format!("{}: {e}", path.display())))?;

        let path = dir.join(VOCAB);
        let ids: HashMap<String, i64> =
            serde_json::from_slice(&read(&path)?).map_err(|source| Error::Config {
                path: path.clone(),
                source,
            })?;
        let id = |token: &str| {
            ids.get(token)
                .copied()
                .ok_or_else(|| Error::Tokenizer(format!("{}: no {token}", path.display())))
        };
        let (unk, eos, pad) = (id("<unk>")?, id("</s>")?, id("<pad>")?);
        let mut pieces = vec![String::new(); ids.values().max().map_or(0, |&m| m as usize + 1)];
        for (piece, &i) in &ids {
            pieces[i as usize] = piece.clone();
        }
        Ok(Self::Marian {
            source,
            ids,
            pieces,
            unk,
            eos,
            special: vec![unk, eos, pad],
        })
    }

    /// The id of a vocabulary entry.
    pub fn token(&self, piece: &str) -> Option<i64> {
        match self {
            Vocab::Json(tokenizer) => tokenizer.token_to_id(piece).map(i64::from),
            Vocab::Marian { ids, .. } => ids.get(piece).copied(),
        }
    }

    /// Token ids and the words they come from.
    pub fn encode_words(&self, text: &str) -> Result<(Vec<i64>, Vec<Word>)> {
        match self {
            Vocab::Json(tokenizer) => {
                let encoding = tokenizer
                    .encode(text, true)
                    .map_err(|e| Error::Tokenizer(e.to_string()))?;
                let ids = encoding.get_ids().iter().map(|&id| id.into()).collect();
                Ok((ids, words(text, &encoding)))
            }
            Vocab::Marian {
                source,
                ids,
                unk,
                eos,
                ..
            } => {
                let encoding = source
                    .encode(text, false)
                    .map_err(|e| Error::Tokenizer(e.to_string()))?;
                let mut out: Vec<i64> = encoding
                    .get_tokens()
                    .iter()
                    .map(|piece| ids.get(piece).copied().unwrap_or(*unk))
                    .collect();
                out.push(*eos);
                Ok((out, words(text, &encoding)))
            }
        }
    }

    /// The vocabulary entry of an output token, `None` for special ones.
    pub fn piece(&self, id: i64) -> Option<Cow<'_, str>> {
        let piece: Cow<str> = match self {
            Vocab::Json(tokenizer) => tokenizer.id_to_token(u32::try_from(id).ok()?)?.into(),
            Vocab::Marian {
                pieces, special, ..
            } => {
                if special.contains(&id) {
                    return None;
                }
                pieces.get(usize::try_from(id).ok()?)?.as_str().into()
            }
        };
        let special = piece.starts_with('<') && piece.ends_with('>');
        (!special).then_some(piece)
    }

    pub fn decode(&self, ids: &[i64]) -> Result<String> {
        match self {
            Vocab::Json(tokenizer) => {
                let ids: Vec<u32> = ids.iter().map(|&t| t as u32).collect();
                let text = tokenizer
                    .decode(&ids, true)
                    .map_err(|e| Error::Tokenizer(e.to_string()))?;
                Ok(clean_up_spaces(&text))
            }
            Vocab::Marian {
                pieces, special, ..
            } => {
                let text: String = ids
                    .iter()
                    .filter(|id| !special.contains(id))
                    .filter_map(|&id| pieces.get(id as usize))
                    .map(String::as_str)
                    .collect();
                Ok(text.replace('\u{2581}', " ").trim().to_string())
            }
        }
    }
}

/// Groups the pieces of an encoding back into the words they were cut from.
fn words(text: &str, encoding: &Encoding) -> Vec<Word> {
    let mut out: Vec<(u32, Word)> = Vec::new();
    for ((word, &(start, end)), token) in encoding
        .get_word_ids()
        .iter()
        .zip(encoding.get_offsets())
        .zip(encoding.get_tokens())
    {
        let Some(word) = *word else {
            continue;
        };
        // Punctuation glued to a word is not part of it.
        if !token.chars().any(char::is_alphanumeric) {
            continue;
        }
        match out.last_mut() {
            Some((last, w)) if *last == word => {
                w.range.end = w.range.end.max(end);
                w.pieces += 1;
            }
            _ => out.push((
                word,
                Word {
                    range: start..end,
                    pieces: 1,
                },
            )),
        }
    }
    // The word boundary marker takes the space before the word along.
    out.into_iter()
        .map(|(_, mut w)| {
            let word = &text[w.range.clone()];
            w.range.start += word.len() - word.trim_start().len();
            w
        })
        .collect()
}

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}

/// The tokenizer SentencePiece would run: its own normalization table, extra whitespace
/// removed, a word boundary marker in front of every word.
fn unigram(
    spm: &SpmModel,
) -> std::result::Result<Tokenizer, Box<dyn std::error::Error + Send + Sync>> {
    if !spm.unigram {
        return Err("only unigram sentencepiece models are supported".into());
    }
    let model = Unigram::from(spm.pieces.clone(), Some(spm.unk_id), false)?;
    let normalizer = Sequence::new(vec![
        Precompiled::from(&spm.charsmap)?.into(),
        Replace::new(
            tokenizers::normalizers::replace::ReplacePattern::Regex(" {2,}".into()),
            " ",
        )?
        .into(),
        Strip::new(true, true).into(),
    ]);
    let mut tokenizer = Tokenizer::new(model);
    tokenizer
        .with_normalizer(Some(normalizer))
        .with_pre_tokenizer(Some(Metaspace::new(
            '\u{2581}',
            PrependScheme::Always,
            true,
        )));
    Ok(tokenizer)
}

fn load_json(path: &Path) -> Result<Tokenizer> {
    let failed = |e: &dyn std::fmt::Display| Error::Tokenizer(format!("{}: {e}", path.display()));
    let json = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let mut config: serde_json::Value = serde_json::from_str(&json).map_err(|e| failed(&e))?;
    let normalizer = &mut config["normalizer"];
    if normalizer["type"] == "Precompiled" && normalizer["precompiled_charsmap"].is_null() {
        *normalizer = nmt_nfkc();
    }
    config.to_string().parse().map_err(|e| failed(&e))
}

/// The web export ships the SentencePiece normalizer without its character map, which the
/// tokenizers crate refuses. transformers.js falls back to this, so the same here: drop
/// control characters, turn other whitespace into spaces, then NFKC.
fn nmt_nfkc() -> serde_json::Value {
    serde_json::json!({
        "type": "Sequence",
        "normalizers": [
            {
                "type": "Replace",
                "pattern": {"Regex": "[\\x{01}-\\x{08}\\x{0B}\\x{0E}-\\x{1F}\\x{7F}\\x{8F}\\x{9F}]"},
                "content": ""
            },
            {
                "type": "Replace",
                "pattern": {"Regex": "[\\x{09}\\x{0A}\\x{0C}\\x{0D}\\x{A0}\\x{1680}\\x{2000}-\\x{200F}\\x{2028}\\x{2029}\\x{202F}\\x{205F}\\x{2581}\\x{3000}\\x{FEFF}\\x{FFFD}]"},
                "content": " "
            },
            {"type": "NFKC"}
        ]
    })
}

/// `clean_up_tokenization` of transformers, which the web export's tokenizer config asks for.
fn clean_up_spaces(text: &str) -> String {
    text.replace(" .", ".")
        .replace(" ?", "?")
        .replace(" !", "!")
        .replace(" ,", ",")
        .replace(" ' ", "'")
        .replace(" n't", "n't")
        .replace(" 'm", "'m")
        .replace(" 's", "'s")
        .replace(" 've", "'ve")
        .replace(" 're", "'re")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_up_like_transformers() {
        assert_eq!(
            clean_up_spaces("I do n't know , really ."),
            "I don't know, really."
        );
        assert_eq!(clean_up_spaces("It 's fine !"), "It's fine!");
    }
}
