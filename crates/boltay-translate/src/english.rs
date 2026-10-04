use std::collections::HashSet;

use crate::prepare::{Prepared, Term};
use crate::SpanKind;

/// Chat abbreviations: what goes to the translator and what they mean, for the hint.
const ABBREVIATIONS: &[(&str, &str, &str)] = &[
    ("asap", "as soon as possible", "как можно скорее"),
    ("eta", "deadline", "ожидаемое время готовности или прибытия"),
    ("eod", "end of day", "до конца дня"),
    ("eow", "end of week", "до конца недели"),
    ("cob", "close of business", "до конца рабочего дня"),
    ("fyi", "for your information", "к сведению"),
    ("btw", "by the way", "кстати"),
    ("imo", "in my opinion", "по-моему"),
    ("imho", "in my humble opinion", "по моему скромному мнению"),
    ("tbh", "to be honest", "если честно"),
    ("tbf", "to be fair", "справедливости ради"),
    ("lmk", "let me know", "дай знать"),
    ("ngl", "not going to lie", "не буду врать, честно говоря"),
    ("idk", "I don't know", "не знаю"),
    ("idc", "I don't care", "мне всё равно"),
    ("wfh", "working from home", "работаю из дома"),
    ("ooo", "out of office", "не на работе, в отпуске"),
    ("pto", "vacation", "оплачиваемый отпуск"),
    ("pls", "please", "пожалуйста"),
    ("plz", "please", "пожалуйста"),
    ("thx", "thanks", "спасибо"),
    ("thnx", "thanks", "спасибо"),
    ("ty", "thank you", "спасибо"),
    ("yw", "you're welcome", "не за что"),
    ("np", "no problem", "без проблем"),
    ("nvm", "never mind", "неважно, забудь"),
    ("omw", "on my way", "уже иду, уже еду"),
    ("brb", "be right back", "скоро вернусь"),
    ("afk", "away from keyboard", "отошёл от компьютера"),
    ("ttyl", "talk to you later", "поговорим позже"),
    ("iirc", "if I remember correctly", "если правильно помню"),
    ("afaik", "as far as I know", "насколько я знаю"),
    ("afaict", "as far as I can tell", "насколько я могу судить"),
    ("fwiw", "for what it's worth", "если это чем-то поможет"),
    ("icymi", "in case you missed it", "если пропустил"),
    ("tl;dr", "in short", "если коротко"),
    ("tldr", "in short", "если коротко"),
    ("jk", "just kidding", "шучу"),
    ("lol", "haha", "смешно, ха-ха"),
    ("lmao", "haha", "очень смешно"),
    ("omg", "oh my god", "боже мой"),
    ("wtf", "what the hell", "какого чёрта"),
    ("smh", "unbelievable", "ну и ну, качаю головой"),
    ("ofc", "of course", "конечно"),
    ("rn", "right now", "прямо сейчас"),
    ("irl", "in real life", "в реальной жизни"),
    ("pov", "point of view", "точка зрения"),
    ("fomo", "fear of missing out", "страх что-то упустить"),
    ("wip", "unfinished", "в работе, не доделано"),
    ("tbd", "to be decided", "будет решено позже"),
    ("tba", "to be announced", "объявим позже"),
    ("lgtm", "approved", "по-моему, всё хорошо, одобряю"),
    ("ptal", "please take a look", "посмотри, пожалуйста"),
    ("wdyt", "what do you think", "что думаешь?"),
    ("ymmv", "results may vary", "у всех по-разному"),
    ("otoh", "on the other hand", "с другой стороны"),
    ("hmu", "contact me", "напиши мне"),
    ("wyd", "what are you doing", "что делаешь?"),
    ("dm", "private message", "личное сообщение"),
    ("dms", "private messages", "личные сообщения"),
    ("gg", "good game", "хорошая игра, молодцы"),
    ("gl", "good luck", "удачи"),
    ("w/", "with", "с"),
    ("w/o", "without", "без"),
    ("b/c", "because", "потому что"),
    ("pr", "merge request", "пул-реквест, запрос на слияние кода"),
    ("prs", "merge requests", "пул-реквесты"),
    ("repo", "repository", "репозиторий"),
    ("repos", "repositories", "репозитории"),
    ("creds", "passwords", "доступы: логины и пароли"),
    ("prod", "production server", "продакшн, боевой сервер"),
    ("infra", "infrastructure", "инфраструктура"),
    ("deps", "dependencies", "зависимости"),
    ("db", "database", "база данных"),
];

