use std::cmp::Reverse;
use std::collections::HashSet;
use std::ops::Range;

use crate::config::{Config, Replacement, Terms};
use crate::token::{capitalize, fold, is_opening, is_word_char, sentence_case};

/// One replacement. Brand names differ from the user's own entries in two ways: Russian
/// case endings are accepted after a Cyrillic variant (`на гитхабе` -> `на GitHub`), and
/// the name keeps the case it is written in, `pull request` stays lowercase mid-sentence.
pub(crate) struct Rule<'a> {
    from: &'a str,
    to: &'a str,
    brand: bool,
}

pub(crate) fn user_rules(entries: &[Replacement]) -> Vec<Rule<'_>> {
    entries
        .iter()
        .map(|e| Rule {
            from: &e.from,
            to: &e.to,
            brand: false,
        })
        .collect()
}

/// The enabled word lists in priority order: the user's entries win over the built-in
/// ones when both match the same words.
pub(crate) fn rules(config: &Config) -> Vec<Rule<'_>> {
    let mut rules = Vec::new();
    if config.dictionary.enabled {
        rules.extend(user_rules(&config.dictionary.entries));
    }
    // A word in the user's dictionary is theirs in every form: `в телеграме` stays as said
    // instead of becoming a brand the user spells their own way.
    let own: HashSet<Vec<char>> = rules.iter().map(|r| pattern(r.from)).collect();
    for terms in [&config.brands, &config.ambiguous] {
        if terms.enabled {
            rules.extend(term_rules(terms).filter(|r| !own.contains(&pattern(r.from))));
        }
    }
    rules
}

fn term_rules(terms: &Terms) -> impl Iterator<Item = Rule<'_>> {
    // Rows the user adds go to the end of the list and win over built-in ones spelled the
    // same. A name still being typed would turn its variants into nothing.
    let named = terms
        .entries
        .iter()
        .rev()
        .filter(|t| !t.name.trim().is_empty());
    named.flat_map(|term| {
        let name = term.name.trim();
        // The name itself too: `github` and `Github` come out as `GitHub`.
        std::iter::once(name)
            .chain(term.variants.iter().map(String::as_str))
            .map(move |from| Rule {
                from,
                to: name,
                brand: true,
            })
    })
}

/// Case endings a Russian noun borrowed from English takes: `гитхаба`, `докером`.
const ENDINGS: &[&str] = &[
    "ами", "ом", "ов", "ам", "ах", "ой", "ем", "а", "у", "е", "ы", "ю", "я", "и",
];
/// Endings that replace a final `й` or `ь`: `спотифай` -> `спотифае`, `эксель` -> `экселе`.
const ENDINGS_SOFT: &[&str] = &["ями", "ем", "ей", "ям", "ях", "ью", "е", "я", "ю", "и"];
/// Endings that replace a final `а` or `я`: `фигма` -> `фигме`, `джира` -> `джирой`.
const ENDINGS_A: &[&str] = &[
    "ами", "ями", "ой", "ей", "ою", "ею", "ам", "ям", "ах", "ях", "ы", "и", "е", "у", "ю",
];

/// Replaces whole words and phrases, ignoring case, `ё`, and whether the recognizer
/// wrote a space or a hyphen between words: `гит хаб`, `Гит-Хаб` -> `GitHub`. Replaced
/// text is not scanned again.
pub(crate) fn apply(rules: &[Rule], text: &str) -> String {
    apply_spans(rules, text).0
}

/// A replacement [`apply_spans`] made: the index of the rule, the replaced bytes of the
/// input and the bytes of the output that took their place.
pub(crate) struct Hit {
    pub rule: usize,
    pub from: Range<usize>,
    pub to: Range<usize>,
}

