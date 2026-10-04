use std::f64::consts::PI;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

pub const N_MELS: usize = 64;
const N_FFT: usize = 320;
const HOP: usize = 160;
const N_BINS: usize = N_FFT / 2 + 1;
const F_MAX: f64 = 8000.0;

/// Log-mel features of GigaAM v3: no centering, periodic Hann window, HTK mel scale.
/// Computed in f64: the model is quantized to int8 and reacts to rounding noise
/// of an f32 FFT, f64 keeps the result within 2e-6 of the reference.
pub struct LogMel {
    window: Vec<f64>,
    filters: Vec<[f64; N_MELS]>,
    fft: Arc<dyn RealToComplex<f64>>,
}

impl LogMel {
    pub fn new() -> Self {
        Self {
            window: hann_window(),
            filters: mel_filters(),
            fft: RealFftPlanner::new().plan_fft_forward(N_FFT),
        }
    }

    pub fn frames(samples: usize) -> usize {
        if samples < N_FFT {
            0
        } else {
            (samples - N_FFT) / HOP + 1
        }
    }

    /// Returns features laid out as `[N_MELS, frames]`.
    pub fn compute(&self, samples: &[f32]) -> Vec<f32> {
        let frames = Self::frames(samples.len());
        let mut out = vec![0.0; N_MELS * frames];
        let mut frame = self.fft.make_input_vec();
        let mut spectrum = self.fft.make_output_vec();
        let mut scratch = self.fft.make_scratch_vec();
        let mut power = [0f64; N_BINS];

        for t in 0..frames {
            let chunk = &samples[t * HOP..t * HOP + N_FFT];
            for ((dst, &x), &w) in frame.iter_mut().zip(chunk).zip(&self.window) {
                *dst = f64::from(x) * w;
            }
            self.fft
                .process_with_scratch(&mut frame, &mut spectrum, &mut scratch)
                .expect("buffers come from the same plan");
            for (p, z) in power.iter_mut().zip(&spectrum) {
                *p = z.norm_sqr();
            }

            let mut mel = [0f64; N_MELS];
            for (&p, row) in power.iter().zip(&self.filters) {
                for (m, &f) in mel.iter_mut().zip(row) {
                    *m += p * f;
                }
            }
            for (m, &energy) in mel.iter().enumerate() {
                out[m * frames + t] = energy.clamp(1e-9, 1e9).ln() as f32;
            }
        }
        out
    }
}

// GigaAM's exported preprocessor keeps the window and filters in bfloat16. Rounding the
// analytic values the same way reproduces its tables bit for bit.
fn bf16(x: f64) -> f64 {
    let bits = (x as f32).to_bits();
    f32::from_bits((bits + 0x7fff + ((bits >> 16) & 1)) & 0xffff_0000).into()
}

fn hann_window() -> Vec<f64> {
    (0..N_FFT)
        .map(|n| bf16(0.5 - 0.5 * (2.0 * PI * n as f64 / N_FFT as f64).cos()))
        .collect()
}

fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10f64.powf(mel / 2595.0) - 1.0)
}

fn mel_filters() -> Vec<[f64; N_MELS]> {
    // Same point placement as numpy.linspace, the endpoint is exact.
    let mel_max = hz_to_mel(F_MAX);
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

    let bin_step = F_MAX / (N_BINS - 1) as f64;
    (0..N_BINS)
        .map(|k| {
            let freq = k as f64 * bin_step;
            std::array::from_fn(|m| {
                let down = (freq - points[m]) / (points[m + 1] - points[m]);
                let up = (points[m + 2] - freq) / (points[m + 2] - points[m + 1]);
                bf16(down.min(up).max(0.0))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_count_has_no_padding() {
        assert_eq!(LogMel::frames(0), 0);
        assert_eq!(LogMel::frames(319), 0);
        assert_eq!(LogMel::frames(320), 1);
        assert_eq!(LogMel::frames(479), 1);
        assert_eq!(LogMel::frames(480), 2);
        assert_eq!(LogMel::frames(16_000), 99);
    }

    #[test]
    fn tables_match_reference() {
        // Spot values from onnx-asr fbanks.npz (gigaam_v3, gigaam_v3_window).
        let window = hann_window();
        assert_eq!(window[0] as f32, 0.0);
        assert_eq!(window[1] as f32, 9.632_110_6e-5);
        assert_eq!(window[37] as f32, 0.125_976_56);
        assert_eq!(window[101] as f32, 0.699_218_75);
        assert_eq!(window[160] as f32, 1.0);

        let filters = mel_filters();
        assert_eq!(filters[1][0] as f32, 0.223_632_81);
        assert_eq!(filters[101][53] as f32, 0.6875);
        assert_eq!(filters[140][60] as f32, 0.152_343_75);
        assert_eq!(filters[159][63] as f32, 0.151_367_19);
        assert_eq!(filters[160][63] as f32, 5.495_604e-15);
        assert_eq!(filters.iter().flatten().filter(|&&f| f > 0.0).count(), 313);
    }

    #[test]
    fn tone_lands_in_its_band() {
        let tone: Vec<f32> = (0..16_000)
            .map(|n| (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / 16_000.0).sin())
            .collect();
        let mel = LogMel::new().compute(&tone);
        let frames = LogMel::frames(tone.len());
        let loudest = (0..N_MELS)
            .max_by(|&a, &b| mel[a * frames + 10].total_cmp(&mel[b * frames + 10]))
            .unwrap();
        let points: Vec<f64> = (0..N_MELS + 2)
            .map(|i| mel_to_hz(i as f64 * hz_to_mel(F_MAX) / (N_MELS + 1) as f64))
            .collect();
        assert!(points[loudest] < 1000.0 && 1000.0 < points[loudest + 2]);
    }

    #[test]
    fn silence_is_clamped() {
        let mel = LogMel::new().compute(&[0.0; 800]);
        assert!(mel.iter().all(|&v| v == 1e-9f32.ln()));
    }
}
