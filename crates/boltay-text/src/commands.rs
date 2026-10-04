use crate::config::Commands;
use crate::token::{self, fold, match_phrase, remove_range, Edge, Kind, Token};

/// Whether a line break command starts at token `i`, for the stages that run earlier.
pub(crate) fn break_at(tokens: &[Token], i: usize) -> bool {
    BREAKS
        .iter()
        .any(|(p, _)| match_phrase(tokens, i, p).is_some())
}

/// Line breaks, with how many `\n` each one puts in. Longest first at the same position.
const BREAKS: &[(&str, usize)] = &[
    ("с новой строки", 1),
    ("новая строка", 1),
    ("new line", 1),
    ("newline", 1),
    ("новый абзац", 2),
    ("new paragraph", 2),
    ("абзац", 2),
];
const DELETE_LAST: &[&str] = &[
    "удали последнее предложение",
    "удали последнее",
    "сотри последнее",
    "scratch that",
];

/// How a spoken mark sits between its neighbours.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stick {
    /// Glued to the word before: `Готово!`
    Left,
    /// Glued to the word before, and the sentence may go on after it: `(в скобках).`
    Close,
    /// Glued to the word after: `«цитата`
    Open,
    /// Glued on both sides: `кто-то`
    Both,
    /// Spaced on both sides: `Москва — столица`
    Dash,
}

/// Spoken marks, longest first so `точка с запятой` is not read as `точка`. The last
/// field marks words that mean something else often enough to be off by default: the
/// recognizer puts full stops and commas in by itself.
const SPOKEN: &[(&str, &str, Stick, bool)] = &[
    ("вопросительный знак", "?", Stick::Left, false),
    ("восклицательный знак", "!", Stick::Left, false),
    ("точка с запятой", ";", Stick::Left, false),
    ("двоеточие", ":", Stick::Left, false),
    ("многоточие", "…", Stick::Left, false),
    ("троеточие", "…", Stick::Left, false),
    ("тире", "—", Stick::Dash, false),
    ("дефис", "-", Stick::Both, false),
    ("открыть кавычки", "«", Stick::Open, false),
    ("открой кавычки", "«", Stick::Open, false),
    ("закрыть кавычки", "»", Stick::Close, false),
    ("закрой кавычки", "»", Stick::Close, false),
    ("открыть скобку", "(", Stick::Open, false),
    ("открой скобку", "(", Stick::Open, false),
    ("закрыть скобку", ")", Stick::Close, false),
    ("закрой скобку", ")", Stick::Close, false),
    ("запятая", ",", Stick::Left, true),
    ("точка", ".", Stick::Left, true),
    ("question mark", "?", Stick::Left, false),
    ("exclamation mark", "!", Stick::Left, false),
    ("exclamation point", "!", Stick::Left, false),
    ("semicolon", ";", Stick::Left, false),
    ("ellipsis", "…", Stick::Left, false),
    ("hyphen", "-", Stick::Both, false),
    ("open quote", "\"", Stick::Open, false),
    ("close quote", "\"", Stick::Close, false),
    ("open paren", "(", Stick::Open, false),
    ("close paren", ")", Stick::Close, false),
    ("full stop", ".", Stick::Left, true),
    ("comma", ",", Stick::Left, true),
    ("period", ".", Stick::Left, true),
    ("colon", ":", Stick::Left, true),
];

/// `точка зрения` is a noun phrase, not a full stop.
const NOT_A_POINT: &[&str] = &[
    "зрения",
    "отсчета",
    "доступа",
    "опоры",
    "кипения",
    "невозврата",
    "росы",
    "роста",
    "входа",
    "сборки",
    "продаж",
    "of",
];

pub(crate) fn apply(config: &Commands, text: &str) -> String {
    let mut tokens = token::tokenize(text);
    while let Some((start, end, mark, stick)) = find_spoken(&tokens, config.spoken_punctuation) {
        place_mark(&mut tokens, start, end, mark, stick);
    }
    while let Some((start, end, breaks)) = find_break(&tokens) {
        place_break(&mut tokens, start, end, breaks);
    }
    while let Some((start, end)) = find_delete(&tokens) {
        delete_last(&mut tokens, start, end);
    }
    token::render(&tokens)
}

