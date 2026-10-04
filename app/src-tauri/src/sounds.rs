use std::f32::consts::TAU;
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
}

/// The start and stop sounds. All synthesized, no audio files in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SoundSet {
    /// Two notes, up for start and down for stop.
    #[default]
    Soft,
    Drop,
    Click,
    Sharp,
    Bell,
    /// A cracking shell and a knock.
    Shell,
}

/// Plays in the background. A missing or busy output device is not worth an error.
pub fn play(set: SoundSet, cue: Cue) {
    thread::spawn(move || {
        let _ = play_blocking(|rate| render(set, cue, rate));
    });
}

/// Start, a breath, stop: what the user hears around one dictation.
pub fn preview(set: SoundSet) {
    thread::spawn(move || {
        let _ = play_blocking(|rate| {
            let mut mix = Mix::new(rate);
            recipe(set, Cue::Start, &mut mix, 0.0);
            recipe(set, Cue::Stop, &mut mix, 0.65);
            mix.finish()
        });
    });
}

fn play_blocking(render: impl FnOnce(u32) -> Vec<f32>) -> Option<()> {
    let device = cpal::default_host().default_output_device()?;
    let supported = device.default_output_config().ok()?;
    let config = supported.config();
    let wave = render(config.sample_rate);
    let length = Duration::from_secs_f32(wave.len() as f32 / config.sample_rate as f32);
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, wave),
        SampleFormat::I16 => build::<i16>(&device, config, wave),
        SampleFormat::U16 => build::<u16>(&device, config, wave),
        SampleFormat::I32 => build::<i32>(&device, config, wave),
        _ => None,
    }?;
    stream.play().ok()?;
    thread::sleep(length + Duration::from_millis(80));
    Some(())
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    wave: Vec<f32>,
) -> Option<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut pos = 0;
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _: &_| {
                for frame in out.chunks_mut(channels) {
                    let sample = wave.get(pos).copied().unwrap_or(0.0);
                    pos += 1;
                    frame.fill(sample.to_sample::<T>());
                }
            },
            |_| {},
            None,
        )
        .ok()
}

/// Mono samples at `rate` for one cue.
pub fn render(set: SoundSet, cue: Cue, rate: u32) -> Vec<f32> {
    let mut mix = Mix::new(rate);
    recipe(set, cue, &mut mix, 0.0);
    mix.finish()
}

fn recipe(set: SoundSet, cue: Cue, m: &mut Mix, t: f32) {
    use Wave::{Sine, Square, Triangle};
    match (set, cue) {
        (SoundSet::Soft, Cue::Start) => {
            m.osc(t, Sine, 660.0, 660.0, 0.09, 0.18);
            m.osc(t + 0.07, Sine, 990.0, 990.0, 0.12, 0.18);
        }
        (SoundSet::Soft, Cue::Stop) => {
            m.osc(t, Sine, 990.0, 990.0, 0.09, 0.18);
            m.osc(t + 0.07, Sine, 660.0, 660.0, 0.12, 0.18);
        }
        (SoundSet::Drop, Cue::Start) => m.osc(t, Sine, 1500.0, 520.0, 0.12, 0.26),
        (SoundSet::Drop, Cue::Stop) => m.osc(t, Sine, 900.0, 320.0, 0.14, 0.24),
        (SoundSet::Click, Cue::Start) => {
            m.noise(t, 0.018, 0.9, 2600.0, 1.5);
            m.osc(t, Triangle, 1800.0, 1800.0, 0.02, 0.06);
        }
        (SoundSet::Click, Cue::Stop) => {
            m.noise(t, 0.016, 0.8, 1500.0, 1.5);
            m.noise(t + 0.06, 0.016, 0.6, 1300.0, 1.5);
        }
        (SoundSet::Sharp, Cue::Start) => m.osc(t, Square, 1200.0, 1200.0, 0.06, 0.07),
        (SoundSet::Sharp, Cue::Stop) => {
            m.osc(t, Square, 900.0, 900.0, 0.045, 0.07);
            m.osc(t + 0.08, Square, 900.0, 900.0, 0.045, 0.07);
        }
        (SoundSet::Bell, Cue::Start) => {
            m.osc(t, Sine, 1320.0, 1320.0, 0.55, 0.16);
            m.osc(t, Sine, 2640.0, 2640.0, 0.35, 0.05);
            m.osc(t, Sine, 3960.0, 3960.0, 0.2, 0.025);
        }
        (SoundSet::Bell, Cue::Stop) => {
            m.osc(t, Sine, 990.0, 990.0, 0.6, 0.16);
            m.osc(t, Sine, 1980.0, 1980.0, 0.4, 0.05);
        }
        (SoundSet::Shell, Cue::Start) => {
            m.noise(t, 0.012, 1.0, 4200.0, 0.9);
            m.noise(t + 0.028, 0.01, 0.7, 5200.0, 0.9);
            m.osc(t, Sine, 220.0, 140.0, 0.05, 0.08);
        }
        (SoundSet::Shell, Cue::Stop) => {
            m.osc(t, Sine, 340.0, 240.0, 0.07, 0.22);
            m.noise(t, 0.01, 0.25, 900.0, 2.0);
        }
    }
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Triangle,
}

/// A buffer the voices are added into, with a little tail so nothing is cut short.
struct Mix {
    rate: f32,
    out: Vec<f32>,
    seed: u32,
}

/// Gain the envelopes start from and fall back to: an exponential ramp cannot reach zero.
const FLOOR: f32 = 0.0001;
const ATTACK: f32 = 0.006;