/// Chat slang.
const SLANG: &[(&str, &str, &str)] = &[
    ("gonna", "going to", "собираюсь"),
    ("wanna", "want to", "хочу"),
    ("gotta", "have to", "надо"),
    ("kinda", "kind of", "вроде как, немного"),
    ("sorta", "sort of", "типа, отчасти"),
    ("dunno", "don't know", "не знаю"),
    ("lemme", "let me", "дай мне"),
    ("gimme", "give me", "дай мне"),
    ("imma", "I'm going to", "я собираюсь"),
    ("y'all", "you all", "вы все"),
    ("cuz", "because", "потому что"),
    ("coz", "because", "потому что"),
    ("tho", "though", "хотя"),
    ("thru", "through", "через"),
    ("w/e", "whatever", "неважно, как угодно"),
    ("janky", "unreliable", "кривой, ненадёжный"),
    (
        "legit",
        "really",
        "реально, по-настоящему; настоящий, без обмана",
    ),
    ("sus", "suspicious", "подозрительный"),
    ("lowkey", "somewhat", "немного, втихую"),
    ("highkey", "really", "очень, откровенно"),
    ("cringe", "embarrassing", "кринж, неловко"),
    ("no cap", "no kidding", "без шуток, честно"),
    ("fr", "for real", "серьёзно, правда"),
    ("bruh", "dude", "чувак (с недоумением)"),
    ("dope", "awesome", "круто"),
    ("yup", "yes", "ага, да"),
    ("nope", "no", "не-а, нет"),
    ("gotcha", "got it", "понял"),
    ("noob", "beginner", "новичок"),
    ("hella", "very", "очень"),
    ("deets", "details", "подробности"),
    ("convo", "conversation", "разговор"),
    ("bday", "birthday", "день рождения"),
    ("pic", "picture", "фото, картинка"),
    ("pics", "pictures", "фото, картинки"),
    ("vid", "video", "видео"),
    ("approx", "approximately", "примерно"),
];

/// When a jargon entry applies.
#[derive(Clone, Copy)]
enum When {
    Always,
    /// Not when the next word is one of these: `logs in` is a verb.
    NotBefore(&'static [&'static str]),
    /// Only when one of these words is in the text too: a `pod` of a cluster, not of peas.
    Near(&'static [&'static str]),
    /// Only before a name or a pronoun: `ping Alex`, not `ping is high`.
    BeforeName,
    /// Only where chat drops the subject: at the start of a sentence or after a comma.
    NoSubject,
    /// Not right after one of these words: `docs` but not `Google Docs`.
    NotAfter(&'static [&'static str]),
    /// At the start of a sentence and right before its next word, no comma between:
    /// `Heads up the API is down`, not `Heads up, Jack, the ball!`.
    Opening,
}

const KUBERNETES: &[&str] = &[
    "k8s",
    "kubernetes",
    "kubectl",
    "container",
    "containers",
    "cluster",
    "docker",
    "node",
    "nodes",
];
/// Words that make a `bug` one in the code rather than in the soup.
const SOFTWARE: &[&str] = &[
    "code",
    "app",
    "apps",
    "fix",
    "fixed",
    "fixing",
    "report",
    "reported",
    "test",
    "tests",
    "testing",
    "prod",
    "production",
    "release",
    "crash",
    "crashes",
    "crashed",
    "repro",
    "reproduce",
    "ticket",
    "tickets",
    "pr",
    "build",
    "deploy",
    "server",
    "api",
    "issue",
    "jira",
    "github",
    "gitlab",
    "commit",
    "merge",
    "version",
    "feature",
    "staging",
    "qa",
    "login",
    "page",
    "site",
    "website",
    "ui",
    "frontend",
    "backend",
    "android",
    "ios",
    "browser",
    "update",
    "patch",
    "hotfix",
    "error",
    "errors",
    "debug",
];

