use crate::{Error, Result};

pub struct SpmModel {
    /// Piece and its log probability, in model order.
    pub pieces: Vec<(String, f64)>,
    pub unk_id: usize,
    pub charsmap: Vec<u8>,
    pub unigram: bool,
}

const TYPE_UNKNOWN: u64 = 2;
const MODEL_UNIGRAM: u64 = 1;

impl SpmModel {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut model = SpmModel {
            pieces: Vec::new(),
            unk_id: 0,
            charsmap: Vec::new(),
            unigram: true,
        };
        for field in Fields::new(data) {
            match field? {
                // repeated SentencePiece pieces = 1
                (1, Value::Bytes(piece)) => {
                    let (mut text, mut score, mut kind) = (String::new(), 0.0, 1);
                    for f in Fields::new(piece) {
                        match f? {
                            (1, Value::Bytes(b)) => text = String::from_utf8_lossy(b).into_owned(),
                            (2, Value::Fixed32(v)) => score = f32::from_bits(v).into(),
                            (3, Value::Varint(v)) => kind = v,
                            _ => {}
                        }
                    }
                    if kind == TYPE_UNKNOWN {
                        model.unk_id = model.pieces.len();
                    }
                    model.pieces.push((text, score));
                }
                // TrainerSpec trainer_spec = 2, model_type = 3
                (2, Value::Bytes(spec)) => {
                    for f in Fields::new(spec) {
                        if let (3, Value::Varint(v)) = f? {
                            model.unigram = v == MODEL_UNIGRAM;
                        }
                    }
                }
                // NormalizerSpec normalizer_spec = 3, precompiled_charsmap = 2
                (3, Value::Bytes(spec)) => {
                    for f in Fields::new(spec) {
                        if let (2, Value::Bytes(b)) = f? {
                            model.charsmap = b.to_vec();
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(model)
    }
}

enum Value<'a> {
    Varint(u64),
    Fixed64,
    Bytes(&'a [u8]),
    Fixed32(u32),
}

struct Fields<'a> {
    data: &'a [u8],
}

impl<'a> Fields<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    fn varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = self.data.split_first().ok_or_else(truncated)?;
            self.data = rest;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(truncated())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.data.len() < n {
            return Err(truncated());
        }
        let (head, rest) = self.data.split_at(n);
        self.data = rest;
        Ok(head)
    }

    fn field(&mut self) -> Result<(u64, Value<'a>)> {
        let key = self.varint()?;
        let value = match key & 7 {
            0 => Value::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                Value::Fixed64
            }
            2 => {
                let len = self.varint()? as usize;
                Value::Bytes(self.take(len)?)
            }
            5 => {
                let b = self.take(4)?;
                Value::Fixed32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            wire => {
                return Err(Error::Tokenizer(format!(
                    "sentencepiece model: wire type {wire}"
                )))
            }
        };
        Ok((key >> 3, value))
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u64, Value<'a>)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        let field = self.field();
        if field.is_err() {
            self.data = &[];
        }
        Some(field)
    }
}

fn truncated() -> Error {
    Error::Tokenizer("sentencepiece model is truncated".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(text: &str, score: f32, kind: u8) -> Vec<u8> {
        let mut p = vec![0x0a, text.len() as u8];
        p.extend_from_slice(text.as_bytes());
        p.push(0x15);
        p.extend_from_slice(&score.to_le_bytes());
        p.extend_from_slice(&[0x18, kind]);
        let mut out = vec![0x0a, p.len() as u8];
        out.extend(p);
        out
    }

    #[test]
    fn reads_pieces_and_normalizer() {
        let mut data = piece("<unk>", 0.0, 2);
        data.extend(piece("▁да", -3.5, 1));
        // trainer_spec { model_type: UNIGRAM }, normalizer_spec { name: "x", charsmap: [1, 2] }
        data.extend([0x12, 0x02, 0x18, 0x01]);
        data.extend([0x1a, 0x07, 0x0a, 0x01, b'x', 0x12, 0x02, 0x01, 0x02]);
        let model = SpmModel::parse(&data).unwrap();
        assert_eq!(model.pieces, [("<unk>".into(), 0.0), ("▁да".into(), -3.5)]);
        assert_eq!(model.unk_id, 0);
        assert_eq!(model.charsmap, [1, 2]);
        assert!(model.unigram);
    }

    #[test]
    fn truncated_data_is_an_error() {
        assert!(SpmModel::parse(&[0x0a, 0x05, 0x01]).is_err());
    }
}
