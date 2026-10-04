use crate::config::Fillers;
use crate::token::{self, cut, fold, match_phrase, rewind, Edge, Kind, Token};

pub(crate) fn apply(config: &Fillers, text: &str) -> String {
    let mut soft: Vec<String> = config
        .soft
        .iter()
        .map(|p| fold(p).split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect();
    // Longest first, so `как бы` wins over a user-added `как`.
    soft.sort_by_key(|p| std::cmp::Reverse(p.split(' ').count()));

    let mut tokens = token::tokenize(text);
    // Removing one kind can set off a word of the other kind: `Ээ ну, я` -> `Ну, я` -> `Я`.
    loop {
        let mut changed = false;
        let mut from = 0;
        while let Some((start, end)) = find_soft(&tokens, &soft, config.hard, from) {
            from = rewind(cut(&mut tokens, start, end));
            changed = true;
        }
        from = 0;
        while let Some(i) = config.hard.then(|| find_hard(&tokens, from)).flatten() {
            from = rewind(cut(&mut tokens, i, i + 1));
            changed = true;
        }
        if !changed {
            break;
        }
    }
    token::tidy(&mut tokens);
    // Only pauses make line breaks this early, and one left at an edge belonged to a line
    // that was cut out whole: `Эээ.⏎⏎Привет.` -> `Привет.`
    let blank = |t: &Token| t.is(Kind::Space) || t.is(Kind::Newline);
    while tokens.last().is_some_and(blank) {
        tokens.pop();
    }
    let leading = tokens.iter().take_while(|t| blank(t)).count();
    tokens.drain(..leading);
    token::render(&tokens)
}

fn find_hard(tokens: &[Token], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|&i| tokens[i].is(Kind::Word) && is_hesitation(&tokens[i].text))
}

/// `ээ`, `эмм`, `ммм`, `э-э-э`, `uh`, `umm`, `erm`. Not `мм` (millimetres), not `err`,
/// and nothing in capitals: `АА` batteries and the `ER` are not hesitations.
pub(crate) fn is_hesitation(word: &str) -> bool {
    let letters = word.chars().filter(|c| c.is_alphabetic()).count();
    if letters > 1 && word.chars().all(|c| !c.is_lowercase()) {
        return false;
    }
    let word = fold(word);
    if word.contains('-') {
        let joined: String = word.split('-').collect();
        return word.split('-').count() > 1
            && (runs(&joined, &['э'])
                || runs(&joined, &['э', 'м'])
                || runs(&joined, &['м'])
                || runs(&joined, &['а']));
    }
    let w = word.as_str();
    (runs(w, &['э']) && w.chars().count() >= 2)
        || runs(w, &['э', 'м'])
        || (runs(w, &['м']) && w.chars().count() >= 3)
        || (runs(w, &['а']) && w.chars().count() >= 2)
        || runs(w, &['u', 'h'])
        || runs(w, &['u', 'm'])
        || runs(w, &['u', 'h', 'm'])
        || (runs(w, &['h', 'm']) && w.chars().filter(|&c| c == 'm').count() >= 2)
        || (runs(w, &['m']) && w.chars().count() >= 3)
        || w == "er"
        || w == "erm"
}

/// Whether `word` is exactly the letters of `pattern` in order, each repeated one or more times.
fn runs(word: &str, pattern: &[char]) -> bool {
    let mut chars = word.chars().peekable();
    for &p in pattern {
        if chars.next() != Some(p) {
            return false;
        }
        while chars.peek() == Some(&p) {
            chars.next();
        }
    }
    chars.next().is_none()
}