const PRONOUNS: &[&str] = &[
    "me",
    "you",
    "him",
    "her",
    "them",
    "us",
    "everyone",
    "everybody",
];

/// Work jargon both models translate literally (`staging` comes out as a theatre play, `logs`
/// as firewood), turned into plain English each of them gets right. Picked by running both
/// models over chat phrases.
const JARGON: &[(&str, &str, &str, When)] = &[
    (
        "staging env",
        "test server",
        "стейджинг, тестовый сервер",
        When::Always,
    ),
    (
        "staging environment",
        "test server",
        "стейджинг, тестовый сервер",
        When::Always,
    ),
    (
        "staging server",
        "test server",
        "стейджинг, тестовый сервер",
        When::Always,
    ),
    (
        "staging",
        "test server",
        "стейджинг, тестовый сервер",
        When::NotBefore(&[
            "a", "an", "the", "of", "this", "that", "these", "those", "my", "your", "his", "her",
            "its", "our", "their", "area", "areas", "ground", "post",
        ]),
    ),
    (
        "logs",
        "log files",
        "логи, журналы",
        When::NotBefore(&["in", "into", "on", "onto", "out", "off"]),
    ),
    (
        "pod",
        "container",
        "под, контейнер в Kubernetes",
        When::Near(KUBERNETES),
    ),
    (
        "pods",
        "containers",
        "поды, контейнеры в Kubernetes",
        When::Near(KUBERNETES),
    ),
    (
        "onboarding flow",
        "user onboarding process",
        "онбординг, первое знакомство пользователя с продуктом",
        When::Always,
    ),
    (
        "let's circle back",
        "let's come back to this",
        "вернёмся к этому позже",
        When::Always,
    ),
    (
        "will circle back",
        "I will get back to you",
        "вернусь к этому, отвечу позже",
        When::NoSubject,
    ),
    (
        "circle back",
        "get back to you",
        "вернусь к этому, отвечу позже",
        When::Always,
    ),
    (
        "looking into it",
        "I am looking into it",
        "разбираюсь",
        When::NoSubject,
    ),
    (
        "circling back",
        "getting back to you",
        "возвращаюсь к вопросу",
        When::Always,
    ),
    (
        "heads up:",
        "note:",
        "предупреждаю, к сведению",
        When::Always,
    ),
    (
        "heads-up:",
        "note:",
        "предупреждаю, к сведению",
        When::Always,
    ),
    (
        "heads up",
        "note:",
        "предупреждаю, к сведению",
        When::Opening,
    ),
    ("a heads up", "a warning", "предупреждение", When::Always),
    ("a heads-up", "a warning", "предупреждение", When::Always),
    (
        "the heads up",
        "the warning",
        "предупреждение",
        When::Always,
    ),
    (
        "the heads-up",
        "the warning",
        "предупреждение",
        When::Always,
    ),
    (
        "rate limit",
        "request limit",
        "ограничение частоты запросов",
        When::Always,
    ),
    (
        "rate limits",
        "request limits",
        "ограничения частоты запросов",
        When::Always,
    ),
    ("ping", "write to", "напиши, тегни", When::BeforeName),
    ("pinged", "wrote to", "написал, тегнул", When::BeforeName),
    (
        "hop on a quick call",
        "have a quick call",
        "быстро созвониться",
        When::Always,
    ),
    ("hop on a call", "have a call", "созвониться", When::Always),
    ("sync on", "discuss", "обсудить, свериться", When::Always),
    ("sync up", "talk", "свериться, обсудить", When::Always),
    ("ship it", "publish it", "выпустить, выкатить", When::Always),
    (
        "ship this",
        "publish this",
        "выпустить, выкатить",
        When::Always,
    ),
    (
        "ship that",
        "publish that",
        "выпустить, выкатить",
        When::Always,
    ),
    (
        "shipped it",
        "published it",
        "выпустили, выкатили",
        When::Always,
    ),
    (
        "the deploy",
        "the deployment",
        "деплой, выкладка на сервер",
        When::Always,
    ),
    (
        "a deploy",
        "a deployment",
        "деплой, выкладка на сервер",
        When::Always,
    ),
    (
        "this deploy",
        "this deployment",
        "деплой, выкладка на сервер",
        When::Always,
    ),
    (
        "the build",
        "the compilation",
        "сборка",
        When::NotBefore(&["quality", "of"]),
    ),
    (
        "flaky",
        "unstable",
        "нестабильный, то работает, то нет",
        When::Always,
    ),
    (
        "blocker",
        "critical problem",
        "блокер, то, что мешает двигаться дальше",
        When::Always,
    ),
    (
        "blockers",
        "critical problems",
        "блокеры, то, что мешает двигаться дальше",
        When::Always,
    ),
    (
        "a hotfix",
        "an urgent fix",
        "хотфикс, срочное исправление",
        When::Always,
    ),
    (
        "hotfix",
        "urgent fix",
        "хотфикс, срочное исправление",
        When::Always,
    ),
    (
        "hotfixes",
        "urgent fixes",
        "хотфиксы, срочные исправления",
        When::Always,
    ),
    (
        "rollback",
        "return to the previous version",
        "откат",
        When::Always,
    ),
    (
        "standup",
        "daily meeting",
        "стендап, ежедневная планёрка",
        When::Always,
    ),
    (
        "stand-up",
        "daily meeting",
        "стендап, ежедневная планёрка",
        When::Always,
    ),
    (
        "standups",
        "daily meetings",
        "стендапы, ежедневные планёрки",
        When::Always,
    ),
    ("backlog", "task list", "бэклог, список задач", When::Always),
    (
        "a bug",
        "an error",
        "баг, ошибка в программе",
        When::Near(SOFTWARE),
    ),
    (
        "bug",
        "error",
        "баг, ошибка в программе",
        When::Near(SOFTWARE),
    ),
    (
        "bugs",
        "errors",
        "баги, ошибки в программе",
        When::Near(SOFTWARE),
    ),
    ("repro", "reproduce", "воспроизвести", When::Always),
    (
        "merge it",
        "add it to the main branch",
        "смержить, влить в основную ветку",
        When::Always,
    ),
    (
        "merging it",
        "adding it to the main branch",
        "вливаю в основную ветку",
        When::Always,
    ),
    (
        "merging now",
        "adding it to the main branch now",
        "вливаю в основную ветку",
        When::Always,
    ),
    (
        "merged it",
        "added it to the main branch",
        "влил в основную ветку",
        When::Always,
    ),
    (
        "docs",
        "documentation",
        "документация",
        When::NotAfter(&["google"]),
    ),
    (
        "nudge them",
        "remind them",
        "напомнить, поторопить",
        When::Always,
    ),
    (
        "nudge him",
        "remind him",
        "напомнить, поторопить",
        When::Always,
    ),
    (
        "nudge her",
        "remind her",
        "напомнить, поторопить",
        When::Always,
    ),
    (
        "nudge me",
        "remind me",
        "напомнить, поторопить",
        When::Always,
    ),
    (
        "nudge you",
        "remind you",
        "напомнить, поторопить",
        When::Always,
    ),
];

