use std::f64::consts::PI;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

pub const N_MELS: usize = 128;
const N_FFT: usize = 512;
const WIN: usize = 400;
const HOP: usize = 160;
const N_BINS: usize = N_FFT / 2 + 1;
const SAMPLE_RATE: f64 = 16_000.0;
const PREEMPH: f32 = 0.97;
const LOG_GUARD: f64 = 1.0 / (1 << 24) as f64;

/// NeMo log-mel features as Parakeet was trained on: pre-emphasis, centered 25 ms Hann
/// frames, Slaney mel scale, then per-feature normalization over the utterance.
/// Computed in f64 for the same reason as the GigaAM features.
pub struct NemoMel {
    window: Vec<f64>,
    filters: Vec<[f64; N_MELS]>,
    fft: Arc<dyn RealToComplex<f64>>,
}

impl NemoMel {
    pub fn new() -> Self {
        Self {
            window: window(),
            filters: mel_filters(),
            fft: RealFftPlanner::new().plan_fft_forward(N_FFT),
        }
    }

    /// Frames in the output, the last one is padding and stays zero.
    pub fn frames(samples: usize) -> usize {
        samples / HOP + 1
    }

    /// Frames the encoder should look at.
    pub fn valid_frames(samples: usize) -> usize {
        samples / HOP
    }

    /// Returns features laid out as `[N_MELS, frames]`.
    pub fn compute(&self, samples: &[f32]) -> Vec<f32> {
        let frames = Self::frames(samples.len());
        let valid = Self::valid_frames(samples.len());

        // Pre-emphasis stays in f32 to round exactly like the reference.
        let mut padded = vec![0f64; samples.len() + N_FFT];
        let mut prev = 0f32;
        for (dst, &x) in padded[N_FFT / 2..].iter_mut().zip(samples) {
            *dst = f64::from(x - PREEMPH * prev);
            prev = x;
        }

        let mut logmel = vec![0f64; N_MELS * valid];
        let mut frame = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut scratch = self.fft.make_scratch_vec();
        for t in 0..valid {
            let chunk = &padded[t * HOP..t * HOP + N_FFT];
            for ((dst, &x), &w) in frame.iter_mut().zip(chunk).zip(&self.window) {
                *dst = x * w;
            }
            self.fft
                .process_with_scratch(&mut frame, &mut spectrum, &mut scratch)
                .expect("buffers come from the same plan");
            let mut mel = [0f64; N_MELS];
            for (z, row) in spectrum.iter().zip(&self.filters) {
                let p = z.norm_sqr();
                for (m, &f) in mel.iter_mut().zip(row) {
                    *m += p * f;
                }
            }
            for (m, &energy) in mel.iter().enumerate() {
                logmel[m * valid + t] = (energy + LOG_GUARD).ln();
            }
        }

        let mut out = vec![0f32; N_MELS * frames];
        if valid < 2 {
            return out;
        }
        for (m, row) in logmel.chunks_exact(valid).enumerate() {
            let mean = row.iter().sum::<f64>() / valid as f64;
            let var = row.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (valid - 1) as f64;
            let scale = var.sqrt() + 1e-5;
            for (dst, v) in out[m * frames..m * frames + valid].iter_mut().zip(row) {
                *dst = ((v - mean) / scale) as f32;
            }
        }
        out
    }
}

/// Symmetric 400-point Hann window centered in the 512-point frame.
fn window() -> Vec<f64> {
    let offset = (N_FFT - WIN) / 2;
    (0..N_FFT)
        .map(|i| match i.checked_sub(offset) {
            Some(n) if n < WIN => 0.5 - 0.5 * (2.0 * PI * n as f64 / (WIN - 1) as f64).cos(),
            _ => 0.0,
        })
        .collect()
}

const F_SP: f64 = 200.0 / 3.0;
const MIN_LOG_HZ: f64 = 1000.0;
const MIN_LOG_MEL: f64 = MIN_LOG_HZ / F_SP;

fn log_step() -> f64 {
    6.4f64.ln() / 27.0
}

fn hz_to_mel(hz: f64) -> f64 {
    if hz >= MIN_LOG_HZ {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / log_step()
    } else {
        hz / F_SP
    }
}

fn mel_to_hz(mel: f64) -> f64 {
    if mel >= MIN_LOG_MEL {
        MIN_LOG_HZ * (log_step() * (mel - MIN_LOG_MEL)).exp()
    } else {
        F_SP * mel
    }
}

/// Slaney-normalized mel filters as librosa builds them, rounded to f32 like the
/// reference table.
fn mel_filters() -> Vec<[f64; N_MELS]> {
    let mel_max = hz_to_mel(SAMPLE_RATE / 2.0);
    let step = mel_max / (N_MELS + 1) as f64;
    let points: Vec<f64> = (0..N_MELS + 2)
        .map(|i| {
            let mel = if i == N_MELS + 1 {
                mel_max
            } else {
                i as f64 * step
            };
            mel_to_hz(mel)
        })
        .collect();

    (0..N_BINS)
        .map(|k| {
            let freq = k as f64 * SAMPLE_RATE / N_FFT as f64;
            std::array::from_fn(|m| {
                let down = (freq - points[m]) / (points[m + 1] - points[m]);
                let up = (points[m + 2] - freq) / (points[m + 2] - points[m + 1]);
                let norm = 2.0 / (points[m + 2] - points[m]);
                f64::from((down.min(up).max(0.0) * norm) as f32)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_centered() {
        assert_eq!(NemoMel::frames(0), 1);
        assert_eq!(NemoMel::valid_frames(159), 0);
        assert_eq!(NemoMel::valid_frames(16_000), 100);
        assert_eq!(NemoMel::frames(16_000), 101);
    }

    #[test]
    fn filters_match_reference() {
        // Spot values from onnx-asr fbanks.npz (nemo128). That table was built by a different
        // implementation of the same formula, entries differ by up to 3e-8.
        let filters = mel_filters();
        let expected = [
            (1, 0, 0.028_377_542),
            (31, 40, 0.024_372_78),
            (61, 68, 0.010_636_676),
            (91, 85, 0.013_066_837),
            (121, 96, 0.000_869_819_95),
            (151, 106, 0.007_866_095),
        ];
        for (k, m, v) in expected {
            assert!(
                (filters[k][m] - v).abs() < 5e-8,
                "{k} {m}: {}",
                filters[k][m]
            );
        }
        assert_eq!(filters.iter().flatten().filter(|&&f| f > 0.0).count(), 504);
    }

    #[test]
    fn window_is_centered_hann() {
        let w = window();
        assert_eq!(w[55], 0.0);
        assert_eq!(w[56], 0.0);
        assert!((w[56 + 200] - 0.999_984_5).abs() < 1e-6);
        assert_eq!(w[511], 0.0);
    }

    #[test]
    fn features_are_normalized() {
        let tone: Vec<f32> = (0..16_000)
            .map(|n| (n as f32 * 0.3).sin() * (1.0 + (n as f32 * 0.001).sin()))
            .collect();
        let frames = NemoMel::frames(tone.len());
        let features = NemoMel::new().compute(&tone);
        for row in features.chunks_exact(frames) {
            let valid = &row[..frames - 1];
            let mean = valid.iter().sum::<f32>() / valid.len() as f32;
            assert!(mean.abs() < 1e-4, "{mean}");
            assert_eq!(row[frames - 1], 0.0);
        }
    }
}
