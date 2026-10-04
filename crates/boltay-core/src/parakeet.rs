use std::path::Path;
use std::sync::Mutex;

use ort::inputs;
use ort::session::Session;
use ort::value::TensorRef;

use crate::nemo_mel::{NemoMel, N_MELS};
use crate::transducer::{argmax, time_major, Vocab};
use crate::{onnx, EngineOptions, Error, Language, Result, Transcriber};

const ENCODER: &str = "encoder-model.int8.onnx";
const DECODER_JOINT: &str = "decoder_joint-model.int8.onnx";
const VOCAB: &str = "vocab.txt";

const ENC_DIM: usize = 1024;
const STATE: usize = 2 * 640;
const MAX_TOKENS_PER_STEP: usize = 10;

/// Parakeet TDT 0.6B v3: 25 European languages with punctuation and casing.
pub struct Parakeet {
    mel: NemoMel,
    vocab: Vocab,
    sessions: Mutex<Sessions>,
}

struct Sessions {
    encoder: Session,
    decoder_joint: Session,
}

impl Parakeet {
    /// Loads the int8 model from a directory with the original file names.
    pub fn load(dir: &Path, options: EngineOptions) -> Result<Self> {
        let vocab = Vocab::load(&dir.join(VOCAB))?;
        let sessions = Sessions {
            encoder: onnx::heavy(&dir.join(ENCODER), options.graph_cache)?,
            decoder_joint: onnx::light(&dir.join(DECODER_JOINT))?,
        };
        Ok(Self {
            mel: NemoMel::new(),
            vocab,
            sessions: Mutex::new(sessions),
        })
    }

    fn encode(sessions: &mut Sessions, features: &[f32], valid: usize) -> Result<Vec<f32>> {
        let frames = features.len() / N_MELS;
        let outputs = sessions.encoder.run(inputs![
            "audio_signal" => TensorRef::from_array_view(([1, N_MELS, frames], features))?,
            "length" => TensorRef::from_array_view(([1], &[valid as i64][..]))?,
        ])?;
        let (shape, encoded) = outputs["outputs"].try_extract_tensor::<f32>()?;
        let (_, len) = outputs["encoded_lengths"].try_extract_tensor::<i64>()?;
        let len = (len[0].max(0) as usize).min(shape[2] as usize);
        Ok(time_major(encoded, ENC_DIM, len))
    }
}

impl Transcriber for Parakeet {
    fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let valid = NemoMel::valid_frames(samples.len());
        // Normalization needs a variance, one frame has none.
        if valid < 2 {
            return Ok(String::new());
        }
        let features = self.mel.compute(samples);
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let encoded = Self::encode(&mut sessions, &features, valid)?;
        let mut net = Network {
            session: &mut sessions.decoder_joint,
            encoded: &encoded,
            vocab_size: self.vocab.len(),
            state: vec![0.0; 2 * STATE],
            pending: Vec::new(),
        };
        let steps = encoded.len() / ENC_DIM;
        let tokens = tdt_decode(&mut net, steps, self.vocab.blank, MAX_TOKENS_PER_STEP)?;
        Ok(self.vocab.decode(&tokens))
    }

    fn language(&self) -> Language {
        Language::Other
    }
}

/// Decoder and joint networks of a token-and-duration transducer.
trait Tdt {
    /// Best token and duration for encoder step `t` after `token`. The new decoder state
    /// stays pending until `commit`.
    fn step(&mut self, t: usize, token: usize) -> Result<(usize, usize)>;
    fn commit(&mut self);
}

/// Greedy TDT decoding, same loop as onnx-asr: the predicted duration moves along the
/// encoder steps, a zero duration stays on the step for at most `max_per_step` tokens.
fn tdt_decode(
    net: &mut impl Tdt,
    steps: usize,
    blank: usize,
    max_per_step: usize,
) -> Result<Vec<usize>> {
    let mut tokens = Vec::new();
    let mut emitted = 0;
    let mut t = 0;
    while t < steps {
        let (token, duration) = net.step(t, tokens.last().copied().unwrap_or(blank))?;
        if token != blank {
            net.commit();
            tokens.push(token);
            emitted += 1;
        }
        if duration > 0 {
            t += duration;
            emitted = 0;
        } else if token == blank || emitted == max_per_step {
            t += 1;
            emitted = 0;
        }
    }
    Ok(tokens)
}