pub(crate) fn apply_spans(rules: &[Rule], text: &str) -> (String, Vec<Hit>) {
    let mut patterns: Vec<(Vec<char>, usize, &Rule)> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| (pattern(r.from), i, r))
        .filter(|(p, ..)| !p.is_empty())
        .collect();
    // Longest first: `пулл реквест` must win over a separate entry for `пулл`. The sort is
    // stable, so between equal patterns the earlier list wins.
    patterns.sort_by_key(|(p, ..)| Reverse(p.len()));
    if patterns.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let chars: Vec<char> = text.chars().collect();
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain([text.len()])
        .collect();
    let folded: Vec<char> = chars.iter().map(|&c| fold_char(c)).collect();
    let mut out = String::with_capacity(text.len());
    let mut hits = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i == 0 || !is_word_char(chars[i - 1]) {
            let hit = patterns.iter().find_map(|(p, index, rule)| {
                match_at(&chars, &folded, i, p, rule.brand).map(|m| (m, *index, *rule))
            });
            let hit = hit.filter(|((end, _), _, rule)| !rule.brand || !in_address(&chars, i, *end));
            if let Some(((end, ending), index, rule)) = hit {
                let start = out.len();
                out.push_str(&cased(rule, &chars, i, end, &folded[ending..end]));
                hits.push(Hit {
                    rule: index,
                    from: bytes[i]..bytes[end],
                    to: start..out.len(),
                });
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, hits)
}

fn cased(rule: &Rule, chars: &[char], i: usize, end: usize, ending: &[char]) -> String {
    if rule.brand {
        let name = inflect(rule.to, ending);
        // A lowercase name keeps the capital of a Russian word that opens a quote, a
        // bracket, a reply or a line: `Ответил: «Окей, жду»`, `— Окей, — сказал он`.
        // Anywhere else a capital is the recognizer's guess: `мой Пулл реквест` -> `мой pull request`.
        let opens = chars[..i]
            .iter()
            .rev()
            .find(|&&c| c != ' ')
            .is_some_and(|&c| matches!(c, '«' | '“' | '„' | '"' | '(' | '—' | '\n'));
        let kept = opens
            && rule.to.chars().all(|c| !c.is_uppercase())
            && matches!(chars[i], 'А'..='Я' | 'Ё');
        return if kept || sentence_start(chars, i) {
            sentence_case(&name)
        } else {
            name
        };
    }
    // `Гит хаб` opening a sentence stays capitalized: `Пулл реквест` -> `Pull request`.
    // An abbreviation is written in capitals anywhere, so it only counts at the start.
    let abbreviation = chars[i..end]
        .iter()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase());
    if chars[i].is_uppercase() && (!abbreviation || sentence_start(chars, i)) {
        capitalize(rule.to)
    } else {
        rule.to.to_string()
    }
}

/// A name written in Cyrillic keeps the ending it was said with, `на яндекс маркете` ->
/// `на Яндекс Маркете`. A Latin one cannot take it: `на гитхабе` -> `на GitHub`. Only a
/// plain lowercase term shows the plural the English way, `пулл реквесты` -> `pull
/// requests`; a brand like `GitHub` has none.
fn inflect(name: &str, ending: &[char]) -> String {
    if ending.is_empty() {
        return name.to_string();
    }
    let ending: String = ending.iter().collect();
    let mut chars = name.chars();
    match chars.next_back() {
        Some('й' | 'ь' | 'а' | 'я') => format!("{}{ending}", chars.as_str()),
        Some(c) if is_cyrillic_consonant(c) => format!("{name}{ending}"),
        Some(c)
            if c.is_ascii_lowercase()
                && !matches!(c, 's' | 'z' | 'x')
                && name.chars().all(|c| !c.is_uppercase())
                && PLURAL.contains(&ending.as_str()) =>
        {
            format!("{name}s")
        }
        _ => name.to_string(),
    }
}

/// Endings that only a plural takes after a hard consonant: `реквесты`, `реквестов`.
const PLURAL: &[&str] = &["ы", "и", "ов", "ам", "ами", "ах"];

