pub mod audio;
mod dictation;
mod error;
mod file;
mod gigaam;
mod mel;
mod models;
mod nemo_mel;
mod onnx;
mod parakeet;
mod transducer;
mod vad;

use std::ops::Range;
use std::time::Duration;

pub use boltay_translate::{Detailed, Direction, Span, SpanKind, Translator};
pub use dictation::Dictation;
pub use error::{Error, Result};
pub use file::{transcribe, transcribe_file, FileJob, FileLanguage, Transcript};
pub use gigaam::Gigaam;
pub use models::{
    translation_model, Language, Model, ModelFile, Progress, Source, Status, Store, GIGAAM, MODELS,
    OPUS_MT, OPUS_MT_BIG, OPUS_MT_EN_RU, OPUS_MT_EN_RU_BIG, PARAKEET, SILERO_VAD,
};
pub use parakeet::Parakeet;
pub use vad::Vad;

const SECOND: usize = audio::SAMPLE_RATE as usize;
/// Both engines are trained on utterances up to about 25 s, longer input degrades and
/// costs memory quadratic in length.
const MAX_SINGLE_PASS: usize = 25 * SECOND;
const MAX_CHUNK: usize = 20 * SECOND;

#[derive(Clone, Copy, Debug, Default)]
pub struct EngineOptions {
    /// Keep an optimized copy of the encoder graph next to the model. Loading gets about
    /// twice as fast (Parakeet 4.7 s to 2.1 s from a cold disk), at the cost of as much disk
    /// space as the model itself and a slower first load with a memory peak (1.9 GB for
    /// Parakeet) while the copy is written.
    pub graph_cache: bool,
}

/// A speech recognition engine. Loads its model once and can be shared between threads.
pub trait Transcriber: Send + Sync {
    /// Recognizes 16 kHz mono audio in one pass.
    fn transcribe(&self, samples: &[f32]) -> Result<String>;
    fn language(&self) -> Language;
}

/// Speech the engine recognized in one pass, and where it lies in the recording, in
/// 16 kHz samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Runs an engine on the speech parts of a recording.
pub struct Recognizer {
    engine: Box<dyn Transcriber>,
    vad: Option<Vad>,
}

impl Recognizer {
    /// Without a VAD the engine gets the whole recording as is.
    pub fn new(engine: Box<dyn Transcriber>, vad: Option<Vad>) -> Self {
        Self { engine, vad }
    }

    pub fn language(&self) -> Language {
        self.engine.language()
    }

    pub fn vad(&self) -> Option<&Vad> {
        self.vad.as_ref()
    }

    pub fn engine(&self) -> &dyn Transcriber {
        self.engine.as_ref()
    }

    /// Recognizes 16 kHz mono audio. Silence gives an empty string without running the engine.
    pub fn recognize(&self, samples: &[f32]) -> Result<String> {
        Ok(join(&self.segments(samples, None)?, None))
    }

    /// Recognizes 16 kHz mono audio piece by piece. A pause of `split` or longer always
    /// ends a piece, even in a recording short enough for one pass.
    pub fn segments(&self, samples: &[f32], split: Option<Duration>) -> Result<Vec<Segment>> {
        let Some(vad) = &self.vad else {
            let text = self.engine.transcribe(samples)?;
            if text.is_empty() {
                return Ok(Vec::new());
            }
            return Ok(vec![Segment {
                text,
                start: 0,
                end: samples.len(),
            }]);
        };
        let split = split.map(samples_in);
        let mut out = Vec::new();
        for chunk in chunks(&vad.segments(samples)?, split) {
            let text = self.engine.transcribe(&samples[chunk.clone()])?;
            if !text.is_empty() {
                out.push(Segment {
                    text,
                    start: chunk.start,
                    end: chunk.end,
                });
            }
        }
        Ok(out)
    }
}

