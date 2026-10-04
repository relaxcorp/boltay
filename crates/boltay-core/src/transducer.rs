use std::fs;
use std::path::Path;

use crate::{Error, Result};

/// SentencePiece vocabulary in the `token id` per line format of the exported models.
pub struct Vocab {
    tokens: Vec<String>,
    pub blank: usize,
}

impl Vocab {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?;
        let tokens = parse(&text)
            .ok_or_else(|| Error::Model(format!("{}: malformed vocabulary", path.display())))?;
        let blank = tokens
            .iter()
            .position(|t| t == "<blk>")
            .ok_or_else(|| Error::Model(format!("{}: no <blk> token", path.display())))?;
        Ok(Self { tokens, blank })
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn decode(&self, ids: &[usize]) -> String {
        let text: String = ids.iter().map(|&t| self.tokens[t].as_str()).collect();
        join_pieces(&text)
    }
}

/// First index of the maximum, like `numpy.argmax`.
pub fn argmax(x: &[f32]) -> usize {
    let mut best = 0;
    for (i, &v) in x.iter().enumerate() {
        if v > x[best] {
            best = i;
        }
    }
    best
}

/// Transposes encoder output from `[channels, steps]` to `[len, channels]`, keeping the
/// first `len` steps, so each step is a contiguous slice for the joint network.
pub fn time_major(encoded: &[f32], channels: usize, len: usize) -> Vec<f32> {
    let steps = encoded.len() / channels;
    let mut out = vec![0f32; len * channels];
    for (c, row) in encoded.chunks_exact(steps.max(1)).enumerate() {
        for (t, &v) in row[..len].iter().enumerate() {
            out[t * channels + c] = v;
        }
    }
    out
}

fn parse(text: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let (token, id) = line.rsplit_once(' ')?;
        if id.parse::<usize>().ok()? != i {
            return None;
        }
        tokens.push(token.replace('\u{2581}', " "));
    }
    Some(tokens)
}

/// Drops the leading space and spaces not followed by a word character,
/// same as onnx-asr's `\A\s|\s\B|(\s)\b` substitution.
fn join_pieces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut first = true;
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            let before_word = chars
                .peek()
                .is_some_and(|&n| n.is_alphanumeric() || n == '_');
            if !first && before_word {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
        first = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argmax_takes_first_maximum() {
        assert_eq!(argmax(&[0.1, 0.7, 0.7, 0.2]), 1);
        assert_eq!(argmax(&[-3.0]), 0);
    }

    #[test]
    fn time_major_keeps_valid_steps() {
        // Two channels, three steps, the last one is padding.
        let encoded = [1.0, 2.0, 9.0, 3.0, 4.0, 9.0];
        assert_eq!(time_major(&encoded, 2, 2), [1.0, 3.0, 2.0, 4.0]);
        assert!(time_major(&[], 2, 0).is_empty());
    }

    #[test]
    fn vocab_lines_must_be_sequential() {
        let tokens = parse("<unk> 0\n\u{2581} 1\n\u{2581}при 2\n<blk> 3\n").unwrap();
        assert_eq!(tokens, ["<unk>", " ", " при", "<blk>"]);
        assert!(parse("a 0\nb 2\n").is_none());
        assert!(parse("a0\n").is_none());
    }

    #[test]
    fn pieces_join_like_reference() {
        assert_eq!(join_pieces(" привет, мир"), "привет, мир");
        assert_eq!(join_pieces(" Да ,  нет ."), "Да, нет.");
        assert_eq!(join_pieces(" 25 лет"), "25 лет");
        assert_eq!(join_pieces(" «Ёлки»"), "«Ёлки»");
        assert_eq!(join_pieces(" конец "), "конец");
        assert_eq!(join_pieces(""), "");
    }
}