/// `github.com/user`, `steam@mail.ru`, `C:\github\x`, `site.ru/?ref=telegram`: part of an
/// address or a path, spelled as is. So is a Latin compound, `docker-compose`, while
/// `гитхаб-репозиторий` is still a brand.
fn in_address(chars: &[char], start: usize, end: usize) -> bool {
    let from = chars[..start]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |p| p + 1);
    let to = chars[end..]
        .iter()
        .position(|c| c.is_whitespace())
        .map_or(chars.len(), |p| end + p);
    let word = &chars[from..to];
    let dot_inside = |k: usize| {
        k > 0
            && word[k - 1].is_alphanumeric()
            && word.get(k + 1).is_some_and(|c| c.is_alphanumeric())
    };
    let latin = |j: Option<usize>| {
        j.and_then(|j| chars.get(j))
            .is_some_and(char::is_ascii_alphabetic)
    };
    word.iter()
        .enumerate()
        .any(|(k, &c)| matches!(c, '@' | '/' | '\\') || (c == '.' && dot_inside(k)))
        || (chars.get(end) == Some(&'-') && latin(Some(end + 1)))
        || (start >= 2 && chars[start - 1] == '-' && latin(start.checked_sub(2)))
}

/// Whether the word at `i` opens a sentence, looking through opening quotes and brackets.
/// After `…` or an abbreviation like `т.е.` the sentence may go on: there the word keeps
/// the case the recognizer gave it.
fn sentence_start(chars: &[char], i: usize) -> bool {
    let mut j = i;
    while j > 0 && (chars[j - 1].is_whitespace() || is_opening(Some(chars[j - 1]))) {
        j -= 1;
    }
    let before = &chars[..j];
    match before.last() {
        None | Some('!' | '?') => true,
        Some('…') => chars[i].is_uppercase(),
        Some('.') if before.ends_with(&['.', '.']) || is_abbreviation(before) => {
            chars[i].is_uppercase()
        }
        Some('.') => true,
        _ => false,
    }
}

/// Abbreviations that end with a dot in the middle of a sentence.
const ABBREVIATIONS: &[&str] = &[
    "т.е.",
    "т.к.",
    "т.д.",
    "т.п.",
    "т.н.",
    "см.",
    "напр.",
    "др.",
    "пр.",
    "e.g.",
    "i.e.",
    "etc.",
    "vs.",
];

fn is_abbreviation(before: &[char]) -> bool {
    let from = before
        .iter()
        .rposition(|&c| c.is_whitespace() || is_opening(Some(c)))
        .map_or(0, |p| p + 1);
    let word: String = before[from..]
        .iter()
        .flat_map(|c| c.to_lowercase())
        .collect();
    ABBREVIATIONS.contains(&word.as_str())
}

fn pattern(from: &str) -> Vec<char> {
    fold(from)
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect()
}

fn fold_char(c: char) -> char {
    match c.to_lowercase().next().unwrap_or(c) {
        'ё' => 'е',
        '-' => ' ',
        c if c.is_whitespace() && c != '\n' => ' ',
        c => c,
    }
}

/// Where the match ends and where its case ending starts, the same place when it has none.
fn match_at(
    chars: &[char],
    folded: &[char],
    start: usize,
    pattern: &[char],
    inflects: bool,
) -> Option<(usize, usize)> {
    let at_word_end = |j: usize| chars.get(j).is_none_or(|&c| !is_word_char(c));
    let exact = match_chars(folded, start, pattern);
    if let Some(end) = exact.filter(|&end| at_word_end(end)) {
        return Some((end, end));
    }
    if !inflects {
        return None;
    }
    let (stem, endings) = match pattern.split_last()? {
        (&('й' | 'ь'), stem) => (stem, ENDINGS_SOFT),
        (&('а' | 'я'), stem) => (stem, ENDINGS_A),
        (&last, _) if is_cyrillic_consonant(last) => (pattern, ENDINGS),
        _ => return None,
    };
    let stem_end = match_chars(folded, start, stem)?;
    endings.iter().find_map(|ending| {
        let ending: Vec<char> = ending.chars().collect();
        let end = match_chars(folded, stem_end, &ending)?;
        at_word_end(end).then_some((end, stem_end))
    })
}

/// Matches `pattern` at `start`; a space in it stands for any run of spaces.
fn match_chars(folded: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    let mut j = start;
    for &p in pattern {
        if *folded.get(j)? != p {
            return None;
        }
        j += 1;
        if p == ' ' {
            while folded.get(j) == Some(&' ') {
                j += 1;
            }
        }
    }
    Some(j)
}

fn is_cyrillic_consonant(c: char) -> bool {
    ('б'..='я').contains(&c) && !"аеиоуыэюяйьъ".contains(c)
}