fn find_phrase<'a>(
    tokens: &[Token],
    phrases: &[&'a str],
    from: usize,
) -> Option<(usize, usize, &'a str)> {
    (from..tokens.len())
        .filter(|&i| tokens[i].is(Kind::Word))
        .find_map(|i| {
            phrases
                .iter()
                .find_map(|p| match_phrase(tokens, i, p).map(|end| (i, end, *p)))
        })
}

/// A break is a phrase of its own: `…текста. Абзац. Так…` or `Иван, новая строка, как
/// дела`. Inside a sentence it is just words: `прочитай этот абзац`, `Абзац, который…`.
fn find_break(tokens: &[Token]) -> Option<(usize, usize, usize)> {
    let phrases: Vec<&str> = BREAKS.iter().map(|(p, _)| *p).collect();
    let mut from = 0;
    while let Some((start, end, phrase)) = find_phrase(tokens, &phrases, from) {
        let left = token::left_edge(tokens, start);
        let standalone = match token::right_edge(tokens, end) {
            Edge::Line | Edge::Terminal | Edge::Stop => left != Edge::Word,
            Edge::Pause => left == Edge::Pause,
            _ => false,
        };
        if standalone {
            let breaks = BREAKS.iter().find(|(p, _)| *p == phrase).unwrap().1;
            return Some((start, end, breaks));
        }
        from = end;
    }
    None
}

/// The command eats the punctuation the recognizer put after it (`Новая строка.`) and the
/// spaces around it. What comes before stays: `Дорогой Иван, новая строка` keeps the comma.
/// Line breaks a pause made next to it go too, or the command would stack its break on
/// them. Breaks said out loud add up: `Новая строка. Новая строка.` is an empty line.
fn place_break(tokens: &mut Vec<Token>, start: usize, end: usize, breaks: usize) {
    let said = end;
    let end = skip_marks(tokens, end, |c| {
        matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '…')
    });
    let closes = (said..end).any(|j| token::punct_at(tokens, Some(j)).is_some_and(is_terminal));
    let blank = |t: &Token| t.is(Kind::Space) || (t.is(Kind::Newline) && !t.spoken);
    let mut from = start;
    while from > 0 && blank(&tokens[from - 1]) {
        from -= 1;
    }
    let mut to = end;
    while to < tokens.len() && blank(&tokens[to]) {
        to += 1;
    }
    let before = token::left_edge(tokens, start);
    // `Скидываю отчёт, абзац. Жду` ends a sentence: the recognizer's comma becomes a full
    // stop. `Первое, абзац, второе` goes on with the list and keeps it, and so do a greeting
    // and a sign-off: `Дорогой Иван, новая строка. Как дела?`
    let mut ends_sentence = false;
    if let Some(j) = token::prev_solid(tokens, from).filter(|_| closes) {
        if matches!(token::punct_at(tokens, Some(j)), Some(',' | ';')) && !salutation(tokens, j) {
            tokens[j] = Token::new(Kind::Punct, ".");
            ends_sentence = true;
        }
    }
    let spoken = Token {
        spoken: true,
        ..Token::new(Kind::Newline, "\n")
    };
    tokens.splice(from..to, (0..breaks).map(|_| spoken.clone()));
    if before.starts_sentence() || ends_sentence {
        token::capitalize_word_from(tokens, from + breaks);
    }
}

