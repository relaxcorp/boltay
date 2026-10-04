#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Word,
    Space,
    Newline,
    Punct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: Kind,
    pub text: String,
    /// A line break said out loud. The next break command keeps it, while a break
    /// left by a pause gets swallowed.
    pub spoken: bool,
}

impl Token {
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            spoken: false,
        }
    }

    pub fn space() -> Self {
        Self::new(Kind::Space, " ")
    }

    pub fn is(&self, kind: Kind) -> bool {
        self.kind == kind
    }

    fn punct(&self) -> Option<char> {
        match self.kind {
            Kind::Punct => self.text.chars().next(),
            _ => None,
        }
    }
}

pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '*' || c == '_' || c == '\u{301}'
}

fn is_joiner(c: char) -> bool {
    matches!(c, '-' | '\'' | '’')
}

pub(crate) fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        let kind = if c == '\n' {
            i += 1;
            Kind::Newline
        } else if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() && chars[i] != '\n' {
                i += 1;
            }
            Kind::Space
        } else if is_word_char(c) {
            i += 1;
            while i < chars.len() {
                if is_word_char(chars[i]) {
                    i += 1;
                } else if is_joiner(chars[i]) && chars.get(i + 1).is_some_and(|&n| is_word_char(n))
                {
                    i += 2;
                } else {
                    break;
                }
            }
            Kind::Word
        } else {
            i += 1;
            Kind::Punct
        };
        tokens.push(Token::new(kind, chars[start..i].iter().collect::<String>()));
    }
    tokens
}

pub(crate) fn render(tokens: &[Token]) -> String {
    tokens.iter().map(|t| t.text.as_str()).collect()
}

/// Lowercase and fold `ё` into `е`: speech models and people use both spellings.
pub(crate) fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect()
}

/// `fold(text) == folded`, without allocating: this runs for every word and phrase.
pub(crate) fn folds_to(text: &str, folded: &str) -> bool {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .eq(folded.chars())
}

pub(crate) fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Capitalizes a word that opens a sentence, but leaves `iPhone` and `eBay` alone.
pub(crate) fn sentence_case(word: &str) -> String {
    if word.chars().any(char::is_uppercase) {
        word.to_string()
    } else {
        capitalize(word)
    }
}

/// Carries the case pattern of `source` over to `replacement`: `БЛЯТЬ` -> `БЛИН`, `Блять` -> `Блин`.
pub(crate) fn match_case(source: &str, replacement: &str) -> String {
    let letters: Vec<char> = source.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if letters.first().is_some_and(|c| c.is_uppercase()) {
        capitalize(replacement)
    } else {
        replacement.to_string()
    }
}

/// What sits next to a word, ignoring spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    /// Start or end of the text, or a line break.
    Line,
    /// `,` or a dash: a pause inside the sentence.
    Pause,
    /// `.` `!` `?` `…`
    Terminal,
    /// `;` `:`
    Stop,
    /// Opening or closing bracket or quote.
    Bracket,
    Word,
    Other,
}

impl Edge {
    pub fn is_boundary(self) -> bool {
        !matches!(self, Edge::Word)
    }

    /// Whether the next word starts a new sentence.
    pub fn starts_sentence(self) -> bool {
        matches!(self, Edge::Line | Edge::Terminal)
    }
}

fn edge_of(token: Option<&Token>) -> Edge {
    let Some(token) = token else {
        return Edge::Line;
    };
    match token.kind {
        Kind::Newline => Edge::Line,
        Kind::Word => Edge::Word,
        Kind::Space => unreachable!("edges skip spaces"),
        Kind::Punct => match token.punct() {
            Some(',' | '—' | '–' | '-') => Edge::Pause,
            Some('.' | '!' | '?' | '…') => Edge::Terminal,
            Some(';' | ':') => Edge::Stop,
            Some('(' | ')' | '[' | ']' | '«' | '»' | '"' | '“' | '”' | '„') => {
                Edge::Bracket
            }
            _ => Edge::Other,
        },
    }
}

/// Index of the nearest non-space token before `i`.
pub(crate) fn prev_solid(tokens: &[Token], i: usize) -> Option<usize> {
    (0..i).rev().find(|&j| !tokens[j].is(Kind::Space))
}

