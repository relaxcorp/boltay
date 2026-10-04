use std::path::Path;
use std::sync::Mutex;

use ort::inputs;
use ort::session::Session;
use ort::value::TensorRef;

use crate::mel::{LogMel, N_MELS};
use crate::transducer::{argmax, time_major, Vocab};
use crate::{onnx, EngineOptions, Error, Language, Result, Transcriber};

const ENCODER: &str = "v3_e2e_rnnt_encoder.int8.onnx";
const DECODER: &str = "v3_e2e_rnnt_decoder.int8.onnx";
const JOINT: &str = "v3_e2e_rnnt_joint.int8.onnx";
const VOCAB: &str = "v3_e2e_rnnt_vocab.txt";

const ENC_DIM: usize = 768;
const PRED_DIM: usize = 320;
const MAX_TOKENS_PER_STEP: usize = 3;

/// GigaAM v3 e2e RNN-T: Russian with punctuation and casing.
pub struct Gigaam {
    mel: LogMel,
    vocab: Vocab,
    sessions: Mutex<Sessions>,
}

struct Sessions {
    encoder: Session,
    decoder: Session,
    joint: Session,
}

impl Gigaam {
    /// Loads the int8 model from a directory with the original file names.
    pub fn load(dir: &Path, options: EngineOptions) -> Result<Self> {
        let vocab = Vocab::load(&dir.join(VOCAB))?;

        let sessions = Sessions {
            encoder: onnx::heavy(&dir.join(ENCODER), options.graph_cache)?,
            decoder: onnx::light(&dir.join(DECODER))?,
            joint: onnx::light(&dir.join(JOINT))?,
        };

        Ok(Self {
            mel: LogMel::new(),
            vocab,
            sessions: Mutex::new(sessions),
        })
    }

    fn encode(sessions: &mut Sessions, features: &[f32]) -> Result<Vec<f32>> {
        let frames = features.len() / N_MELS;
        let outputs = sessions.encoder.run(inputs![
            "audio_signal" => TensorRef::from_array_view(([1, N_MELS, frames], features))?,
            "length" => TensorRef::from_array_view(([1], &[frames as i64][..]))?,
        ])?;
        let (shape, encoded) = outputs["encoded"].try_extract_tensor::<f32>()?;
        let (_, len) = outputs["encoded_len"].try_extract_tensor::<i32>()?;
        let steps = shape[2] as usize;
        let len = (len[0].max(0) as usize).min(steps);
        Ok(time_major(encoded, ENC_DIM, len))
    }
}

impl Transcriber for Gigaam {
    fn transcribe(&self, samples: &[f32]) -> Result<String> {
        if LogMel::frames(samples.len()) == 0 {
            return Ok(String::new());
        }
        let features = self.mel.compute(samples);
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let encoded = Self::encode(&mut sessions, &features)?;
        let mut net = Network {
            sessions: &mut sessions,
            encoded: &encoded,
            state: [[0.0; PRED_DIM], [0.0; PRED_DIM]],
            pending: None,
        };
        let steps = encoded.len() / ENC_DIM;
        let tokens = greedy_decode(&mut net, steps, self.vocab.blank, MAX_TOKENS_PER_STEP)?;
        Ok(self.vocab.decode(&tokens))
    }

    fn language(&self) -> Language {
        Language::Russian
    }
}

/// The prediction and joint networks as the greedy decoder sees them.
trait Transducer {
    /// Runs the prediction network on `token`. Its new state stays pending until `commit`.
    fn predict(&mut self, token: usize) -> Result<()>;
    fn commit(&mut self);
    /// Most likely token for encoder step `t` given the last prediction.
    fn best_token(&mut self, t: usize) -> Result<usize>;
}

/// Greedy RNN-T decoding, same loop as onnx-asr: the prediction network is rerun only after
/// a non-blank token, at most `max_per_step` tokens are emitted per encoder step.
fn greedy_decode(
    net: &mut impl Transducer,
    steps: usize,
    blank: usize,
    max_per_step: usize,
) -> Result<Vec<usize>> {
    let mut tokens = Vec::new();
    let mut stale = true;
    let mut emitted = 0;
    let mut t = 0;
    while t < steps {
        if stale {
            net.predict(tokens.last().copied().unwrap_or(blank))?;
            stale = false;
        }
        let token = net.best_token(t)?;
        if token != blank {
            net.commit();
            tokens.push(token);
            emitted += 1;
            stale = true;
        }
        if token == blank || emitted == max_per_step {
            t += 1;
            emitted = 0;
        }
    }
    Ok(tokens)
}