/// A soft filler goes only when punctuation sets it off on both sides. A following `?` or
/// `!` does not count: `Ну?` and `Вот!` mean something.
///
/// Fillers next to each other, with or without commas between them, are judged and cut as
/// one span: `знаю, это самое, короче, капец`. Cutting them one by one would strip the
/// commas around the first and leave the second glued to a word.
fn find_soft(
    tokens: &[Token],
    phrases: &[String],
    hard: bool,
    from: usize,
) -> Option<(usize, usize)> {
    if phrases.is_empty() {
        return None;
    }
    for i in from..tokens.len() {
        let Some((mut end, mut soft)) = unit_at(tokens, i, phrases, hard) else {
            continue;
        };
        if !token::left_edge(tokens, i).is_boundary() {
            continue;
        }
        // Every place the span could stop, longest last. A span of hesitations alone is
        // left to the hard pass.
        let mut ends = Vec::new();
        loop {
            if soft {
                ends.push(end);
            }
            match link(tokens, end, phrases, hard) {
                Some((next, next_soft)) => {
                    end = next;
                    soft |= next_soft;
                }
                None => break,
            }
        }
        if let Some(&end) = ends.iter().rev().find(|&&e| closes_off(tokens, e)) {
            return Some((i, end));
        }
    }
    None
}

/// A filler starting at `i`: its end and whether it is a soft one.
fn unit_at(tokens: &[Token], i: usize, phrases: &[String], hard: bool) -> Option<(usize, bool)> {
    if let Some(end) = match_any(tokens, i, phrases) {
        return Some((end, true));
    }
    let token = tokens.get(i)?;
    (hard && token.is(Kind::Word) && is_hesitation(&token.text)).then_some((i + 1, false))
}

/// The next filler after `end`, separated by a space or a comma.
fn link(tokens: &[Token], end: usize, phrases: &[String], hard: bool) -> Option<(usize, bool)> {
    if tokens.get(end)?.is(Kind::Space) {
        if let Some(unit) = unit_at(tokens, end + 1, phrases, hard) {
            return Some(unit);
        }
    }
    let comma = token::next_solid(tokens, end)?;
    if tokens[comma].text != "," {
        return None;
    }
    unit_at(tokens, token::next_solid(tokens, comma + 1)?, phrases, hard)
}

fn match_any(tokens: &[Token], start: usize, phrases: &[String]) -> Option<usize> {
    phrases.iter().find_map(|p| match_phrase(tokens, start, p))
}

fn closes_off(tokens: &[Token], end: usize) -> bool {
    let next = token::next_solid(tokens, end);
    match token::right_edge(tokens, end) {
        Edge::Word | Edge::Other => false,
        Edge::Terminal => !matches!(token::punct_at(tokens, next), Some('?' | '!')),
        // `Это самое, что у меня есть`: a comma before a subordinate clause is grammar,
        // not a pause around a filler.
        Edge::Pause => !next
            .and_then(|j| token::next_solid(tokens, j + 1))
            .is_some_and(|j| tokens[j].is(Kind::Word) && is_conjunction(&tokens[j].text)),
        Edge::Line | Edge::Stop | Edge::Bracket => true,
    }
}

fn is_conjunction(word: &str) -> bool {
    let word = fold(word);
    matches!(
        word.as_str(),
        "что"
            | "чтобы"
            | "чем"
            | "где"
            | "куда"
            | "откуда"
            | "ли"
            | "если"
            | "that"
            | "which"
            | "who"
            | "whom"
            | "whose"
    ) || word.starts_with("котор")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hesitations() {
        for w in [
            "ээ", "Эээ", "эм", "эммм", "ммм", "аа", "э-э", "э-э-э", "м-м", "uh", "Umm", "uhm",
            "hmm", "er", "erm", "mmm",
        ] {
            assert!(is_hesitation(w), "{w}");
        }
    }

    #[test]
    fn not_hesitations() {
        for w in [
            "э",
            "а",
            "м",
            "мм",
            "мама",
            "эмаль",
            "err",
            "hm",
            "um-hm-yes",
            "АА",
            "ЭЭ",
            "ER",
            "UM",
            "umbrella",
            "ah",
        ] {
            assert!(!is_hesitation(w), "{w}");
        }
    }
}