/// Index of the nearest non-space token at or after `i`.
pub(crate) fn next_solid(tokens: &[Token], i: usize) -> Option<usize> {
    (i..tokens.len()).find(|&j| !tokens[j].is(Kind::Space))
}

pub(crate) fn left_edge(tokens: &[Token], i: usize) -> Edge {
    edge_of(prev_solid(tokens, i).map(|j| &tokens[j]))
}

pub(crate) fn right_edge(tokens: &[Token], i: usize) -> Edge {
    edge_of(next_solid(tokens, i).map(|j| &tokens[j]))
}

pub(crate) fn punct_at(tokens: &[Token], i: Option<usize>) -> Option<char> {
    i.and_then(|i| tokens[i].punct())
}

/// Matches `phrase` (already folded, words separated by single spaces) against the
/// words starting at `start`. Returns the end index of the match.
pub(crate) fn match_phrase(tokens: &[Token], start: usize, phrase: &str) -> Option<usize> {
    let mut i = start;
    for (n, word) in phrase.split(' ').enumerate() {
        if n > 0 {
            if !tokens.get(i)?.is(Kind::Space) {
                return None;
            }
            i += 1;
        }
        let token = tokens.get(i)?;
        if !token.is(Kind::Word) || !folds_to(&token.text, word) {
            return None;
        }
        i += 1;
    }
    Some(i)
}

pub(crate) fn capitalize_word_from(tokens: &mut [Token], i: usize) {
    let from = i.min(tokens.len());
    if let Some(token) = tokens[from..].iter_mut().find(|t| !t.is(Kind::Space)) {
        if token.is(Kind::Word) {
            token.text = sentence_case(&token.text);
        }
    }
}

/// Removes the words in `start..end` and repairs the punctuation around them, the way
/// a person would after crossing a word out:
///
/// - `Я, короче, пошёл` -> `Я пошёл` (the pair of commas goes with the word)
/// - `Я пришёл, короче.` -> `Я пришёл.`
/// - `Короче, я пошёл` -> `Я пошёл`
/// - `Я пришёл. Вот. Потом` -> `Я пришёл. Потом`
/// - `Я ээ пошёл` -> `Я пошёл`
///
/// Returns where the removed span began. Leaves stray spaces for [`tidy`].
pub(crate) fn cut(tokens: &mut Vec<Token>, start: usize, end: usize) -> usize {
    let left_idx = prev_solid(tokens, start);
    let right_idx = next_solid(tokens, end);
    let left = edge_of(left_idx.map(|j| &tokens[j]));
    let right = edge_of(right_idx.map(|j| &tokens[j]));
    let opens = matches!(left, Edge::Line | Edge::Terminal | Edge::Stop)
        || (left == Edge::Bracket && is_opening(punct_at(tokens, left_idx)));

    let (from, to, capitalize) = match (left, right) {
        (Edge::Pause, Edge::Pause) => {
            let (l, r) = (left_idx.unwrap(), right_idx.unwrap());
            let chain = tokens[start..end].iter().any(|t| t.text == ",");
            if keeps_left_comma(tokens, l, r, chain) {
                (start, r + 1, false)
            } else {
                (l, r + 1, false)
            }
        }
        (Edge::Pause, Edge::Line | Edge::Terminal | Edge::Stop | Edge::Bracket) => {
            (left_idx.unwrap(), end, false)
        }
        _ if opens => {
            let swallow_right = match right {
                Edge::Pause => true,
                Edge::Terminal | Edge::Stop => left != Edge::Bracket,
                _ => false,
            };
            let to = if swallow_right {
                // All of `...` or `?!`, not just the first dot.
                let mut to = right_idx.unwrap() + 1;
                while tokens
                    .get(to)
                    .is_some_and(|t| t.is(Kind::Punct) && edge_of(Some(t)) == right)
                {
                    to += 1;
                }
                to
            } else {
                end
            };
            // `«Ну, ладно»` -> `«Ладно»`: a capital on the cut word moves to the next one.
            let capitalized = tokens[start].text.starts_with(char::is_uppercase);
            (start, to, left.starts_sentence() || capitalized)
        }
        _ => (start, end, false),
    };

    remove_range(tokens, from, to);
    if capitalize {
        capitalize_word_from(tokens, from);
    }
    from
}