type PredState = [[f32; PRED_DIM]; 2];

struct Network<'a> {
    sessions: &'a mut Sessions,
    encoded: &'a [f32],
    state: PredState,
    pending: Option<([f32; PRED_DIM], PredState)>,
}

impl Transducer for Network<'_> {
    fn predict(&mut self, token: usize) -> Result<()> {
        let outputs = self.sessions.decoder.run(inputs![
            "x" => TensorRef::from_array_view(([1, 1], &[token as i64][..]))?,
            "h.1" => TensorRef::from_array_view(([1, 1, PRED_DIM], &self.state[0][..]))?,
            "c.1" => TensorRef::from_array_view(([1, 1, PRED_DIM], &self.state[1][..]))?,
        ])?;
        let read = |name: &str| -> Result<[f32; PRED_DIM]> {
            let (_, v) = outputs[name].try_extract_tensor::<f32>()?;
            v.try_into()
                .map_err(|_| Error::Model(format!("decoder output {name} has {} values", v.len())))
        };
        self.pending = Some((read("dec")?, [read("h")?, read("c")?]));
        Ok(())
    }

    fn commit(&mut self) {
        if let Some((_, state)) = &self.pending {
            self.state = *state;
        }
    }

    fn best_token(&mut self, t: usize) -> Result<usize> {
        let (dec, _) = self
            .pending
            .as_ref()
            .expect("predict runs before the first step");
        let enc = &self.encoded[t * ENC_DIM..(t + 1) * ENC_DIM];
        // [1, 1, 320] and [1, 320, 1] share the same layout, no transpose needed.
        let outputs = self.sessions.joint.run(inputs![
            "enc" => TensorRef::from_array_view(([1, ENC_DIM, 1], enc))?,
            "dec" => TensorRef::from_array_view(([1, PRED_DIM, 1], &dec[..]))?,
        ])?;
        let (_, logits) = outputs["joint"].try_extract_tensor::<f32>()?;
        Ok(argmax(logits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLANK: usize = 0;

    /// Scripted network: `script[t]` lists what the joint returns at step `t`, one entry per
    /// call, then blank.
    struct Scripted {
        script: Vec<Vec<usize>>,
        calls: Vec<usize>,
        predicted: Vec<usize>,
        committed: usize,
    }

    impl Scripted {
        fn new(script: Vec<Vec<usize>>) -> Self {
            let calls = vec![0; script.len()];
            Self {
                script,
                calls,
                predicted: Vec::new(),
                committed: 0,
            }
        }
    }

    impl Transducer for Scripted {
        fn predict(&mut self, token: usize) -> Result<()> {
            self.predicted.push(token);
            Ok(())
        }

        fn commit(&mut self) {
            self.committed += 1;
        }

        fn best_token(&mut self, t: usize) -> Result<usize> {
            let i = self.calls[t];
            self.calls[t] += 1;
            Ok(self.script[t].get(i).copied().unwrap_or(BLANK))
        }
    }

    #[test]
    fn prediction_reruns_only_after_emission() {
        let mut net = Scripted::new(vec![vec![], vec![5, 6], vec![], vec![7]]);
        let tokens = greedy_decode(&mut net, 4, BLANK, 3).unwrap();
        assert_eq!(tokens, [5, 6, 7]);
        assert_eq!(net.predicted, [BLANK, 5, 6, 7]);
        assert_eq!(net.committed, 3);
    }

    #[test]
    fn emissions_per_step_are_capped() {
        let mut net = Scripted::new(vec![vec![1, 2, 3, 4, 5], vec![]]);
        let tokens = greedy_decode(&mut net, 2, BLANK, 3).unwrap();
        // After three tokens the decoder moves on even though the joint wants more.
        assert_eq!(tokens, [1, 2, 3]);
        assert_eq!(net.calls, [3, 1]);
    }
}