struct Network<'a> {
    session: &'a mut Session,
    encoded: &'a [f32],
    vocab_size: usize,
    state: Vec<f32>,
    pending: Vec<f32>,
}

impl Tdt for Network<'_> {
    fn step(&mut self, t: usize, token: usize) -> Result<(usize, usize)> {
        let enc = &self.encoded[t * ENC_DIM..(t + 1) * ENC_DIM];
        let (h, c) = self.state.split_at(STATE);
        let outputs = self.session.run(inputs![
            "encoder_outputs" => TensorRef::from_array_view(([1, ENC_DIM, 1], enc))?,
            "targets" => TensorRef::from_array_view(([1, 1], &[token as i32][..]))?,
            "target_length" => TensorRef::from_array_view(([1], &[1i32][..]))?,
            "input_states_1" => TensorRef::from_array_view(([2, 1, 640], h))?,
            "input_states_2" => TensorRef::from_array_view(([2, 1, 640], c))?,
        ])?;
        let (_, logits) = outputs["outputs"].try_extract_tensor::<f32>()?;
        if logits.len() <= self.vocab_size {
            return Err(Error::Model(format!(
                "joint output has {} values for {} tokens",
                logits.len(),
                self.vocab_size
            )));
        }
        let (token_logits, duration_logits) = logits.split_at(self.vocab_size);

        self.pending.clear();
        for name in ["output_states_1", "output_states_2"] {
            let (_, s) = outputs[name].try_extract_tensor::<f32>()?;
            self.pending.extend_from_slice(s);
        }
        Ok((argmax(token_logits), argmax(duration_logits)))
    }

    fn commit(&mut self) {
        if self.pending.len() == self.state.len() {
            self.state.copy_from_slice(&self.pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLANK: usize = 0;

    /// Replays `(token, duration)` pairs and records what the decoder was fed.
    struct Scripted {
        script: std::vec::IntoIter<(usize, usize)>,
        visited: Vec<(usize, usize)>,
        committed: usize,
    }

    impl Scripted {
        fn new(script: Vec<(usize, usize)>) -> Self {
            Self {
                script: script.into_iter(),
                visited: Vec::new(),
                committed: 0,
            }
        }
    }

    impl Tdt for Scripted {
        fn step(&mut self, t: usize, token: usize) -> Result<(usize, usize)> {
            self.visited.push((t, token));
            Ok(self.script.next().unwrap_or((BLANK, 1)))
        }

        fn commit(&mut self) {
            self.committed += 1;
        }
    }

    #[test]
    fn durations_skip_encoder_steps() {
        let mut net = Scripted::new(vec![(5, 2), (BLANK, 3), (6, 0), (7, 1)]);
        let tokens = tdt_decode(&mut net, 6, BLANK, 10).unwrap();
        assert_eq!(tokens, [5, 6, 7]);
        assert_eq!(net.visited, [(0, BLANK), (2, 5), (5, 5), (5, 6)]);
        assert_eq!(net.committed, 3);
    }

    #[test]
    fn blank_with_zero_duration_still_advances() {
        let mut net = Scripted::new(vec![(BLANK, 0), (BLANK, 0)]);
        assert!(tdt_decode(&mut net, 2, BLANK, 10).unwrap().is_empty());
        assert_eq!(net.visited, [(0, BLANK), (1, BLANK)]);
    }

    #[test]
    fn zero_durations_are_capped() {
        let mut net = Scripted::new(vec![(3, 0); 20]);
        let tokens = tdt_decode(&mut net, 1, BLANK, 4).unwrap();
        assert_eq!(tokens, [3; 4]);
    }
}