/// Where to resume a left-to-right scan after a cut at `at`: a few words back, since a
/// cut can leave the words before it newly set off by punctuation.
pub(crate) fn rewind(at: usize) -> usize {
    at.saturating_sub(16)
}

/// A word cut from between two commas takes both with it (`Я, короче, пошёл` ->
/// `Я пошёл`), unless the left one belongs to the sentence:
///
/// - it comes before a conjunction: `Хорошо, вот, а потом` -> `Хорошо, а потом`;
/// - it closes an aside or an address: `Проверь, пожалуйста, ну, этот код`,
///   `Слушай, Серёга, короче, надо`, `Слушай, короче, надо`;
/// - it follows a whole clause of three words or more: `Когда я пришёл, ну, все ушли` ->
///   `Когда я пришёл, все ушли`. One or two words are a subject glued to its verb:
///   `Мой брат, типа, пришёл` -> `Мой брат пришёл`;
/// - several fillers in a row follow a whole clause: `Я не знаю, это самое, короче, капец`.
///   A lone subject stays glued: `Мы, типа, ну, договорились` -> `Мы договорились`;
/// - a line break command follows, which only counts as a phrase of its own:
///   `Так, короче, абзац` -> `Так, абзац`.
fn keeps_left_comma(tokens: &[Token], left: usize, right: usize, chain: bool) -> bool {
    if word_after(tokens, right).is_some_and(takes_comma) {
        return true;
    }
    if next_solid(tokens, right + 1).is_some_and(|j| crate::commands::break_at(tokens, j)) {
        return true;
    }
    let Some(word) = prev_solid(tokens, left).filter(|&w| tokens[w].is(Kind::Word)) else {
        return false;
    };
    let text = &tokens[word].text;
    if is_aside(text) {
        return true;
    }
    match left_edge(tokens, word) {
        // `Блин, я, ээ, опоздал`: a pronoun after a comma is the subject, not an aside.
        Edge::Pause => !takes_comma(text) && !is_pronoun(text),
        Edge::Line | Edge::Terminal | Edge::Stop => is_address(text),
        _ => chain || clause_words(tokens, left) >= 3,
    }
}

/// Words between the previous punctuation mark, or the start, and the token at `end`.
fn clause_words(tokens: &[Token], end: usize) -> usize {
    tokens[..end]
        .iter()
        .rev()
        .take_while(|t| !t.is(Kind::Punct))
        .filter(|t| t.is(Kind::Word))
        .count()
}

/// Parenthetical words that keep their commas wherever they stand. `говоря` covers
/// `честно говоря`, `короче говоря`, `кстати говоря`.
fn is_aside(word: &str) -> bool {
    matches!(
        fold(word).as_str(),
        "говоря"
            | "пожалуйста"
            | "кстати"
            | "конечно"
            | "наверное"
            | "например"
            | "во-первых"
            | "во-вторых"
            | "по-моему"
    )
}

fn is_pronoun(word: &str) -> bool {
    matches!(
        fold(word).as_str(),
        "я" | "ты"
            | "он"
            | "она"
            | "оно"
            | "мы"
            | "вы"
            | "они"
            | "i"
            | "you"
            | "he"
            | "she"
            | "it"
            | "we"
            | "they"
    )
}

/// Words that open a sentence and are set off by a comma of their own.
fn is_address(word: &str) -> bool {
    matches!(
        fold(word).as_str(),
        "слушай"
            | "слушайте"
            | "смотри"
            | "смотрите"
            | "кстати"
            | "пожалуйста"
            | "знаешь"
            | "знаете"
            | "понимаешь"
            | "понимаете"
            | "представляешь"
            | "извини"
            | "извините"
            | "привет"
            | "здравствуй"
            | "здравствуйте"
            | "спасибо"
            | "хорошо"
            | "ладно"
            | "окей"
            | "итак"
            | "да"
            | "нет"
            | "эй"
            | "listen"
            | "look"
            | "please"
            | "hey"
            | "okay"
            | "ok"
            | "yes"
            | "no"
            | "well"
    )
}

fn word_after(tokens: &[Token], i: usize) -> Option<&str> {
    next_solid(tokens, i + 1)
        .filter(|&j| tokens[j].is(Kind::Word))
        .map(|j| tokens[j].text.as_str())
}

