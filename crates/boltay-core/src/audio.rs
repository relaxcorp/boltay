use std::fs::File;
use std::path::Path;

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoderOptions};
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::packet::Packet;

use crate::{Error, Result};

pub const SAMPLE_RATE: u32 = 16_000;
/// What a file may claim. Below it nothing is speech, and a header claiming 1 Hz would be
/// stretched 16 000 times on the way to 16 kHz: a 200 KB file would take gigabytes.
const RATES: std::ops::RangeInclusive<u32> = 4_000..=768_000;
/// The longest recording taken at once. A few megabytes of compressed silence decode to
/// days of audio, and those must not take all the memory there is.
const MAX_SECONDS: u64 = 4 * 3600;

/// Decodes an audio file into 16 kHz mono samples.
pub fn load(path: &Path) -> Result<Vec<f32>> {
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::Audio(format!("{}: no audio track", path.display())))?;
    let track_id = track.id;
    let delay = track.delay.unwrap_or(0) as usize;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| Error::Audio(format!("{}: no audio track", path.display())))?
        .clone();
    let sink = if params.codec == CODEC_ID_OPUS {
        decode_opus(format.as_mut(), track_id, &params, delay)
            .map_err(|e| Error::Audio(format!("{}: {e}", path.display())))?
    } else {
        decode(format.as_mut(), track_id, &params)?
    };
    sink.map_or(Ok(Vec::new()), Sink::finish)
}

fn decode(
    format: &mut dyn FormatReader,
    track_id: u32,
    params: &AudioCodecParameters,
) -> Result<Option<Sink>> {
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())?;
    let mut buf = Vec::new();
    let mut sink = None;
    while let Some(packet) = next_packet(format)? {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A damaged frame in a long recording should not throw away the rest of it.
            Err(DecodeError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };
        let s = decoded.spec();
        if sink.is_none() {
            sink = Some(Sink::new(s.rate(), s.channels().count())?);
        }
        buf.resize(decoded.samples_interleaved(), 0.0);
        decoded.copy_to_slice_interleaved(&mut buf);
        if let Some(sink) = &mut sink {
            sink.push(&buf)?;
        }
    }
    Ok(sink)
}