/// The recognizer may have put the mark in already: `открыть кавычки: «Всё`, `завтра».
/// Закрыть кавычки`. A quote inside the quote is not that mark: in `открыть кавычки фильм
/// «Брат» закрыть кавычки` both pairs are needed.
fn already_there(tokens: &[Token], from: usize, to: usize, mark: &str, stick: Stick) -> bool {
    let mark = mark.chars().next();
    let (open, close) = match mark {
        Some('«' | '»') => ('«', '»'),
        Some('(' | ')') => ('(', ')'),
        _ => ('"', '"'),
    };
    let unclosed = |tokens: &[Token]| {
        let marks = (0..tokens.len()).filter_map(|j| token::punct_at(tokens, Some(j)));
        if open == close {
            marks.filter(|&c| c == open).count() % 2
        } else {
            marks.fold(0usize, |depth, c| match c {
                c if c == open => depth + 1,
                c if c == close => depth.saturating_sub(1),
                _ => depth,
            })
        }
    };
    match stick {
        // The recognizer's quote is ours, unless the user went on after its pair and only
        // then said the close: `открыть кавычки «Брат» мой любимый фильм закрыть кавычки`.
        Stick::Open => {
            let Some(at) =
                token::next_solid(tokens, to).filter(|&j| token::punct_at(tokens, Some(j)) == mark)
            else {
                return false;
            };
            let Some(end) = (at + 1..tokens.len()).find(|&j| unclosed(&tokens[at..=j]) == 0) else {
                return true;
            };
            let said = |j: usize, stick: Stick, c: char| {
                SPOKEN.iter().any(|&(phrase, m, s, _)| {
                    s == stick && m.starts_with(c) && match_phrase(tokens, j, phrase).is_some()
                })
            };
            let mut words = false;
            for j in (end + 1..tokens.len()).filter(|&j| tokens[j].is(Kind::Word)) {
                if said(j, Stick::Close, close) {
                    return !words;
                }
                if said(j, Stick::Open, open) {
                    break;
                }
                words = true;
            }
            true
        }
        // `«Ты придёшь»? Закрыть кавычки`: the question mark of the sentence sits between.
        Stick::Close => {
            let mut j = token::prev_solid(tokens, from);
            while token::punct_at(tokens, j).is_some_and(is_terminal) {
                j = j.and_then(|j| token::prev_solid(tokens, j));
            }
            token::punct_at(tokens, j) == mark && unclosed(&tokens[..from]) == 0
        }
        _ => token::punct_at(tokens, token::prev_solid(tokens, from)) == mark,
    }
}

/// Greetings that open a greeting line: `Добрый день, Анна Сергеевна`, `Привет, мам`.
const GREETINGS: &[&str] = &[
    "привет",
    "всем привет",
    "здравствуй",
    "здравствуйте",
    "приветствую",
    "добрый день",
    "добрый вечер",
    "доброе утро",
    "доброй ночи",
    "hi",
    "hello",
    "hey",
    "good morning",
    "good afternoon",
    "good evening",
];

/// An address may open the line on its own: `Уважаемая Анна Сергеевна`, `Дорогой друг`.
const TITLES: &[&str] = &[
    "дорогой",
    "дорогая",
    "дорогие",
    "уважаемый",
    "уважаемая",
    "уважаемые",
    "милый",
    "милая",
    "dear",
];

/// Who a greeting or a title speaks to, besides names, by the start of the word:
/// `коллеги`, `клиентам`, `всем`. A longer word is another word: `гостиница`.
const ADDRESSEES: &[&str] = &[
    "коллег",
    "клиент",
    "партнер",
    "друг",
    "друз",
    "подруг",
    "покупател",
    "пользовател",
    "подписчик",
    "гост",
    "участник",
    "сотрудник",
    "господ",
    "дам",
    "родител",
    "сосед",
    "читател",
    "зрител",
    "мам",
    "пап",
    "сын",
    "доч",
    "все",
    "команд",
    "ребят",
    "народ",
    "мужик",
    "девочк",
    "девчонк",
    "пацан",
    "бро",
    "дружищ",
    "любим",
    "team",
    "all",
    "everyone",
    "guys",
    "folks",
    "there",
    "colleague",
    "friend",
    "sir",
    "madam",
    "customer",
    "client",
    "partner",
];

