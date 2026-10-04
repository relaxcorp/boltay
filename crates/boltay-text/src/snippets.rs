use crate::config::{Snippets, Variables};
use crate::token::fold;

/// The snippet whose trigger is the whole phrase, give or take recognition noise:
/// punctuation, case and a letter or two do not matter.
pub(crate) fn find<'a>(config: &'a Snippets, text: &str) -> Option<&'a str> {
    let phrase = normalize(text);
    if phrase.is_empty() {
        return None;
    }
    // Below one half any phrase matches some trigger and dictation stops working.
    let threshold = config.threshold.clamp(0.5, 1.0);
    config
        .entries
        .iter()
        .filter_map(|s| {
            let trigger = normalize(&s.trigger);
            if trigger.is_empty() {
                return None;
            }
            let score = similarity(&phrase, &trigger);
            (score >= threshold).then_some((score, s.text.as_str()))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, text)| text)
}

/// Fills the placeholders of a snippet: `{дата}`, `{время}`, `{буфер}`, or `{date}`,
/// `{time}`, `{clipboard}`, in any case. Anything else in braces stays as written.
pub fn fill(text: &str, vars: &Variables) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find('}') else {
            out.push_str(after);
            return out;
        };
        let value = match fold(&after[1..close]).as_str() {
            "дата" | "date" => Some(&vars.date),
            "время" | "time" => Some(&vars.time),
            "буфер" | "clipboard" => Some(&vars.clipboard),
            _ => None,
        };
        match value {
            Some(value) => out.push_str(value),
            None => out.push_str(&after[..=close]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Whether a snippet text needs the clipboard: reading it is not free on every system.
pub fn uses_clipboard(text: &str) -> bool {
    let text = fold(text);
    text.contains("{буфер}") || text.contains("{clipboard}")
}

fn normalize(text: &str) -> Vec<char> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect()
}

fn similarity(a: &[char], b: &[char]) -> f64 {
    let longest = a.len().max(b.len());
    1.0 - levenshtein(a, b) as f64 / longest as f64
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lev(a: &str, b: &str) -> usize {
        levenshtein(
            &a.chars().collect::<Vec<_>>(),
            &b.chars().collect::<Vec<_>>(),
        )
    }

    #[test]
    fn edit_distance() {
        assert_eq!(lev("", ""), 0);
        assert_eq!(lev("abc", ""), 3);
        assert_eq!(lev("kitten", "sitting"), 3);
        assert_eq!(lev("реквизиты", "реквезиты"), 1);
        assert_eq!(lev("мои", "мой"), 1);
    }
}