/// Opus always decodes at 48 kHz. Symphonia has no Opus decoder but reads the Ogg container,
/// Telegram voice messages among others: the encoder delay comes with the track, the end
/// padding is marked on the last packet.
fn decode_opus(
    format: &mut dyn FormatReader,
    track_id: u32,
    params: &AudioCodecParameters,
    mut delay: usize,
) -> std::result::Result<Option<Sink>, String> {
    const RATE: u32 = 48_000;
    // The longest Opus packet: 120 ms.
    const MAX_FRAMES: usize = 5_760;
    let channels = params.channels.as_ref().map_or(1, |c| c.count());
    let layout = match channels {
        1 => opus::Channels::Mono,
        2 => opus::Channels::Stereo,
        n => return Err(format!("opus with {n} channels is not supported")),
    };
    let mut decoder = opus::Decoder::new(RATE, layout).map_err(|e| e.to_string())?;
    let mut sink = Sink::new(RATE, channels).map_err(|e| e.to_string())?;
    let mut buf = vec![0.0; MAX_FRAMES * channels];
    loop {
        let packet = match next_packet(format) {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id != track_id {
            continue;
        }
        let frames = match decoder.decode_float(&packet.data, &mut buf, false) {
            Ok(frames) => frames,
            Err(e) if e.code() == opus::ErrorCode::InvalidPacket => continue,
            Err(e) => return Err(e.to_string()),
        };
        let skip = delay.min(frames);
        delay -= skip;
        let keep = frames
            .saturating_sub(packet.trim_end.get() as usize)
            .max(skip);
        sink.push(&buf[skip * channels..keep * channels])
            .map_err(|e| e.to_string())?;
    }
    Ok(Some(sink))
}

/// The next packet, `None` at the end. Some Telegram clients write voice messages without
/// the end-of-stream mark, and a file may lose its last page: the stream ends there too,
/// as in any player, instead of throwing away what was decoded.
fn next_packet(format: &mut dyn FormatReader) -> std::result::Result<Option<Packet>, DecodeError> {
    match format.next_packet() {
        Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        other => other,
    }
}

/// Resamples mono audio from `rate` to 16 kHz.
pub fn resample(samples: &[f32], rate: u32) -> Result<Vec<f32>> {
    let mut sink = Sink::new(rate, 1)?;
    sink.push(samples)?;
    sink.finish()
}

/// Takes decoded audio as it comes and keeps it as 16 kHz mono only: a long recording at
/// 48 kHz in stereo would otherwise take six times the memory before being converted.
struct Sink {
    channels: usize,
    resampler: Option<Fft<f32>>,
    /// Mono at the source rate, less than one resampler chunk.
    pending: Vec<f32>,
    out: Vec<f32>,
    /// Mono frames taken so far, at the source rate, and how many may come.
    taken: u64,
    limit: u64,
    /// The resampler's own delay, still to be cut from the start of the output.
    trim: usize,
    scratch: Vec<f32>,
}

impl Sink {
    fn new(rate: u32, channels: usize) -> Result<Self> {
        if !RATES.contains(&rate) {
            return Err(Error::Audio(format!(
                "a sample rate of {rate} Hz is not audio"
            )));
        }
        let resampler = (rate != SAMPLE_RATE)
            .then(|| {
                Fft::<f32>::new(
                    rate as usize,
                    SAMPLE_RATE as usize,
                    1024,
                    1,
                    FixedSync::Input,
                )
            })
            .transpose()
            .map_err(|e| resampling(&e))?;
        let trim = resampler.as_ref().map_or(0, |r| r.output_delay());
        let scratch = vec![0.0; resampler.as_ref().map_or(0, |r| r.output_frames_max())];
        Ok(Self {
            channels: channels.max(1),
            resampler,
            pending: Vec::new(),
            out: Vec::new(),
            taken: 0,
            limit: MAX_SECONDS * u64::from(rate),
            trim,
            scratch,
        })
    }

    fn push(&mut self, interleaved: &[f32]) -> Result<()> {
        let frames = interleaved.len() / self.channels;
        self.taken += frames as u64;
        if self.taken > self.limit {
            return Err(Error::Audio(format!(
                "the recording is longer than {} hours, split it",
                MAX_SECONDS / 3600
            )));
        }
        let mono = interleaved
            .chunks_exact(self.channels)
            .map(|frame| frame.iter().sum::<f32>() / self.channels as f32);
        if self.resampler.is_none() {
            self.out.extend(mono);
            return Ok(());
        }
        self.pending.extend(mono);
        loop {
            let resampler = self.resampler.as_ref().expect("checked above");
            if self.pending.len() < resampler.input_frames_next() {
                return Ok(());
            }
            let used = self.chunk(None)?;
            self.pending.drain(..used);
        }
    }

    /// One resampler chunk from `pending`; `partial` is how much of it is real at the end.
    fn chunk(&mut self, partial: Option<usize>) -> Result<usize> {
        let resampler = self
            .resampler
            .as_mut()
            .expect("only called when resampling");
        let needed = resampler.input_frames_next();
        if self.pending.len() < needed {
            self.pending.resize(needed, 0.0);
        }
        let input = InterleavedSlice::new(&self.pending[..], 1, self.pending.len())
            .map_err(|e| resampling(&e))?;
        let capacity = self.scratch.len();
        let mut output = InterleavedSlice::new_mut(&mut self.scratch[..], 1, capacity)
            .map_err(|e| resampling(&e))?;
        let indexing = partial.map(|n| Indexing::new().partial_len(n));
        let (used, produced) = resampler
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|e| resampling(&e))?;
        let cut = self.trim.min(produced);
        self.trim -= cut;
        self.out.extend_from_slice(&self.scratch[cut..produced]);
        Ok(used)
    }

    fn finish(mut self) -> Result<Vec<f32>> {
        let Some(resampler) = &self.resampler else {
            return Ok(self.out);
        };
        let expected = (resampler.resample_ratio() * self.taken as f64).ceil() as usize;
        let left = self.pending.len();
        if left > 0 {
            self.chunk(Some(left))?;
            self.pending.clear();
        }
        // The delay held back the end of the audio: silence pushes it out.
        while self.out.len() < expected {
            self.chunk(Some(0))?;
        }
        self.out.truncate(expected);
        Ok(self.out)
    }
}