/// Thanks and nothing else is a sign-off line: `Спасибо вам большое`, `Thanks in advance`.
const THANKS: &[&str] = &[
    "спасибо",
    "благодарю",
    "благодарен",
    "благодарна",
    "благодарны",
    "regards",
    "thanks",
    "thank",
    "cheers",
    "sincerely",
];
const THANKS_WITH: &[&str] = &[
    "большое",
    "огромное",
    "всем",
    "заранее",
    "еще",
    "раз",
    "вам",
    "вас",
    "тебе",
    "you",
    "many",
    "warm",
    "kind",
    "best",
    "yours",
    "again",
    "a",
    "lot",
    "in",
    "advance",
    "so",
    "much",
];

/// Closings said as a line of their own.
const SIGN_OFFS: &[&str] = &[
    "всего доброго",
    "всего хорошего",
    "с наилучшими пожеланиями",
    "best wishes",
];

/// Whether the sentence that ends at the comma `at` is a greeting or a sign-off line, which
/// keeps its comma before the break. `Спасибо за отчёт, абзац`, `Привет, я тут подумал,
/// абзац` and `Передай Ане привет, абзац` are ordinary sentences.
fn salutation(tokens: &[Token], at: usize) -> bool {
    let start = sentence_start(tokens, at);
    let words: Vec<usize> = (start..at).filter(|&j| tokens[j].is(Kind::Word)).collect();
    let folded: Vec<String> = words.iter().map(|&j| fold(&tokens[j].text)).collect();
    let folded: Vec<&str> = folded.iter().map(String::as_str).collect();
    let line = folded.join(" ");
    // The word at `k` opens the line or follows a comma: `Жду ответа, с уважением`.
    let own_clause = |k: usize| {
        k == 0 || token::punct_at(tokens, token::prev_solid(tokens, words[k])) == Some(',')
    };

    let thanks = folded.iter().any(|w| THANKS.contains(w))
        && folded
            .iter()
            .all(|w| THANKS.contains(w) || THANKS_WITH.contains(w));
    if thanks || SIGN_OFFS.contains(&line.as_str()) {
        return true;
    }
    // `С уважением`, `С глубоким уважением`, `…, с уважением и благодарностью`.
    let respect = ["уважением", "благодарностью"]
        .iter()
        .any(|last| folded.last() == Some(last))
        && folded
            .iter()
            .rposition(|w| *w == "с")
            .is_some_and(|k| folded.len() - k <= 4 && own_clause(k));
    if respect {
        return true;
    }

    // A name, or a word from the list; `и`, `and`, `or` join them: `друзья и коллеги`.
    let listed = |k: usize| {
        let word = folded[k];
        matches!(word, "и" | "and" | "or")
            || TITLES.contains(&word)
            || ADDRESSEES.iter().any(|stem| {
                word.starts_with(stem) && word.chars().count() <= stem.chars().count() + 3
            })
    };
    let name = |k: usize| {
        let word = folded[k];
        tokens[words[k]].text.starts_with(char::is_uppercase)
            && word != "i"
            && !word.starts_with("i'")
            && !word.starts_with("i’")
    };
    let addressee = |k: usize| listed(k) || name(k);
    // Without a comma `маме привет` passes regards on: only `всем` and English words
    // like `team`, which have no case, still make an address.
    let bare =
        |k: usize| matches!(folded[k], "всем" | "все") || (folded[k].is_ascii() && listed(k));
    if folded.first().is_some_and(|w| TITLES.contains(w)) {
        return folded.len() <= 4 && (1..folded.len()).all(addressee);
    }
    GREETINGS.iter().any(|greeting| {
        let greeting: Vec<&str> = greeting.split(' ').collect();
        let n = greeting.len();
        // `Привет, мам`, `Добрый день, Анна Сергеевна`; `Привет, у меня вопрос` is a sentence.
        let opens = folded.starts_with(&greeting)
            && folded.len() - n <= 4
            && (n == folded.len()
                || own_clause(n)
                || (n..folded.len()).all(|k| bare(k) || name(k)))
            && (n..folded.len()).all(addressee);
        // `Иван, привет`, `Всем добрый день`; `Передай Ане привет` and `Он говорит, привет`
        // are sentences.
        let k = folded.len().saturating_sub(n);
        let closes = folded.ends_with(&greeting)
            && (1..=2).contains(&k)
            && ((own_clause(k) && (0..k).all(addressee)) || (0..k).all(bare));
        opens || closes
    })
}

