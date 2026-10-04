use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use ort::inputs;
use ort::session::Session;
use ort::value::TensorRef;

use crate::audio::SAMPLE_RATE;
use crate::{onnx, Result};

const HOP: usize = 512;
const CONTEXT: usize = 64;
const STATE: usize = 2 * 128;

/// Speech ends once the probability drops this far below the threshold it started at.
const HYSTERESIS: f32 = 0.15;
const MIN_SPEECH: usize = 4_000;
const MIN_SILENCE: usize = 1_600;
const MAX_SPEECH: usize = 20 * SAMPLE_RATE as usize;
pub(crate) const PAD: usize = 480;

/// Silero VAD v5.
pub struct Vad {
    session: Mutex<Session>,
    /// As `f32` bits: settable while recognition runs on another thread.
    threshold: AtomicU32,
}

impl Vad {
    /// The onnx-asr default.
    pub const DEFAULT_THRESHOLD: f32 = 0.5;

    pub fn load(path: &Path) -> Result<Self> {
        let session = onnx::light(path)?;
        Ok(Self {
            session: Mutex::new(session),
            threshold: AtomicU32::new(Self::DEFAULT_THRESHOLD.to_bits()),
        })
    }

    /// Speech probability that starts a segment. Lower catches quiet speech and whispers,
    /// higher ignores more background noise.
    pub fn set_threshold(&self, threshold: f32) {
        let threshold = threshold.clamp(HYSTERESIS + 0.01, 0.99);
        self.threshold.store(threshold.to_bits(), Ordering::Relaxed);
    }

    /// Speech segments of 16 kHz audio, in samples. Segmentation follows onnx-asr defaults.
    pub fn segments(&self, samples: &[f32]) -> Result<Vec<Range<usize>>> {
        let probs = self.probabilities(samples)?;
        let threshold = f32::from_bits(self.threshold.load(Ordering::Relaxed));
        Ok(merge(&find_speech(&probs, threshold), samples.len()))
    }

    /// Speech probability for each 512-sample hop. Every window also carries the last
    /// 64 samples of the previous hop, the last hop is zero-padded.
    fn probabilities(&self, samples: &[f32]) -> Result<Vec<f32>> {
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = vec![0f32; STATE];
        let mut window = [0f32; CONTEXT + HOP];
        let sr = [SAMPLE_RATE as i64];
        let mut probs = Vec::with_capacity(samples.len().div_ceil(HOP));

        for start in (0..samples.len()).step_by(HOP) {
            window.fill(0.0);
            let from = start.saturating_sub(CONTEXT);
            let to = (start + HOP).min(samples.len());
            window[CONTEXT - (start - from)..CONTEXT + (to - start)]
                .copy_from_slice(&samples[from..to]);

            let outputs = session.run(inputs![
                "input" => TensorRef::from_array_view(([1, CONTEXT + HOP], &window[..]))?,
                "state" => TensorRef::from_array_view(([2, 1, 128], &state[..]))?,
                "sr" => TensorRef::from_array_view(((), &sr[..]))?,
            ])?;
            let (_, prob) = outputs["output"].try_extract_tensor::<f32>()?;
            probs.push(prob[0]);
            let (_, next) = outputs["stateN"].try_extract_tensor::<f32>()?;
            state.copy_from_slice(next);
        }
        Ok(probs)
    }
}

fn find_speech(probs: &[f32], threshold: f32) -> Vec<Range<usize>> {
    let mut segments = Vec::new();
    let mut start = None;
    for (i, &p) in probs.iter().chain([&0.0]).enumerate() {
        match start {
            None if p >= threshold => start = Some(i * HOP),
            Some(s) if p < threshold - HYSTERESIS => {
                segments.push(s..i * HOP);
                start = None;
            }
            _ => {}
        }
    }
    segments
}

/// Joins segments separated by short pauses, drops too short ones, splits too long ones
/// and pads each side. A straight port of onnx-asr `BaseVad._merge_segments`.
fn merge(segments: &[Range<usize>], len: usize) -> Vec<Range<usize>> {
    const INF: i64 = 1_000_000_000_000_000;
    let n = len as i64;
    let pad = PAD as i64;
    let min_speech = MIN_SPEECH as i64 - 2 * pad;
    let max_speech = MAX_SPEECH as i64 - 2 * pad;
    let min_silence = MIN_SILENCE as i64 + 2 * pad;
    let clip = |a: i64, b: i64| a.max(0) as usize..b.min(n) as usize;

    let mut out = Vec::new();
    let (mut cur_start, mut cur_end) = (-INF, -INF);
    let tail = [(n, n), (INF, INF)];
    for (mut start, end) in segments
        .iter()
        .map(|s| (s.start as i64, s.end as i64))
        .chain(tail)
    {
        if start - cur_end < min_silence && end - cur_start < max_speech {
            cur_end = end;
            continue;
        }
        if cur_start < n && cur_end > cur_start && cur_end - cur_start > min_speech {
            out.push(clip(cur_start - pad, cur_end + pad));
        }
        while end - start > max_speech {
            out.push(clip(start - pad, start + max_speech + pad));
            start += max_speech;
        }
        (cur_start, cur_end) = (start, end);
    }
    out
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    const S: usize = SAMPLE_RATE as usize;

    #[test]
    fn hysteresis_between_thresholds() {
        let probs = [0.1, 0.6, 0.4, 0.36, 0.2, 0.5, 0.9];
        let t = Vad::DEFAULT_THRESHOLD;
        assert_eq!(find_speech(&probs, t), [HOP..4 * HOP, 5 * HOP..7 * HOP]);
        assert!(find_speech(&[0.49; 10], t).is_empty());
    }

    #[test]
    fn lower_threshold_hears_quieter_speech() {
        let whisper = [0.1, 0.4, 0.42, 0.38, 0.1];
        assert!(find_speech(&whisper, Vad::DEFAULT_THRESHOLD).is_empty());
        assert_eq!(find_speech(&whisper, 0.35), [HOP..4 * HOP]);
    }

    #[test]
    fn short_blips_are_dropped() {
        assert!(merge(&[1_000..3_000], 10 * S).is_empty());
        assert_eq!(merge(&[1_000..5_000], 10 * S), [520..5_480]);
    }

    #[test]
    fn short_pauses_are_joined() {
        let segments = [S..2 * S, 2 * S + 2_000..3 * S, 5 * S..6 * S];
        assert_eq!(
            merge(&segments, 10 * S),
            [S - PAD..3 * S + PAD, 5 * S - PAD..6 * S + PAD]
        );
    }

    #[test]
    fn padding_is_clipped_to_audio() {
        assert_eq!(merge(&[0..S], S), [0..S]);
    }

    #[test]
    fn long_speech_is_split() {
        let merged = merge(&[0..50 * S], 50 * S);
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().all(|r| r.len() <= MAX_SPEECH));
        assert_eq!(merged[0].start, 0);
        assert_eq!(merged[2].end, 50 * S);
    }
}
