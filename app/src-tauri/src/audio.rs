use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, StreamConfig};

/// Input device names for the settings list.
pub fn microphones() -> Vec<String> {
    let host = cpal::default_host();
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    let mut names: Vec<String> = devices.map(|d| d.to_string()).collect();
    names.dedup();
    names
}

/// Software gain for quiet microphones. Peaks are clipped rather than wrapped around.
pub fn amplify(samples: &mut [f32], db: f32) {
    if db <= 0.0 {
        return;
    }
    let gain = 10f32.powf(db / 20.0);
    for s in samples {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}

/// A running microphone recording. The cpal stream is not `Send` on every platform, so it
/// lives on its own thread and this handle only talks to it.
pub struct Capture {
    shared: Arc<Shared>,
    stop: mpsc::Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
    rate: u32,
}

struct Shared {
    samples: Mutex<Vec<f32>>,
    /// Peak of the last callback, as `f32` bits.
    level: AtomicU32,
    /// Root mean square of the last callback, as `f32` bits.
    rms: AtomicU32,
}

impl Capture {
    /// Stops keeping audio after `max_secs`, whatever the device does: a driver that
    /// delivers faster than real time must not eat the memory.
    pub fn start(device: Option<&str>, max_secs: u32) -> Result<Self> {
        let shared = Arc::new(Shared {
            samples: Mutex::new(Vec::with_capacity(16_000 * 60)),
            level: AtomicU32::new(0),
            rms: AtomicU32::new(0),
        });
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let device = device.map(str::to_owned);
        let thread_shared = shared.clone();
        let thread = thread::Builder::new()
            .name("capture".into())
            .spawn(move || {
                let stream = match open(device.as_deref(), max_secs, thread_shared) {
                    Ok((stream, rate)) => {
                        let _ = ready_tx.send(Ok(rate));
                        stream
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let _ = stop_rx.recv();
                drop(stream);
            })
            .context("cannot start the capture thread")?;
        let rate = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow!("the microphone did not respond"))??;
        Ok(Self {
            shared,
            stop: stop_tx,
            thread: Some(thread),
            rate,
        })
    }

    /// Current input peak, 0..=1.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.level.load(Ordering::Relaxed))
    }

    /// Current input loudness, 0..=1.
    pub fn rms(&self) -> f32 {
        f32::from_bits(self.shared.rms.load(Ordering::Relaxed))
    }

    /// Stops the microphone and returns 16 kHz mono audio.
    pub fn finish(mut self) -> Result<Vec<f32>> {
        self.shutdown();
        let samples = std::mem::take(&mut *self.shared.samples.lock().unwrap());
        Ok(boltay_core::audio::resample(&samples, self.rate)?)
    }

    fn shutdown(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn open(device: Option<&str>, max_secs: u32, shared: Arc<Shared>) -> Result<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = match device {
        Some(name) => host
            .input_devices()?
            .find(|d| d.to_string() == name)
            // A headset that was unplugged since it was picked in settings.
            .or_else(|| host.default_input_device()),
        None => host.default_input_device(),
    }
    .context("no microphone found")?;
    let supported = device
        .default_input_config()
        .context("the microphone has no usable format")?;
    let config = supported.config();
    let rate = config.sample_rate;
    let limit = rate as usize * max_secs as usize;
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, limit, shared),
        SampleFormat::I16 => build::<i16>(&device, config, limit, shared),
        SampleFormat::U16 => build::<u16>(&device, config, limit, shared),
        SampleFormat::I32 => build::<i32>(&device, config, limit, shared),
        SampleFormat::I8 => build::<i8>(&device, config, limit, shared),
        SampleFormat::U8 => build::<u8>(&device, config, limit, shared),
        other => bail!("unsupported microphone sample format {other}"),
    }?;
    stream.play().context("cannot start the microphone")?;
    Ok((stream, rate))
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    limit: usize,
    shared: Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let mut mono = Vec::new();
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &_| {
            mono.clear();
            downmix(data, channels, &mut mono);
            shared.level.store(peak(&mono).to_bits(), Ordering::Relaxed);
            shared.rms.store(rms(&mono).to_bits(), Ordering::Relaxed);
            let mut samples = shared.samples.lock().unwrap();
            let room = limit.saturating_sub(samples.len());
            samples.extend_from_slice(&mono[..mono.len().min(room)]);
        },
        |e| log::error!("microphone stream: {e}"),
        None,
    )?;
    Ok(stream)
}

/// Averages interleaved channels into mono.
pub fn downmix<T>(data: &[T], channels: usize, out: &mut Vec<f32>)
where
    T: Sample,
    f32: FromSample<T>,
{
    let channels = channels.max(1);
    out.extend(data.chunks(channels).map(|frame| {
        frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / frame.len() as f32
    }));
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_in_decibels_with_clipping() {
        let mut samples = [0.1, -0.1, 0.6];
        amplify(&mut samples, 6.0);
        assert!((samples[0] - 0.1995).abs() < 1e-3, "{samples:?}");
        assert!((samples[1] + 0.1995).abs() < 1e-3);
        assert_eq!(samples[2], 1.0);
        let mut quiet = [0.1];
        amplify(&mut quiet, 0.0);
        assert_eq!(quiet, [0.1]);
    }

    #[test]
    fn mono_passes_through() {
        let mut out = Vec::new();
        downmix(&[0.1f32, -0.2, 0.3], 1, &mut out);
        assert_eq!(out, [0.1, -0.2, 0.3]);
    }

    #[test]
    fn stereo_is_averaged() {
        let mut out = Vec::new();
        downmix(&[1.0f32, 0.0, -0.5, -0.5, 0.2, 0.4], 2, &mut out);
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] + 0.5).abs() < 1e-6);
        assert!((out[2] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn integer_samples_are_scaled() {
        let mut out = Vec::new();
        downmix(&[i16::MAX, i16::MIN, 0], 1, &mut out);
        assert!((out[0] - 1.0).abs() < 1e-3);
        assert!((out[1] + 1.0).abs() < 1e-3);
        assert_eq!(out[2], 0.0);
    }

    #[test]
    fn unsigned_silence_is_zero() {
        let mut out = Vec::new();
        downmix(&[32768u16], 1, &mut out);
        assert!(out[0].abs() < 1e-3);
    }

    #[test]
    fn partial_frame_does_not_panic() {
        let mut out = Vec::new();
        downmix(&[0.2f32, 0.4, 0.6], 2, &mut out);
        assert_eq!(out.len(), 2);
        assert!((out[1] - 0.6).abs() < 1e-6);
    }

    #[test]
    fn rms_of_a_square_wave_is_its_amplitude() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5, 0.5, -0.5]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn peak_is_clamped() {
        assert_eq!(peak(&[]), 0.0);
        assert_eq!(peak(&[0.1, -0.7, 0.3]), 0.7);
        assert_eq!(peak(&[2.0]), 1.0);
    }
}