fn skip_marks(tokens: &[Token], mut i: usize, allowed: impl Fn(char) -> bool) -> usize {
    loop {
        let j = token::next_solid(tokens, i);
        match token::punct_at(tokens, j) {
            Some(c) if allowed(c) => i = j.unwrap() + 1,
            _ => return i,
        }
    }
}

/// Deleting is destructive, so the command must end its phrase: `удали последнее фото` is
/// a request to someone else, not to us.
fn find_delete(tokens: &[Token]) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some((start, end, _)) = find_phrase(tokens, DELETE_LAST, from) {
        if token::right_edge(tokens, end) != Edge::Word {
            return Some((start, end));
        }
        from = end;
    }
    None
}

/// Drops the sentence the command refers to. Said mid-sentence, it drops the sentence so
/// far (`Приду в пять, удали последнее, в шесть` -> `В шесть`). Said at the start of a
/// sentence, it drops the previous one. Never reaches across a line break said out loud;
/// one a pause left is no boundary, the pause was the user making up their mind.
fn delete_last(tokens: &mut Vec<Token>, start: usize, end: usize) {
    let end = skip_marks(tokens, end, |c| !matches!(c, '(' | '«' | '"'));
    let sentence = sentence_start(tokens, start);
    let from = if token::next_solid(tokens, sentence) == Some(start) {
        let mut before = token::prev_solid(tokens, start);
        let mut paused = false;
        while let Some(j) = before.filter(|&j| tokens[j].is(Kind::Newline) && !tokens[j].spoken) {
            paused = true;
            before = token::prev_solid(tokens, j);
        }
        match before {
            Some(mut j) if token::punct_at(tokens, Some(j)).is_some_and(is_terminal) => {
                // Step over the whole `...` or `?!` that ends the previous sentence.
                while j > 0 && token::punct_at(tokens, Some(j - 1)).is_some_and(is_terminal) {
                    j -= 1;
                }
                sentence_start(tokens, j)
            }
            Some(j) if paused && !tokens[j].is(Kind::Newline) => sentence_start(tokens, j + 1),
            _ => sentence,
        }
    } else {
        sentence
    };
    remove_range(tokens, from, end);
    if token::left_edge(tokens, from).starts_sentence() {
        token::capitalize_word_from(tokens, from);
    }
    token::tidy(tokens);
}

fn is_terminal(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…')
}

/// Index right after the previous sentence end or line break.
fn sentence_start(tokens: &[Token], i: usize) -> usize {
    (0..i)
        .rev()
        .find(|&j| {
            tokens[j].is(Kind::Newline) || token::punct_at(tokens, Some(j)).is_some_and(is_terminal)
        })
        .map_or(0, |j| j + 1)
}

fn find_spoken(tokens: &[Token], optional: bool) -> Option<(usize, usize, &'static str, Stick)> {
    let phrases: Vec<&str> = SPOKEN
        .iter()
        .filter(|m| optional || !m.3)
        .map(|m| m.0)
        .collect();
    let mut from = 0;
    loop {
        let (start, end, phrase) = find_phrase(tokens, &phrases, from)?;
        let next = token::next_solid(tokens, end)
            .filter(|&j| tokens[j].is(Kind::Word))
            .map(|j| fold(&tokens[j].text));
        let blocked = matches!(phrase, "точка" | "period")
            && next.is_some_and(|w| NOT_A_POINT.contains(&w.as_str()));
        if !blocked {
            let (_, mark, stick, _) = *SPOKEN.iter().find(|m| m.0 == phrase).unwrap();
            return Some((start, end, mark, stick));
        }
        from = end;
    }
}