impl Mix {
    fn new(rate: u32) -> Self {
        Self {
            rate: rate as f32,
            out: Vec::new(),
            seed: 0x2545_f491,
        }
    }

    fn slot(&mut self, at: f32, len: usize) -> usize {
        let start = (at * self.rate) as usize;
        if self.out.len() < start + len {
            self.out.resize(start + len, 0.0);
        }
        start
    }

    /// A tone gliding exponentially from `f0` to `f1`, with a 6 ms rise and an exponential
    /// fall over `dur`.
    fn osc(&mut self, at: f32, wave: Wave, f0: f32, f1: f32, dur: f32, vol: f32) {
        let len = (dur * self.rate) as usize;
        let start = self.slot(at, len);
        let mut phase = 0.0f32;
        for i in 0..len {
            let t = i as f32 / self.rate;
            let freq = f0 * (f1 / f0).powf(t / dur);
            phase = (phase + freq / self.rate).fract();
            let shape = match wave {
                Wave::Sine => (TAU * phase).sin(),
                Wave::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            };
            let gain = if t < ATTACK {
                FLOOR * (vol / FLOOR).powf(t / ATTACK)
            } else {
                vol * (FLOOR / vol).powf((t - ATTACK) / (dur - ATTACK))
            };
            self.out[start + i] += shape * gain;
        }
    }

    /// A burst of white noise fading out, through a band-pass filter around `freq`.
    fn noise(&mut self, at: f32, dur: f32, vol: f32, freq: f32, q: f32) {
        let len = (dur * self.rate).ceil() as usize;
        let start = self.slot(at, len);
        let mut filter = BandPass::new(freq, q, self.rate);
        // The filter rings on after the burst ends.
        let tail = len + (0.01 * self.rate) as usize;
        self.slot(at, tail);
        for i in 0..tail {
            let input = if i < len {
                self.random() * (1.0 - i as f32 / len as f32).powi(3)
            } else {
                0.0
            };
            self.out[start + i] += filter.run(input) * vol;
        }
    }

    fn random(&mut self) -> f32 {
        // xorshift: the same click every time, and no dependency for a few hundred samples.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn finish(mut self) -> Vec<f32> {
        let tail = (0.03 * self.rate) as usize;
        self.out.resize(self.out.len() + tail, 0.0);
        for s in &mut self.out {
            *s = s.clamp(-1.0, 1.0);
        }
        self.out
    }
}

/// The band-pass of Web Audio's `BiquadFilterNode`: 0 dB at the centre frequency.
struct BandPass {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x: [f32; 2],
    y: [f32; 2],
}

impl BandPass {
    fn new(freq: f32, q: f32, rate: f32) -> Self {
        let w = TAU * freq / rate;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b2: -alpha / a0,
            a1: -2.0 * w.cos() / a0,
            a2: (1.0 - alpha) / a0,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn run(&mut self, input: f32) -> f32 {
        let out = self.b0 * input + self.b2 * self.x[1] - self.a1 * self.y[0] - self.a2 * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [out, self.y[0]];
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETS: [SoundSet; 6] = [
        SoundSet::Soft,
        SoundSet::Drop,
        SoundSet::Click,
        SoundSet::Sharp,
        SoundSet::Bell,
        SoundSet::Shell,
    ];

    #[test]
    fn every_cue_is_short_audible_and_clean() {
        for set in SETS {
            for cue in [Cue::Start, Cue::Stop] {
                let wave = render(set, cue, 48_000);
                let secs = wave.len() as f32 / 48_000.0;
                assert!(
                    (0.03..0.7).contains(&secs),
                    "{set:?} {cue:?} lasts {secs} s"
                );
                let peak = wave.iter().fold(0f32, |m, s| m.max(s.abs()));
                assert!(
                    peak > 0.02 && peak <= 1.0,
                    "{set:?} {cue:?} peaks at {peak}"
                );
                assert!(wave.iter().all(|s| s.is_finite()));
                // Rings out to silence; only the click sets are meant to start sharp.
                assert!(wave.last().unwrap().abs() < 0.01);
                if matches!(set, SoundSet::Soft | SoundSet::Drop | SoundSet::Bell) {
                    assert!(wave[0].abs() < 0.01, "{set:?} {cue:?} clicks");
                }
            }
        }
    }

    #[test]
    fn soft_keeps_the_old_notes() {
        // 660 Hz first, 990 Hz after the overlap: count zero crossings in each part.
        let wave = render(SoundSet::Soft, Cue::Start, 48_000);
        let crossings = |range: std::ops::Range<usize>| {
            wave[range]
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count()
        };
        let first = crossings(480..3360) as f32 / (2880.0 / 48_000.0);
        let second = crossings(4320..7200) as f32 / (2880.0 / 48_000.0);
        assert!((first - 660.0).abs() < 40.0, "{first}");
        assert!((second - 990.0).abs() < 40.0, "{second}");
    }

    #[test]
    fn rate_changes_only_the_length() {
        let a = render(SoundSet::Drop, Cue::Start, 48_000).len() as f32 / 48_000.0;
        let b = render(SoundSet::Drop, Cue::Start, 44_100).len() as f32 / 44_100.0;
        assert!((a - b).abs() < 0.001);
    }

    #[test]
    fn names_are_lowercase_in_settings() {
        assert_eq!(
            serde_json::to_string(&SoundSet::Shell).unwrap(),
            "\"shell\""
        );
        assert_eq!(SoundSet::default(), SoundSet::Soft);
    }
}
