use serde::{Deserialize, Serialize};

/// Settings for the whole pipeline. Every level fills missing fields with defaults,
/// so a partial JSON from an older version of the settings file still loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub whitespace: bool,
    pub fillers: Fillers,
    pub commands: Commands,
    pub profanity: Profanity,
    pub dictionary: Dictionary,
    /// Brand names and tech terms, see [`Terms`].
    pub brands: Terms,
    /// Terms whose misheard forms are ordinary words too (`тонн` -> `TON`). Off by default.
    pub ambiguous: Terms,
    pub snippets: Snippets,
    pub cleanup: Cleanup,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whitespace: true,
            fillers: Fillers::default(),
            commands: Commands::default(),
            profanity: Profanity::Keep,
            dictionary: Dictionary::default(),
            brands: Terms::builtin(true, crate::brands::default_brands()),
            ambiguous: Terms::builtin(false, crate::brands::default_ambiguous()),
            snippets: Snippets::default(),
            cleanup: Cleanup::default(),
        }
    }
}

impl Config {
    /// Every stage off: `process` returns the input untouched.
    pub fn disabled() -> Self {
        Self {
            whitespace: false,
            fillers: Fillers {
                enabled: false,
                ..Fillers::default()
            },
            commands: Commands {
                enabled: false,
                ..Commands::default()
            },
            profanity: Profanity::Keep,
            dictionary: Dictionary {
                enabled: false,
                ..Dictionary::default()
            },
            brands: Terms {
                enabled: false,
                ..Config::default().brands
            },
            ambiguous: Terms {
                enabled: false,
                ..Config::default().ambiguous
            },
            snippets: Snippets {
                enabled: false,
                ..Snippets::default()
            },
            cleanup: Cleanup {
                enabled: false,
                trailing_space: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fillers {
    pub enabled: bool,
    /// Hesitation sounds: `ээ`, `ммм`, `uh`, `um`. Removed wherever they stand.
    pub hard: bool,
    /// Filler words and phrases the user switched on, e.g. `короче`, `you know`.
    /// Removed only when set off by punctuation. See [`SOFT_FILLERS`] for the stock list.
    pub soft: Vec<String>,
}

impl Default for Fillers {
    fn default() -> Self {
        Self {
            enabled: true,
            hard: true,
            soft: Vec::new(),
        }
    }
}

/// Filler words offered in settings. None of them is on by default.
pub const SOFT_FILLERS: &[&str] = &[
    "короче",
    "типа",
    "как бы",
    "ну",
    "вот",
    "значит",
    "это самое",
    "в общем",
    "так сказать",
    "like",
    "you know",
    "basically",
    "i mean",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Commands {
    pub enabled: bool,
    /// Also turn spoken `запятая`, `точка`, `comma`, `period` into marks. Off by default:
    /// the recognizer already punctuates, and the words mean other things too. Marks with
    /// unambiguous names (`двоеточие`, `открыть скобку`) work regardless.
    pub spoken_punctuation: bool,
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            enabled: true,
            spoken_punctuation: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profanity {
    #[default]
    Keep,
    Mask,
    Remove,
    Soften,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dictionary {
    pub enabled: bool,
    pub entries: Vec<Replacement>,
}

impl Default for Dictionary {
    fn default() -> Self {
        Self {
            enabled: true,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

impl Replacement {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// Names the speech model cannot spell, with the ways it misspells them: `гитхап`,
/// `githab` -> `GitHub`. Matched like the dictionary, whole words, ignoring case and `ё`,
/// plus Russian case endings after a Cyrillic variant. The name is inserted as written.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Terms {
    pub enabled: bool,
    pub entries: Vec<Term>,
    /// Every built-in name the list has had. A newer version adds only the names it
    /// brings, and a name the user deleted stays deleted.
    pub seen: Vec<String>,
    /// The same for the variants of a built-in name, as `[name, variant]`. A list saved
    /// before this field existed gets back every built-in variant its rows lack.
    pub seen_variants: Vec<(String, String)>,
}

impl Terms {
    fn builtin(enabled: bool, entries: Vec<Term>) -> Self {
        let seen = entries.iter().map(|t| t.name.clone()).collect();
        let seen_variants = variants(&entries).collect();
        Self {
            enabled,
            entries,
            seen,
            seen_variants,
        }
    }

    /// Adds the built-in terms this list has not seen yet. They go first: of two rows
    /// spelled the same, the lower one wins, and that is the row the user added. A new
    /// variant of a known name goes to the row of that name, if the user kept it.
    pub fn add_new(&mut self, builtin: Vec<Term>) {
        for (name, variant) in variants(&builtin) {
            if self
                .seen_variants
                .contains(&(name.clone(), variant.clone()))
            {
                continue;
            }
            if self.seen.contains(&name) {
                if let Some(row) = self.entries.iter_mut().find(|t| t.name == name) {
                    if !row.variants.contains(&variant) {
                        row.variants.push(variant.clone());
                    }
                }
            }
            self.seen_variants.push((name, variant));
        }
        let mut new = Vec::new();
        for term in builtin {
            if self.seen.contains(&term.name) {
                continue;
            }
            self.seen.push(term.name.clone());
            if !self.entries.iter().any(|t| t.name == term.name) {
                new.push(term);
            }
        }
        self.entries.splice(0..0, new);
    }
}

fn variants(terms: &[Term]) -> impl Iterator<Item = (String, String)> + '_ {
    terms
        .iter()
        .flat_map(|t| t.variants.iter().map(|v| (t.name.clone(), v.clone())))
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Term {
    pub name: String,
    pub variants: Vec<String>,
}

impl Term {
    pub fn new(name: impl Into<String>, variants: &[&str]) -> Self {
        Self {
            name: name.into(),
            variants: variants.iter().map(|v| v.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snippets {
    pub enabled: bool,
    /// Minimum similarity between the phrase and a trigger, 0..=1. 1 means exact.
    pub threshold: f64,
    pub entries: Vec<Snippet>,
    /// What `{дата}`, `{время}` and `{буфер}` stand for in a snippet, set by the app for
    /// each dictation. Not part of the settings.
    #[serde(skip)]
    pub variables: Variables,
}

/// Values for the placeholders in snippet texts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Variables {
    pub date: String,
    pub time: String,
    pub clipboard: String,
}

impl Default for Snippets {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.85,
            entries: Vec::new(),
            variables: Variables::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
}

impl Snippet {
    pub fn new(trigger: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cleanup {
    pub enabled: bool,
    /// Append a space so the next dictation does not stick to this one.
    pub trailing_space: bool,
}

impl Default for Cleanup {
    fn default() -> Self {
        Self {
            enabled: true,
            trailing_space: false,
        }
    }
}