fn resampling(e: &dyn std::fmt::Display) -> Error {
    Error::Audio(format!("resampling failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, seconds: f32) -> Vec<f32> {
        let n = (rate as f32 * seconds) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    // Counts sign changes, which for a pure tone is twice its frequency per second.
    fn zero_crossings(x: &[f32]) -> usize {
        x.windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    fn sink(rate: u32, channels: usize, pieces: &[&[f32]]) -> Result<Vec<f32>> {
        let mut sink = Sink::new(rate, channels)?;
        for piece in pieces {
            sink.push(piece)?;
        }
        sink.finish()
    }

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(
            sink(SAMPLE_RATE, 2, &[&[1.0, 0.0, 0.5, 0.5, -1.0, 1.0]]).unwrap(),
            [0.5, 0.5, 0.0]
        );
        assert_eq!(sink(SAMPLE_RATE, 1, &[&[0.1, 0.2]]).unwrap(), [0.1, 0.2]);
    }

    #[test]
    fn packets_of_any_size_give_the_same_audio() {
        let audio = tone(440.0, 48_000, 1.3);
        let whole = resample(&audio, 48_000).unwrap();
        let pieces: Vec<&[f32]> = audio.chunks(997).collect();
        assert_eq!(sink(48_000, 1, &pieces).unwrap(), whole);
    }

    #[test]
    fn stops_past_the_longest_recording() {
        let mut sink = Sink::new(8_000, 2).unwrap();
        assert_eq!(sink.limit, MAX_SECONDS * 8_000);
        sink.limit = 3;
        sink.push(&[0.0; 6]).unwrap();
        assert!(sink.push(&[0.0; 2]).is_err());
    }

    #[test]
    fn resample_keeps_duration_and_pitch() {
        for rate in [8_000, 22_050, 44_100, 48_000] {
            let out = resample(&tone(440.0, rate, 2.0), rate).unwrap();
            assert!(out.len().abs_diff(32_000) <= 1, "{rate}: {}", out.len());
            let crossings = zero_crossings(&out[1_000..31_000]);
            assert!(crossings.abs_diff(1_650) <= 2, "{rate}: {crossings}");
            let peak = out[1_000..31_000].iter().fold(0f32, |m, x| m.max(x.abs()));
            assert!((peak - 1.0).abs() < 0.02, "{rate}: {peak}");
        }
    }

    #[test]
    fn refuses_rates_no_audio_has() {
        let samples = vec![0.0; 1_000];
        for rate in [0, 1, 8, 3_999, 768_001, u32::MAX] {
            assert!(resample(&samples, rate).is_err(), "{rate}");
        }
        assert!(resample(&samples, 8_000).is_ok());
    }

    #[test]
    fn resample_is_noop_at_16k() {
        let x = tone(440.0, 16_000, 0.1);
        assert_eq!(resample(&x, 16_000).unwrap(), x);
    }

    #[test]
    fn missing_file_names_the_path() {
        let err = load(Path::new("no/such/file.wav")).unwrap_err();
        assert!(err.to_string().starts_with("no/such/file.wav: "), "{err}");
    }
}
