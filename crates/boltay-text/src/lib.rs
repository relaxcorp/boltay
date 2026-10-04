mod brands;
mod cleanup;
mod commands;
mod config;
mod dictionary;
mod fillers;
mod profanity;
mod snippets;
mod token;
mod whitespace;

use std::ops::Range;

pub use brands::{default_ambiguous, default_brands};
pub use config::{
    Cleanup, Commands, Config, Dictionary, Fillers, Profanity, Replacement, Snippet, Snippets,
    Term, Terms, Variables, SOFT_FILLERS,
};
pub use snippets::uses_clipboard;

/// Replaces whole words and phrases as the dictionary stage does, for word lists that are
/// not part of the user's settings.
pub fn replace(entries: &[Replacement], text: &str) -> String {
    dictionary::apply(&dictionary::user_rules(entries), text)
}

/// A replacement made by [`replace_spans`], positions in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// Index of the entry that matched.
    pub entry: usize,
    /// The replaced words in the input.
    pub from: Range<usize>,
    /// What took their place in the output.
    pub to: Range<usize>,
}

/// Same as [`replace`], also telling what was replaced where.
pub fn replace_spans(entries: &[Replacement], text: &str) -> (String, Vec<Replaced>) {
    let (out, hits) = dictionary::apply_spans(&dictionary::user_rules(entries), text);
    let replaced = hits
        .into_iter()
        .map(|h| Replaced {
            entry: h.rule,
            from: h.from,
            to: h.to,
        })
        .collect();
    (out, replaced)
}

/// Runs recognized text through the enabled stages, in a fixed order.
pub fn process(config: &Config, input: &str) -> String {
    process_detailed(config, input).text
}

/// Result of [`process_detailed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processed {
    pub text: String,
    /// The phrase matched a snippet and `text` is the user's saved text.
    pub snippet: bool,
}

/// Same as [`process`], also telling whether a snippet replaced the phrase.
pub fn process_detailed(config: &Config, input: &str) -> Processed {
    let mut text = input.to_string();
    if config.whitespace {
        text = whitespace::normalize(&text);
    }
    if config.fillers.enabled {
        text = fillers::apply(&config.fillers, &text);
    }
    if config.commands.enabled {
        text = commands::apply(&config.commands, &text);
    }
    if config.profanity != Profanity::Keep {
        text = profanity::apply(config.profanity, &text);
    }
    if config.brands.enabled {
        text = brands::split_yandex(&text);
    }
    // The user's dictionary and the brand names in one pass, so neither rewrites what
    // the other put in.
    let said = text.clone();
    let rules = dictionary::rules(config);
    if !rules.is_empty() {
        text = dictionary::apply(&rules, &text);
    }
    // A snippet is the user's own text: it goes out verbatim, cleanup would only mangle it.
    // Its trigger may spell a brand either way, `мой гитхаб` or `мой GitHub`.
    let snippet = config
        .snippets
        .enabled
        .then(|| {
            snippets::find(&config.snippets, &text)
                .or_else(|| snippets::find(&config.snippets, &said))
        })
        .flatten();
    if let Some(snippet) = &snippet {
        text = snippets::fill(snippet, &config.snippets.variables);
    } else if config.cleanup.enabled {
        text = cleanup::apply(&text);
    }
    if config.cleanup.trailing_space && !text.is_empty() && !text.ends_with(char::is_whitespace) {
        text.push(' ');
    }
    Processed {
        text,
        snippet: snippet.is_some(),
    }
}