/// Joins recognized pieces with spaces, or with an empty line where the speaker paused for
/// `paragraph` or longer.
pub fn join(segments: &[Segment], paragraph: Option<Duration>) -> String {
    let paragraph = paragraph.map(samples_in);
    let mut text = String::new();
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            let pause = segment.start.saturating_sub(segments[i - 1].end);
            if paragraph.is_some_and(|p| pause >= p) {
                text.push_str("\n\n");
            } else {
                text.push(' ');
            }
        }
        text.push_str(&segment.text);
    }
    text
}

/// VAD pads every segment on both sides, so a pause between two is measured a little short.
fn samples_in(pause: Duration) -> usize {
    ((pause.as_secs_f64() * SECOND as f64) as usize).saturating_sub(2 * vad::PAD)
}

/// Groups speech segments into engine inputs. Anything that fits in one pass stays whole,
/// with only the leading and trailing silence cut, so the model sees full context.
/// Longer recordings are cut at pauses into chunks of up to 20 s. Pauses of `split` samples
/// or longer cut the recording first.
fn chunks(segments: &[Range<usize>], split: Option<usize>) -> Vec<Range<usize>> {
    let Some(split) = split else {
        return group(segments);
    };
    let mut out = Vec::new();
    let mut from = 0;
    for i in 1..=segments.len() {
        if i == segments.len() || segments[i].start - segments[i - 1].end >= split {
            out.extend(group(&segments[from..i]));
            from = i;
        }
    }
    out
}

fn group(segments: &[Range<usize>]) -> Vec<Range<usize>> {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return Vec::new();
    };
    let limit = if last.end - first.start <= MAX_SINGLE_PASS {
        MAX_SINGLE_PASS
    } else {
        MAX_CHUNK
    };
    let mut out = Vec::new();
    let mut cur = first.clone();
    for seg in &segments[1..] {
        if seg.end - cur.start <= limit {
            cur.end = seg.end;
        } else {
            out.push(cur);
            cur = seg.clone();
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    const S: usize = SECOND;

    #[test]
    fn no_speech_no_chunks() {
        assert!(chunks(&[], None).is_empty());
        assert!(chunks(&[], Some(S)).is_empty());
    }

    #[test]
    fn short_recording_is_one_trimmed_chunk() {
        let segments = [S..3 * S, 10 * S..12 * S, 20 * S..26 * S];
        assert_eq!(chunks(&segments, None), [S..26 * S]);
    }

    #[test]
    fn long_recording_is_cut_at_pauses() {
        let segments = [
            0..8 * S,
            9 * S..19 * S,
            21 * S..30 * S,
            31 * S..35 * S,
            36 * S..52 * S,
        ];
        assert_eq!(
            chunks(&segments, None),
            [0..19 * S, 21 * S..35 * S, 36 * S..52 * S]
        );
    }

    #[test]
    fn long_pauses_split_a_short_recording() {
        let segments = [
            S..3 * S,
            4 * S..6 * S,
            9 * S..12 * S,
            12 * S + S / 2..14 * S,
        ];
        assert_eq!(chunks(&segments, Some(2 * S)), [S..6 * S, 9 * S..14 * S]);
        assert_eq!(chunks(&segments, Some(5 * S)), [S..14 * S]);
    }

    #[test]
    fn long_pauses_split_then_the_length_limit() {
        let segments = [0..10 * S, 13 * S..30 * S, 31 * S..40 * S];
        assert_eq!(
            chunks(&segments, Some(2 * S)),
            [0..10 * S, 13 * S..30 * S, 31 * S..40 * S]
        );
    }

    fn segment(text: &str, start: usize, end: usize) -> Segment {
        Segment {
            text: text.into(),
            start,
            end,
        }
    }

    #[test]
    fn paragraphs_at_long_pauses() {
        let segments = [
            segment("Раз.", 0, S),
            segment("Два.", 3 * S, 4 * S),
            segment("Три.", 4 * S + S / 2, 5 * S),
        ];
        let pause = Some(Duration::from_secs(2));
        assert_eq!(join(&segments, pause), "Раз.\n\nДва. Три.");
        assert_eq!(join(&segments, None), "Раз. Два. Три.");
        assert_eq!(join(&[], pause), "");
    }
}