/// `Привет, запятая, как дела` -> `Привет, как дела`, `дальше, троеточие.` -> `дальше…`:
/// the recognizer often punctuates around the spoken mark as well, those marks are dropped
/// in favour of the spoken one. A closing bracket keeps the full stop after it, an opening
/// one keeps what came before: `сказал: «`.
fn place_mark(tokens: &mut Vec<Token>, start: usize, end: usize, mark: &str, stick: Stick) {
    let eaten_after: &[char] = match stick {
        Stick::Close => &[','],
        Stick::Open | Stick::Both => &['.', ',', ';', ':'],
        Stick::Left | Stick::Dash => &['.', ',', ';', ':', '…'],
    };
    let eaten_before: &[char] = match stick {
        Stick::Open => &[','],
        _ => &['.', ',', ';', ':'],
    };
    let end = skip_marks(tokens, end, |c| eaten_after.contains(&c));
    let sentence_start = token::left_edge(tokens, start).starts_sentence();
    let mut from = start;
    // Only pauses make line breaks before this stage: `Как дела ⏎⏎ вопросительный знак`
    // still ends the sentence that came before the pause.
    if matches!(stick, Stick::Left | Stick::Close) {
        while from > 0 && (tokens[from - 1].is(Kind::Space) || tokens[from - 1].is(Kind::Newline)) {
            from -= 1;
        }
    }
    while let Some(j) = token::prev_solid(tokens, from) {
        match token::punct_at(tokens, Some(j)) {
            Some(c) if eaten_before.contains(&c) => from = j,
            _ => break,
        }
    }
    while from > 0 && tokens[from - 1].is(Kind::Space) {
        from -= 1;
    }
    let mut to = end;
    while to < tokens.len() && tokens[to].is(Kind::Space) {
        to += 1;
    }

    let already = already_there(tokens, from, to, mark, stick);
    // `«Стой»! Закрыть кавычки. И побежал`: the sentence has its mark before the quote.
    if already && stick == Stick::Close {
        let ended =
            token::punct_at(tokens, token::prev_solid(tokens, from)).is_some_and(is_terminal);
        if ended {
            to = skip_marks(tokens, to, |c| c == '.');
            while to < tokens.len() && tokens[to].is(Kind::Space) {
                to += 1;
            }
        }
    }
    let spaced_after = tokens.get(to).is_some_and(|t| {
        t.is(Kind::Word) || t.text == "—" || token::is_opening(token::punct_at(tokens, Some(to)))
    });
    // `«…завтра», закрыть кавычки, но` -> `«…завтра», но`: the clause after the quote wants
    // its comma, the one in a bracket `(без НДС), закрыть скобку, составляет` does not.
    let comma = stick == Stick::Close
        && (from..to).any(|j| token::punct_at(tokens, Some(j)) == Some(','))
        && tokens
            .get(to)
            .is_some_and(|t| t.is(Kind::Word) && token::takes_comma(&t.text));
    let mut replacement = Vec::new();
    if already {
        // Only the command goes away, the words around keep their spaces.
        let keep_space = match stick {
            Stick::Open => from > 0 && to < tokens.len() && !tokens[from - 1].is(Kind::Newline),
            _ => spaced_after,
        };
        if comma {
            replacement.push(Token::new(Kind::Punct, ","));
        }
        if keep_space {
            replacement.push(Token::space());
        }
    } else {
        let spaced_before = from > 0
            && !tokens[from - 1].is(Kind::Newline)
            && !token::is_opening(token::punct_at(tokens, Some(from - 1)));
        if matches!(stick, Stick::Dash | Stick::Open) && spaced_before {
            replacement.push(Token::space());
        }
        replacement.push(Token::new(Kind::Punct, mark));
        if comma {
            replacement.push(Token::new(Kind::Punct, ","));
        }
        if matches!(stick, Stick::Left | Stick::Close | Stick::Dash) && spaced_after {
            replacement.push(Token::space());
        }
    }
    let after = from + replacement.len();
    tokens.splice(from..to, replacement);
    if matches!(mark, "." | "!" | "?") || (stick == Stick::Open && sentence_start) {
        token::capitalize_word_from(tokens, after);
    }
}
