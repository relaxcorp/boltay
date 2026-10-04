pub(crate) fn normalize(text: &str) -> String {
    let text: String = text
        .replace("\r\n", "\n")
        .chars()
        .filter(|&c| !is_invisible(c))
        .map(|c| if c == '\r' { '\n' } else { c })
        .collect();

    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();

    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    // At most one empty line in a row.
    lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
    lines.join("\n")
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'
    )
}
