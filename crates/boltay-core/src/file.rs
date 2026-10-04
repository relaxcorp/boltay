use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    audio, chunks, join, samples_in, Error, Language, Result, Segment, Transcriber, Vad, SECOND,
};

/// Which engine recognizes a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLanguage {
    /// The multilingual engine listens first; Russian speech goes to GigaAM.
    Auto,
    Russian,
    English,
}

/// A recognized file. Segment positions are in 16 kHz samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub text: String,
    pub language: Language,
    pub segments: Vec<Segment>,
}

/// What a file transcription runs with.
pub struct FileJob<'a> {
    /// Without a VAD the engine gets the whole recording in one pass.
    pub vad: Option<&'a Vad>,
    /// Hands out the engine for a language. Detection asks for the multilingual one first,
    /// lets go of it and asks again for it or, when the speech turns out Russian, for
    /// GigaAM: keep the last one loaded and free it before loading the other.
    pub engine: &'a mut dyn FnMut(Language) -> Result<Arc<dyn Transcriber>>,
    /// A pause this long starts a new paragraph.
    pub paragraph: Option<Duration>,
    /// Share of the work done, from 0 to 1.
    pub progress: &'a mut dyn FnMut(f32),
    pub cancel: &'a AtomicBool,
}

/// How much speech the multilingual engine hears before the language is decided.
const DETECT: usize = 20 * SECOND;

pub fn transcribe_file(path: &Path, language: FileLanguage, job: FileJob) -> Result<Transcript> {
    let samples = audio::load(path)?;
    transcribe(&samples, language, job)
}

/// Recognizes 16 kHz mono audio piece by piece, cut at pauses.
pub fn transcribe(samples: &[f32], language: FileLanguage, job: FileJob) -> Result<Transcript> {
    let FileJob {
        vad,
        engine,
        paragraph,
        progress,
        cancel,
    } = job;
    let pieces = match vad {
        Some(vad) => chunks(&vad.segments(samples)?, paragraph.map(samples_in)),
        None if samples.is_empty() => Vec::new(),
        None => std::iter::once(0..samples.len()).collect(),
    };
    let speech: usize = pieces.iter().map(Range::len).sum();
    let mut work = Work {
        done: 0,
        total: speech,
        progress,
        cancel,
    };

    let mut segments = Vec::new();
    let mut next = 0;
    let language = match language {
        FileLanguage::Russian => Language::Russian,
        FileLanguage::English => Language::Other,
        FileLanguage::Auto => {
            let multilingual = engine(Language::Other)?;
            let mut heard = 0;
            while next < pieces.len() && (heard < DETECT || letters(&segments).0 == 0) {
                let piece = &pieces[next];
                segments.extend(work.recognize(multilingual.as_ref(), samples, piece)?);
                heard += piece.len();
                next += 1;
            }
            drop(multilingual);
            let (all, cyrillic) = letters(&segments);
            if cyrillic * 2 > all {
                // What was heard so far gets recognized again.
                work.total += heard;
                segments.clear();
                next = 0;
                Language::Russian
            } else {
                Language::Other
            }
        }
    };
    let engine = engine(language)?;
    for piece in &pieces[next..] {
        segments.extend(work.recognize(engine.as_ref(), samples, piece)?);
    }
    (work.progress)(1.0);
    Ok(Transcript {
        text: join(&segments, paragraph),
        language,
        segments,
    })
}

struct Work<'a> {
    done: usize,
    total: usize,
    progress: &'a mut dyn FnMut(f32),
    cancel: &'a AtomicBool,
}

impl Work<'_> {
    fn recognize(
        &mut self,
        engine: &dyn Transcriber,
        samples: &[f32],
        piece: &Range<usize>,
    ) -> Result<Option<Segment>> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let text = engine.transcribe(&samples[piece.clone()])?;
        self.done += piece.len();
        (self.progress)(self.done as f32 / self.total.max(1) as f32);
        Ok((!text.is_empty()).then_some(Segment {
            text,
            start: piece.start,
            end: piece.end,
        }))
    }
}

/// Letters in the recognized text, and how many of them are Cyrillic.
fn letters(segments: &[Segment]) -> (usize, usize) {
    let mut all = 0;
    let mut cyrillic = 0;
    for c in segments.iter().flat_map(|s| s.text.chars()) {
        if c.is_alphabetic() {
            all += 1;
            if matches!(c, '\u{400}'..='\u{4ff}') {
                cyrillic += 1;
            }
        }
    }
    (all, cyrillic)
}
