use std::path::{Path, PathBuf};

use boltay_core::audio;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn load(name: &str) -> Vec<f32> {
    audio::load(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn zero_crossings(x: &[f32]) -> usize {
    x.windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count()
}

#[test]
fn wav_at_16k_is_read_as_is() {
    let jfk = load("jfk.wav");
    assert_eq!(jfk.len(), 176_000);
    assert!(jfk.iter().all(|v| (-1.0..1.0).contains(v)));
}

#[test]
fn flac_decodes_like_wav() {
    let wav = load("tone.wav");
    assert_eq!(load("tone.flac"), wav);
    // 0.5 s of 44.1 kHz stereo becomes 8000 mono samples.
    assert!(wav.len().abs_diff(8_000) <= 1, "{}", wav.len());
}

#[test]
fn lossy_formats_decode_to_the_same_tone() {
    let wav = load("tone.wav");
    let expected_rms = rms(&wav[1_000..7_000]);
    for name in ["tone.ogg", "tone.mp3", "tone.opus", "tone.m4a"] {
        let x = load(name);
        // mp3 and aac add encoder delay and padding.
        assert!(x.len().abs_diff(wav.len()) < 2_000, "{name}: {}", x.len());
        let middle = &x[1_000..7_000];
        assert!((rms(middle) / expected_rms - 1.0).abs() < 0.05, "{name}");
        // 440 Hz over 6000 samples at 16 kHz.
        assert!(zero_crossings(middle).abs_diff(330) <= 3, "{name}");
    }
}

#[test]
fn opus_voice_message_keeps_its_length() {
    // Encoded from tts_ru_1.wav the way Telegram sends voice: Ogg Opus, mono, 24 kbit/s.
    let wav = load("tts_ru_1.wav");
    let opus = load("voice_ru_1.ogg");
    assert!(
        opus.len().abs_diff(wav.len()) <= 16,
        "{} vs {}",
        opus.len(),
        wav.len()
    );
    let ratio = rms(&opus) / rms(&wav);
    assert!((ratio - 1.0).abs() < 0.1, "{ratio}");
}

/// Ogg page checksum: CRC-32 with the 0x04c11db7 polynomial, not reflected.
fn ogg_crc(page: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in page {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn written(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("boltay-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn voice_message_without_an_end_mark_still_decodes() {
    let whole = load("voice_ru_1.ogg");
    let mut bytes = std::fs::read(fixture("voice_ru_1.ogg")).unwrap();
    let last = bytes.windows(4).rposition(|w| w == b"OggS").unwrap();
    let page = &mut bytes[last..];
    page[5] &= !0x04;
    page[22..26].fill(0);
    let crc = ogg_crc(page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    let path = written("no-eos.ogg", &bytes);
    let x = audio::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    // Without the mark the end padding is not trimmed: a few ms more at most.
    assert!(
        x.len() >= whole.len() && x.len() - whole.len() < 1_000,
        "{}",
        x.len()
    );
}

#[test]
fn voice_message_with_a_cut_last_page_keeps_the_rest() {
    let whole = load("voice_ru_1.ogg");
    let bytes = std::fs::read(fixture("voice_ru_1.ogg")).unwrap();
    let path = written("cut.ogg", &bytes[..bytes.len() - 300]);
    let x = audio::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    // The torn page goes whole: in this short file it holds about the last second.
    assert!(
        x.len() > whole.len() * 3 / 4,
        "{} of {}",
        x.len(),
        whole.len()
    );
}

#[test]
fn not_audio_is_an_error() {
    assert!(audio::load(&fixture("../../Cargo.toml")).is_err());
}
