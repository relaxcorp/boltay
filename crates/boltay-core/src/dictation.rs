use std::time::Duration;

use boltay_text::{Config, Profanity, Terms};
use boltay_translate::Translator;

use crate::{join, Error, Language, Recognizer, Result};

/// Speech to the text that gets inserted: recognition, the text rules and, on request,
/// translation to English with the profanity filter run once more over the English text.
pub struct Dictation {
    pub recognizer: Recognizer,
    pub text: Config,
    pub translator: Option<Translator>,
    /// A pause this long starts a new paragraph.
    pub paragraph_pause: Option<Duration>,
}

impl Dictation {
    pub fn run(&self, samples: &[f32], translate: bool) -> Result<String> {
        let recognized = self.recognize(samples)?;
        self.finish(&recognized, translate)
    }

    /// The raw recognized text, with paragraphs where the speaker paused.
    pub fn recognize(&self, samples: &[f32]) -> Result<String> {
        let segments = self.recognizer.segments(samples, self.paragraph_pause)?;
        Ok(join(&segments, self.paragraph_pause))
    }

    /// Everything after recognition.
    pub fn finish(&self, recognized: &str, translate: bool) -> Result<String> {
        if !translate {
            if self.recognizer.language() == Language::Russian {
                return Ok(boltay_text::process(&self.text, recognized));
            }
            // The brand lists are how the Russian model misspells names. In other languages
            // they would only catch plain words: `face it` -> `Faceit`.
            let config = Config {
                brands: Terms::default(),
                ambiguous: Terms::default(),
                ..self.text.clone()
            };
            return Ok(boltay_text::process(&config, recognized));
        }
        let translator = self
            .translator
            .as_ref()
            .ok_or_else(|| Error::Model("translation model is not loaded".into()))?;
        finish_translated(&self.text, recognized, |text| {
            Ok(translator.translate(&boltay_translate::prepare(text))?)
        })
    }
}

fn finish_translated(
    config: &Config,
    recognized: &str,
    translate: impl FnOnce(&str) -> Result<String>,
) -> Result<String> {
    // The trailing space is for the text as inserted, the model would drop it.
    let mut before = config.clone();
    before.cleanup.trailing_space = false;
    let russian = boltay_text::process_detailed(&before, recognized);
    // A snippet is the user's own text, inserted exactly as saved.
    if russian.snippet {
        return Ok(boltay_text::process(config, recognized));
    }
    // Silence, or a spoken line break and nothing else: nothing to translate.
    if russian.text.trim().is_empty() {
        return Ok(russian.text);
    }
    let mut english = translate(&russian.text)?;
    if config.profanity != Profanity::Keep {
        let filter = Config {
            profanity: config.profanity,
            ..Config::disabled()
        };
        english = boltay_text::process(&filter, &english);
    }
    if config.cleanup.enabled
        && config.cleanup.trailing_space
        && !english.is_empty()
        && !english.ends_with(char::is_whitespace)
    {
        english.push(' ');
    }
    Ok(english)
}

#[cfg(test)]
mod tests {
    use boltay_text::Snippet;

    use super::*;
    use crate::Transcriber;

    fn echo(text: &str) -> Result<String> {
        Ok(format!("<{text}>"))
    }

    struct Silent(Language);

    impl Transcriber for Silent {
        fn transcribe(&self, _: &[f32]) -> Result<String> {
            Ok(String::new())
        }

        fn language(&self) -> Language {
            self.0
        }
    }

    fn dictation(language: Language) -> Dictation {
        Dictation {
            recognizer: Recognizer::new(Box::new(Silent(language)), None),
            text: Config::default(),
            translator: None,
            paragraph_pause: None,
        }
    }

    #[test]
    fn brand_names_are_fixed_in_russian_speech_only() {
        let russian = dictation(Language::Russian);
        assert_eq!(
            russian.finish("код на гитхабе", false).unwrap(),
            "Код на GitHub"
        );
        let other = dictation(Language::Other);
        let english = "Let's face it, every facet of the back end is broken.";
        assert_eq!(other.finish(english, false).unwrap(), english);
    }

    #[test]
    fn text_rules_run_before_translation() {
        let out = finish_translated(&Config::default(), "ээ  привет ,  мир", echo).unwrap();
        assert_eq!(out, "<Привет, мир>");
    }

    #[test]
    fn brand_names_reach_the_translator() {
        let out = finish_translated(&Config::default(), "код на гитхабе", echo).unwrap();
        assert_eq!(out, "<Код на GitHub>");
    }

    #[test]
    fn profanity_filter_runs_on_english() {
        let config = Config {
            profanity: Profanity::Mask,
            ..Config::default()
        };
        let out = finish_translated(&config, "ну", |_| Ok("What the fuck is this".into())).unwrap();
        assert!(!out.contains("fuck"), "{out}");

        let keep = finish_translated(&Config::default(), "ну", |_| Ok("fuck".into())).unwrap();
        assert_eq!(keep, "fuck");
    }

    #[test]
    fn trailing_space_survives_translation() {
        let mut config = Config::default();
        config.cleanup.trailing_space = true;
        let out = finish_translated(&config, "привет", |t| {
            assert!(!t.ends_with(' '));
            Ok("Hi".into())
        })
        .unwrap();
        assert_eq!(out, "Hi ");
    }

    #[test]
    fn no_trailing_space_after_a_line_break() {
        let mut config = Config::default();
        config.cleanup.trailing_space = true;
        let out = finish_translated(&config, "Готово. Новая строка.", |t| {
            assert_eq!(t, "Готово.\n");
            Ok("Done.\n".into())
        })
        .unwrap();
        assert_eq!(out, "Done.\n");
    }

    #[test]
    fn snippets_are_inserted_untranslated() {
        let mut config = Config::default();
        config.snippets.entries = vec![Snippet::new("мои реквизиты", "ИНН 7700000000")];
        config.cleanup.trailing_space = true;
        let out = finish_translated(&config, "Мои реквизиты.", |_| {
            panic!("translated")
        })
        .unwrap();
        assert_eq!(out, "ИНН 7700000000 ");
    }

    #[test]
    fn a_lone_line_break_is_not_translated() {
        let out = finish_translated(&Config::default(), "Абзац.", |_| panic!("called")).unwrap();
        assert_eq!(out, "\n\n");
    }

    #[test]
    fn silence_is_not_translated() {
        let out = finish_translated(&Config::default(), "  ", |_| panic!("called")).unwrap();
        assert_eq!(out, "");
    }
}