/// Conjunctions that open a clause and want a comma in front of them.
pub(crate) fn takes_comma(word: &str) -> bool {
    let word = fold(word);
    matches!(
        word.as_str(),
        "а" | "но"
            | "однако"
            | "зато"
            | "хотя"
            | "что"
            | "чтобы"
            | "чем"
            | "где"
            | "куда"
            | "откуда"
            | "когда"
            | "если"
            | "как"
            | "потому"
            | "поэтому"
            | "but"
            | "that"
            | "which"
            | "who"
            | "whom"
            | "whose"
    ) || word.starts_with("котор")
}

pub(crate) fn is_opening(c: Option<char>) -> bool {
    matches!(c, Some('(' | '[' | '«' | '“' | '„' | '"'))
}

/// Removes `from..to` and keeps the neighbours from gluing together.
pub(crate) fn remove_range(tokens: &mut Vec<Token>, from: usize, to: usize) {
    tokens.drain(from..to);
    let glued = from > 0
        && from < tokens.len()
        && !tokens[from - 1].is(Kind::Space)
        && !tokens[from - 1].is(Kind::Newline)
        && !tokens[from].is(Kind::Space)
        && !tokens[from].is(Kind::Newline)
        && !matches!(
            edge_of(Some(&tokens[from])),
            Edge::Pause | Edge::Terminal | Edge::Stop
        );
    if glued && !is_opening(tokens[from - 1].punct()) && tokens[from].punct() != Some(')') {
        tokens.insert(from, Token::space());
    }
}

/// Collapses runs of spaces, drops spaces at line edges and before closing punctuation.
pub(crate) fn tidy(tokens: &mut Vec<Token>) {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens.drain(..) {
        match token.kind {
            Kind::Space => {
                let after_opening = out
                    .last()
                    .and_then(Token::punct)
                    .is_some_and(|c| is_opening(Some(c)) && c != '"');
                if after_opening
                    || out
                        .last()
                        .is_none_or(|t| t.is(Kind::Space) || t.is(Kind::Newline))
                {
                    continue;
                }
                out.push(Token::space());
            }
            Kind::Newline => {
                if out.last().is_some_and(|t| t.is(Kind::Space)) {
                    out.pop();
                }
                out.push(token);
            }
            Kind::Punct
                if matches!(
                    token.punct(),
                    Some(',' | '.' | '!' | '?' | '…' | ';' | ':' | ')')
                ) =>
            {
                if out.last().is_some_and(|t| t.is(Kind::Space)) {
                    out.pop();
                }
                out.push(token);
            }
            _ => out.push(token),
        }
    }
    if out.last().is_some_and(|t| t.is(Kind::Space)) {
        out.pop();
    }
    *tokens = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        tokenize(text)
            .into_iter()
            .filter(|t| t.is(Kind::Word))
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn round_trips_any_text() {
        for text in [
            "",
            "  a\t b\n\nc ",
            "Привет, мир!",
            "кто-то — э-э-э… 3.5 don't",
        ] {
            assert_eq!(render(&tokenize(text)), text);
        }
    }

    #[test]
    fn keeps_hyphens_and_apostrophes_inside_words() {
        assert_eq!(
            words("кто-то э-э-э don't веб-сайт"),
            ["кто-то", "э-э-э", "don't", "веб-сайт"]
        );
    }

    #[test]
    fn splits_dangling_hyphens_and_apostrophes() {
        assert_eq!(words("слово- 'цитата' - тире"), ["слово", "цитата", "тире"]);
    }

    #[test]
    fn keeps_masked_words_whole() {
        assert_eq!(words("это б**ть"), ["это", "б**ть"]);
    }

    #[test]
    fn folds_case_and_yo() {
        assert_eq!(fold("ЁЛКИ Ёжик"), "елки ежик");
    }

    #[test]
    fn carries_case() {
        assert_eq!(match_case("БЛЯТЬ", "блин"), "БЛИН");
        assert_eq!(match_case("Блять", "блин"), "Блин");
        assert_eq!(match_case("блять", "блин"), "блин");
        assert_eq!(match_case("Б", "блин"), "Блин");
    }
}
