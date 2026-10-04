use crate::token::{self, Kind, Token};

pub(crate) fn apply(text: &str) -> String {
    let text = number_sign(&space_before_currency(text));
    let mut tokens = token::tokenize(&text);
    drop_stray_marks(&mut tokens);
    space_after_commas(&mut tokens);
    token::tidy(&mut tokens);
    at_most_one_empty_line(&mut tokens);
    capitalize_lines(&mut tokens);
    token::render(&tokens)
}

/// The recognizer glues the amount to the word before it: `перевёл$200`.
fn space_before_currency(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    let mut prev = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        // After a Russian word only: `US$200` and `HK$300` are written glued.
        let glued = matches!(c, '$' | '€' | '£')
            && prev.is_some_and(is_russian)
            && chars.peek().is_some_and(char::is_ascii_digit);
        if glued {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// The recognizer has no `№` and writes `Кабинет No 312`. After an English word `No 5` is
/// left as it is.
fn number_sign(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("No ") {
        let (head, tail) = rest.split_at(at);
        let after = &tail[3..];
        let russian = head.ends_with(' ') && head.trim_end().chars().last().is_some_and(is_russian);
        out.push_str(head);
        out.push_str(
            if russian && after.starts_with(|c: char| c.is_ascii_digit()) {
                "№ "
            } else {
                "No "
            },
        );
        rest = after;
    }
    out.push_str(rest);
    out
}

fn is_russian(c: char) -> bool {
    matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

/// A paragraph is one empty line, however many breaks the stages before stacked up.
fn at_most_one_empty_line(tokens: &mut Vec<Token>) {
    let mut run = 0;
    tokens.retain(|t| {
        run = if t.is(Kind::Newline) { run + 1 } else { 0 };
        run <= 2
    });
}

fn is_mark(c: Option<char>) -> bool {
    matches!(c, Some(',' | '.' | '!' | '?' | '…' | ';' | ':'))
}

/// `, .` `,,` `. ,` and a comma opening a line are leftovers of cut words.
fn drop_stray_marks(tokens: &mut Vec<Token>) {
    let mut i = 0;
    while i < tokens.len() {
        let prev = token::prev_solid(tokens, i);
        let next = token::next_solid(tokens, i + 1);
        let stray = match token::punct_at(tokens, Some(i)) {
            Some(',') => {
                is_mark(token::punct_at(tokens, prev)) || is_mark(token::punct_at(tokens, next))
            }
            Some(';' | ':') => is_mark(token::punct_at(tokens, prev)),
            _ => false,
        };
        let line_start = prev.is_none_or(|j| tokens[j].is(Kind::Newline));
        let opens_line = line_start
            && match token::punct_at(tokens, Some(i)) {
                Some(',' | ';' | ':') => true,
                Some('.') => {
                    token::punct_at(tokens, next) != Some('.') && prev_char(tokens, i) != Some('.')
                }
                _ => false,
            };
        if stray || opens_line {
            tokens.remove(i);
        } else {
            i += 1;
        }
    }
}

fn prev_char(tokens: &[Token], i: usize) -> Option<char> {
    i.checked_sub(1).and_then(|j| tokens[j].text.chars().last())
}

/// `привет,как дела` -> `привет, как дела`. Digits are left alone: `3,5`.
fn space_after_commas(tokens: &mut Vec<Token>) {
    let mut i = 0;
    while i + 1 < tokens.len() {
        let glued = tokens[i].text == ","
            && tokens[i + 1].is(Kind::Word)
            && tokens[i + 1].text.starts_with(char::is_alphabetic);
        if glued {
            tokens.insert(i + 1, Token::space());
        }
        i += 1;
    }
}

/// Capital letter at the start of the text and of every line that starts a sentence.
fn capitalize_lines(tokens: &mut [Token]) {
    let mut sentence_ends = true;
    for i in 0..tokens.len() {
        match tokens[i].kind {
            Kind::Newline => {}
            Kind::Space => continue,
            Kind::Word if sentence_ends && at_line_start(tokens, i) => {
                tokens[i].text = token::sentence_case(&tokens[i].text);
            }
            _ => {}
        }
        sentence_ends = match tokens[i].kind {
            Kind::Newline => sentence_ends,
            _ => matches!(
                token::punct_at(tokens, Some(i)),
                Some('.' | '!' | '?' | '…')
            ),
        };
    }
}

fn at_line_start(tokens: &[Token], i: usize) -> bool {
    token::prev_solid(tokens, i).is_none_or(|j| tokens[j].is(Kind::Newline))
}