struct Rule<'a> {
    pattern: Vec<char>,
    to: &'a str,
    meaning: &'a str,
    kind: SpanKind,
    when: When,
}

fn rules() -> Vec<Rule<'static>> {
    let mut rules = Vec::new();
    for (list, kind) in [
        (ABBREVIATIONS, SpanKind::Abbreviation),
        (SLANG, SpanKind::Slang),
    ] {
        rules.extend(list.iter().map(|&(from, to, meaning)| Rule {
            pattern: from.chars().collect(),
            to,
            meaning,
            kind,
            when: When::Always,
        }));
    }
    rules.extend(JARGON.iter().map(|&(from, to, meaning, when)| Rule {
        pattern: from.chars().collect(),
        to,
        meaning,
        kind: SpanKind::Slang,
        when,
    }));
    // Longest first: `staging env` before `staging`.
    rules.sort_by_key(|r| std::cmp::Reverse(r.pattern.len()));
    rules
}

/// Spells out abbreviations and slang and turns jargon into plain English, whole words and
/// phrases only, ignoring case. Clock times go to the 24-hour form: `3 p.m.` would end a
/// sentence for the splitter, and the base model reads `3pm` as `15 вечера`.
pub(crate) fn prepare(text: &str) -> Prepared {
    let rules = rules();
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain([text.len()])
        .collect();
    let words: HashSet<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();

    let mut out = String::with_capacity(text.len());
    let mut terms = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i == 0 || !is_word(chars[i - 1]) {
            let hit = time_at(&lower, i)
                .map(|(end, time)| (end, time.clone(), time, SpanKind::Slang))
                .or_else(|| {
                    rules.iter().find_map(|rule| {
                        let end = match_at(&lower, i, &rule.pattern)?;
                        let fits = !in_address(&chars, i, end)
                            && holds(rule.when, &chars, &lower, i, end, &words);
                        fits.then(|| {
                            let to = cased(rule.to, &chars, i, end);
                            (end, to, rule.meaning.to_string(), rule.kind)
                        })
                    })
                });
            if let Some((end, to, meaning, kind)) = hit {
                let start = out.len();
                out.push_str(&to);
                terms.push(Term {
                    kind,
                    original: bytes[i]..bytes[end],
                    prepared: start..out.len(),
                    meaning,
                });
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    Prepared { text: out, terms }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Where `pattern` ends if it matches at `start`; a space stands for any run of spaces.
fn match_at(lower: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    let mut j = start;
    for &p in pattern {
        if p == ' ' {
            if !lower.get(j).is_some_and(|&c| c == ' ' || c == '\t') {
                return None;
            }
            while lower.get(j).is_some_and(|&c| c == ' ' || c == '\t') {
                j += 1;
            }
        } else {
            if *lower.get(j)? != p {
                return None;
            }
            j += 1;
        }
    }
    let ends_word = pattern.last().is_some_and(|&c| is_word(c));
    if ends_word && lower.get(j).is_some_and(|&c| is_word(c)) {
        return None;
    }
    Some(j)
}

fn holds(
    when: When,
    chars: &[char],
    lower: &[char],
    start: usize,
    end: usize,
    words: &HashSet<String>,
) -> bool {
    let next = next_word(lower, end);
    match when {
        When::Always => true,
        When::NotBefore(list) => next.is_none_or(|(_, w)| !list.contains(&w.as_str())),
        When::Near(list) => list.iter().any(|w| words.contains(*w)),
        When::BeforeName => {
            next.is_some_and(|(at, w)| PRONOUNS.contains(&w.as_str()) || chars[at].is_uppercase())
        }
        When::NotAfter(list) => prev_word(lower, start).is_none_or(|w| !list.contains(&w.as_str())),
        When::Opening => sentence_start(chars, start) && next.is_some(),
        When::NoSubject => {
            sentence_start(chars, start)
                || chars[..start].iter().rev().find(|c| !c.is_whitespace()) == Some(&',')
        }
    }
}

fn prev_word(lower: &[char], start: usize) -> Option<String> {
    let end = start
        - lower[..start]
            .iter()
            .rev()
            .take_while(|c| **c == ' ')
            .count();
    let len = lower[..end]
        .iter()
        .rev()
        .take_while(|c| is_word(**c))
        .count();
    (len > 0).then(|| lower[end - len..end].iter().collect())
}

/// The word after `end` on the same line and where it starts.
fn next_word(lower: &[char], end: usize) -> Option<(usize, String)> {
    let start = end + lower[end..].iter().take_while(|c| **c == ' ').count();
    let word: String = lower[start..].iter().take_while(|c| is_word(**c)).collect();
    (!word.is_empty()).then_some((start, word))
}

/// `github.com/user/repo`, `me@pr.dev`: part of an address, left as is.
fn in_address(chars: &[char], start: usize, end: usize) -> bool {
    let before = start.checked_sub(1).map(|j| chars[j]);
    let glued_after =
        |c: char| chars.get(end) == Some(&c) && chars.get(end + 1).is_some_and(|&n| is_word(n));
    matches!(before, Some('.' | '/' | '@' | ':' | '-'))
        || glued_after('.')
        || glued_after('/')
        || glued_after('@')
        || glued_after('-')
}

/// The case of what was written carries over. An abbreviation is written in capitals
/// anywhere, so it only counts at the start of a sentence: `ASAP` mid-sentence comes out
/// as `as soon as possible`.
fn cased(to: &str, chars: &[char], start: usize, end: usize) -> String {
    let letters: Vec<char> = chars[start..end]
        .iter()
        .copied()
        .filter(|c| c.is_alphabetic())
        .collect();
    let abbreviation = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
    if chars[start].is_uppercase() && (!abbreviation || sentence_start(chars, start)) {
        let mut c = to.chars();
        c.next()
            .map(|first| first.to_uppercase().chain(c).collect())
            .unwrap_or_default()
    } else {
        to.to_string()
    }
}

fn sentence_start(chars: &[char], i: usize) -> bool {
    chars[..i]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_none_or(|c| matches!(c, '.' | '!' | '?' | '…'))
}

/// `3pm`, `3 PM`, `11:30 a.m.` in the 24-hour form.
fn time_at(lower: &[char], start: usize) -> Option<(usize, String)> {
    let digits = |from: usize| {
        lower[from..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
    };
    let n = digits(start);
    if !(1..=2).contains(&n) {
        return None;
    }
    let number = |from: usize, len: usize| -> u32 {
        lower[from..from + len]
            .iter()
            .fold(0, |v, c| v * 10 + c.to_digit(10).unwrap_or(0))
    };
    let hour = number(start, n);
    let mut j = start + n;
    let mut minute = 0;
    if lower.get(j) == Some(&':') && digits(j + 1) == 2 {
        minute = number(j + 1, 2);
        j += 3;
    }
    if lower.get(j) == Some(&' ') {
        j += 1;
    }
    let (pm, len) = [
        ("am", false, 2),
        ("pm", true, 2),
        ("a.m.", false, 4),
        ("p.m.", true, 4),
    ]
    .iter()
    .find_map(|(suffix, pm, len)| {
        let s: Vec<char> = suffix.chars().collect();
        (lower.get(j..j + len) == Some(&s[..])).then_some((*pm, *len))
    })?;
    let end = j + len;
    if lower.get(end).is_some_and(|&c| is_word(c)) || !(1..=12).contains(&hour) || minute > 59 {
        return None;
    }
    let hour = match (hour, pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, true) => h + 12,
        (h, false) => h,
    };
    Some((end, format!("{hour}:{minute:02}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> String {
        prepare(text).text
    }

    macro_rules! rewrites {
        ($($name:ident: $from:expr => $to:expr,)*) => {
            $(#[test] fn $name() { assert_eq!(plain($from), $to); })*
        };
    }

    macro_rules! untouched {
        ($($name:ident: $text:expr,)*) => {
            $(#[test] fn $name() { assert_eq!(plain($text), $text); })*
        };
    }

    rewrites! {
        staging_env: "The staging env is down" => "The test server is down",
        staging_environment: "Deploy to the staging environment" => "Deploy to the test server",
        staging_alone: "It works on staging, not on prod" => "It works on test server, not on production server",
        logs: "the logs are useless" => "the log files are useless",
        logs_at_the_end: "Check the logs." => "Check the log files.",
        pod_in_a_cluster: "the kubernetes pod keeps crashing" => "the kubernetes container keeps crashing",
        pods_in_k8s: "k8s pods restart" => "k8s containers restart",
        onboarding_flow: "Review the onboarding flow" => "Review the user onboarding process",
        circle_back: "Will circle back after lunch" => "I will get back to you after lunch",
        circle_back_with_subject: "We will circle back" => "We will get back to you",
        looking_into_it: "It failed, looking into it" => "It failed, I am looking into it",
        lets_circle_back: "Let's circle back tomorrow" => "Let's come back to this tomorrow",
        heads_up: "Heads up: the API is down" => "Note: the API is down",
        heads_up_opening: "Heads up the API is down" => "Note: the API is down",
        heads_up_noun: "Thanks for the heads-up" => "Thanks for the warning",
        rate_limit: "the API rate limit is 100" => "the API request limit is 100",
        ping_a_name: "ping Alex if it's urgent" => "write to Alex if it's urgent",
        ping_me: "Ping me later" => "Write to me later",
        pinged_them: "I pinged them" => "I wrote to them",
        hop_on_a_call: "Can we hop on a quick call?" => "Can we have a quick call?",
        sync_on: "to sync on the roadmap" => "to discuss the roadmap",
        ship_this: "We wanna ship this" => "We want to publish this",
        ship_it: "Ship it!" => "Publish it!",
        the_deploy: "the deploy failed" => "the deployment failed",
        the_build: "the build is flaky" => "the compilation is unstable",
        blocker: "This is a blocker" => "This is a critical problem",
        hotfix: "We need a hotfix" => "We need an urgent fix",
        rollback: "Do a rollback" => "Do a return to the previous version",
        standup: "See you at standup" => "See you at daily meeting",
        stand_up: "See you at the stand-up" => "See you at the daily meeting",
        backlog: "Add it to the backlog" => "Add it to the task list",
        bug: "I can't repro the bug" => "I can't reproduce the error",
        bug_in_the_app: "There is a bug in the app" => "There is an error in the app",
        merging_now: "LGTM, merging now" => "Approved, adding it to the main branch now",
        nudge: "could you nudge them?" => "could you remind them?",
        eta: "What's the ETA?" => "What's the deadline?",
        wip: "It's WIP" => "It's unfinished",
        docs: "Read the docs first" => "Read the documentation first",
        pto: "I'm on PTO" => "I'm on vacation",
        time_pm: "a call at 3pm" => "a call at 15:00",
        time_spaced: "at 3 PM today" => "at 15:00 today",
        time_minutes: "at 11:30 a.m. sharp" => "at 11:30 sharp",
        time_midnight: "until 12am" => "until 0:00",
        time_noon: "at 12 pm" => "at 12:00",
    }

    untouched! {
        the_ship_sailed: "The ship sailed at dawn.",
        ship_the_goods: "We ship the goods by sea.",
        logs_in: "He logs in every morning.",
        log_in: "Please log in first.",
        logs_out: "The app logs out after an hour.",
        pea_pod: "Peas grow in a pea pod.",
        build_a_house: "They build a house every year.",
        staging_a_play: "They are staging a play.",
        staging_area: "Meet at the staging area.",
        ping_is_high: "My ping is high today.",
        ping_the_server: "Ping the server first.",
        looking_into_it_with_subject: "I'm looking into it.",
        bug_in_a_word: "Debugging takes time.",
        time_without_suffix: "See you at 3 tomorrow.",
        not_a_time: "Room 15 pmol and 13pm.",
        repo_in_an_address: "Clone github.com/acme/repo please.",
        google_docs: "Share the Google Docs link.",
        bug_in_the_soup: "There is a bug in my soup.",
        bug_on_the_wall: "A bug crawled up the wall.",
        build_quality: "The build quality of this phone is great.",
        build_of_the_house: "The build of the house took a year.",
        heads_up_with_a_name: "Heads up, Jack, the ball is coming!",
        heads_up_with_a_comma: "Heads up, the ball!",
        heads_up_mid_sentence: "He gave me heads up yesterday.",
    }

    #[test]
    fn terms_point_at_both_texts() {
        let text = "Heads up: the staging env logs are useless, ping Alex at 3pm.";
        let prepared = prepare(text);
        assert_eq!(
            prepared.text,
            "Note: the test server log files are useless, write to Alex at 15:00."
        );
        let terms: Vec<(&str, &str, &str)> = prepared
            .terms
            .iter()
            .map(|t| {
                (
                    &text[t.original.clone()],
                    &prepared.text[t.prepared.clone()],
                    t.meaning.as_str(),
                )
            })
            .collect();
        assert_eq!(
            terms,
            [
                ("Heads up:", "Note:", "предупреждаю, к сведению"),
                ("staging env", "test server", "стейджинг, тестовый сервер"),
                ("logs", "log files", "логи, журналы"),
                ("ping", "write to", "напиши, тегни"),
                ("3pm", "15:00", "15:00"),
            ]
        );
        assert!(prepared.terms.iter().all(|t| t.kind == SpanKind::Slang));
    }

    #[test]
    fn abbreviations_keep_their_kind() {
        let prepared = prepare("BTW, lmk asap");
        assert_eq!(prepared.text, "By the way, let me know as soon as possible");
        assert!(prepared
            .terms
            .iter()
            .all(|t| t.kind == SpanKind::Abbreviation));
    }
}
