use crate::config::Profanity;
use crate::token::{self, cut, fold, match_case, Kind, Token};

pub(crate) fn apply(mode: Profanity, text: &str) -> String {
    if mode == Profanity::Keep {
        return text.to_string();
    }
    let mut tokens = token::tokenize(text);
    let mut i = 0;
    while i < tokens.len() {
        if !tokens[i].is(Kind::Word) || !tokens[i].text.split('-').any(is_profane) {
            i += 1;
            continue;
        }
        let parts: Vec<&str> = tokens[i].text.split('-').collect();
        let replaced: Vec<String> = match mode {
            Profanity::Keep => unreachable!(),
            Profanity::Mask => parts.iter().map(|p| masked_if_profane(p)).collect(),
            Profanity::Soften => {
                let context = Context {
                    interjection: token::left_edge(&tokens, i).is_boundary()
                        && token::right_edge(&tokens, i + 1).is_boundary(),
                    next: following_words(&tokens, i + 1),
                };
                parts
                    .iter()
                    .map(|&p| match is_profane(p) {
                        true => soften(p, &context).unwrap_or_else(|| mask(p)),
                        false => p.to_string(),
                    })
                    .collect()
            }
            Profanity::Remove => {
                if parts.iter().all(|p| is_profane(p)) {
                    i = cut(&mut tokens, i, i + 1);
                    continue;
                }
                parts
                    .iter()
                    .filter(|p| !is_profane(p))
                    .map(|p| p.to_string())
                    .collect()
            }
        };
        tokens[i].text = replaced.join("-");
        i += 1;
    }
    token::tidy(&mut tokens);
    token::render(&tokens)
}

fn masked_if_profane(word: &str) -> String {
    if is_profane(word) {
        mask(word)
    } else {
        word.to_string()
    }
}

/// `блять` -> `б**ть`: first letter, last letter, and a trailing soft sign keeps its consonant.
fn mask(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n <= 2 {
        return chars[..1]
            .iter()
            .chain(std::iter::repeat_n(&'*', n - 1))
            .collect();
    }
    let tail = if n > 3 && matches!(chars[n - 1], 'ь' | 'ъ' | 'Ь' | 'Ъ') {
        2
    } else {
        1
    };
    let mut out = String::new();
    out.push(chars[0]);
    out.extend(std::iter::repeat_n('*', n - 1 - tail));
    out.extend(&chars[n - tail..]);
    out
}

/// Words that contain a root below but are not swearing.
const EXCEPTIONS: &[&str] = &[
    "корабл",
    "рубл",
    "употребл",
    "потребл",
    "оскорбл",
    "страху",
    "застраху",
    "подстраху",
    "сабл",
    "команд",
    "скипидар",
    "хлебал",
    "мандарин",
    "мандат",
    "мандраж",
    "себ",
    "веб",
    "сукн",
    "сукин",
    "ебен",
    "ебург",
    "блях",
    "бляш",
    "сучк",
    "сучок",
    "assum",
    "class",
    "pass",
    "assist",
    "shiitake",
    "shitake",
];

/// Russian prefixes a root may hide behind: `на-хуй`, `за-еб-ал`, `вы-бляд-ок`.
const PREFIXES: &[&str] = &[
    "без", "бес", "в", "во", "въ", "вь", "вз", "взъ", "вс", "вы", "до", "за", "из", "изъ", "ис",
    "на", "над", "надъ", "не", "недо", "ни", "о", "об", "обо", "объ", "от", "ото", "отъ", "пере",
    "по", "под", "подъ", "пре", "при", "про", "раз", "разъ", "рас", "с", "со", "съ", "сь", "у",
];

/// Forms of `сука`. Not `сук` (a branch) and not `сучья` (branches).
const SUKA: &[&str] = &["сука", "суки", "суке", "суку", "сукой", "сукою", "ссука"];

/// Stems that only ever start a word.
const LEADING: &[&str] = &[
    "мудак",
    "мудач",
    "мудил",
    "мудозвон",
    "пидор",
    "пидар",
    "пидр",
    "залуп",
    "долбоеб",
    "долбаеб",
    "далбаеб",
    "сучар",
];

const ASS: &[&str] = &[
    "ass",
    "asses",
    "asshole",
    "assholes",
    "asshat",
    "dumbass",
    "jackass",
    "smartass",
    "arse",
    "arsehole",
    "arseholes",
];

pub(crate) fn is_profane(word: &str) -> bool {
    let w = fold(word);
    if EXCEPTIONS.iter().any(|e| w.starts_with(e)) {
        return false;
    }
    w.contains("fuck")
        || (w.contains("shit") && !w.contains("shita"))
        || w.contains("bitch")
        || w.starts_with("cunt")
        || ASS.contains(&w.as_str())
        || SUKA.contains(&w.as_str())
        || LEADING.iter().any(|s| w.starts_with(s))
        || rooted(&w, None, 0)
}

/// A root at the very start of the word or after one or two prefixes. Deeper than that is
/// how `страхуй` and `употреблять` turn into false alarms. A one-letter prefix never
/// stacks with another: `с-у-хую` and `вс-у-хую` are `сухую` and `всухую`.
fn rooted(w: &str, prefix: Option<&str>, depth: usize) -> bool {
    let short = |p: &str| p.chars().count() == 1;
    root_here(w, prefix)
        || (depth < 2
            && !prefix.is_some_and(short)
            && PREFIXES.iter().any(|p| {
                (depth == 0 || !short(p))
                    && w.strip_prefix(p).is_some_and(|rest| {
                        rest.chars().count() >= 2 && rooted(rest, Some(p), depth + 1)
                    })
            }))
}

fn root_here(w: &str, prefix: Option<&str>) -> bool {
    let mut chars = w.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some('х'), Some('у'), Some('й' | 'я' | 'е' | 'и' | 'ю')) => true,
        (Some('б'), Some('л'), Some('я')) => matches!(chars.next(), None | Some('д' | 'т')),
        // A consonant prefix before `еб` takes a hard sign in swearing (`съебать`, `въебать`),
        // without it it is `себе`, `веб`, `отсебятина`.
        (Some('е'), Some('б'), _) => {
            prefix.is_none_or(|p| p.ends_with(['а', 'е', 'и', 'о', 'у', 'ы', 'ъ', 'ь']))
        }
        _ => w.starts_with("пизд"),
    }
}

const INTERJECTIONS: &[(&str, &str)] = &[("ебать", "ёлки"), ("сука", "блин")];

const EXACT: &[(&str, &str)] = &[
    ("бля", "блин"),
    ("блять", "блин"),
    ("блядь", "блин"),
    ("пиздец", "капец"),
    ("пиздеца", "капца"),
    ("пиздецу", "капцу"),
    ("пиздецом", "капцом"),
    ("пиздеце", "капце"),
    ("пиздеж", "трёп"),
    ("хуй", "фиг"),
    ("хуя", "фига"),
    ("хую", "фигу"),
    ("хуем", "фигом"),
    ("нахуй", "нафиг"),
    ("нахуя", "нафига"),
    ("похуй", "пофиг"),
    ("нихуя", "нифига"),
    ("дохуя", "дофига"),
    ("нехуй", "нефиг"),
    ("заебал", "достал"),
    ("заебала", "достала"),
    ("заебали", "достали"),
    ("заебало", "достало"),
    ("заебать", "достать"),
    ("заебись", "зашибись"),
    ("поебать", "пофиг"),
    ("съебал", "свалил"),
    ("съебался", "свалил"),
    ("съебалась", "свалила"),
    ("съебались", "свалили"),
    ("съебаться", "свалить"),
    ("съебись", "свали"),
    ("ебанулся", "рехнулся"),
    ("ебанулась", "рехнулась"),
    ("ебанулись", "рехнулись"),
    ("мудак", "придурок"),
    ("уебок", "урод"),
    ("уебка", "урода"),
    ("уебку", "уроду"),
    ("уебком", "уродом"),
    ("уебки", "уроды"),
    ("уебков", "уродов"),
    ("долбоеб", "придурок"),
    ("долбоеба", "придурка"),
    ("долбоебу", "придурку"),
    ("долбоебом", "придурком"),
    ("долбоебы", "придурки"),
    ("долбоебов", "придурков"),
    ("fuck", "heck"),
    ("fucking", "freaking"),
    ("fuckin", "freakin"),
    ("fucked", "screwed"),
    ("shit", "crap"),
    ("shitty", "crappy"),
    ("bullshit", "nonsense"),
    ("bitch", "jerk"),
    ("bitches", "jerks"),
    ("asshole", "jerk"),
    ("assholes", "jerks"),
    ("ass", "butt"),
    ("dumbass", "dummy"),
    ("jackass", "jerk"),
];

const ADJ: &[&str] = &[
    "ый", "ий", "ая", "ое", "ые", "ого", "ой", "ому", "ым", "ых", "ую", "ыми", "ом",
];
const ADJ_ADV: &[&str] = &[
    "о", "ый", "ий", "ая", "ое", "ые", "ого", "ой", "ому", "ым", "ых", "ую", "ыми", "ом",
];
const PAST: &[&str] = &["л", "ла", "ли", "ло", "ть"];

/// Stem swaps that keep the ending: `пиздатая` -> `клёвая`, `спиздили` -> `стырили`.
const STEMS: &[(&str, &str, &[&str])] = &[
    ("хуйн", "фигн", &["я", "и", "е", "ю", "ей", "ею"]),
    (
        "охуе",
        "офиге",
        &[
            "ть",
            "л",
            "ла",
            "ли",
            "ло",
            "нно",
            "нный",
            "нная",
            "нное",
            "нные",
            "нного",
            "нной",
            "нному",
            "нным",
            "нных",
            "нную",
            "вший",
            "вшая",
            "вшие",
            "вшего",
            "ваю",
            "ваешь",
            "вает",
            "ваем",
            "ваете",
            "вают",
        ],
    ),
    (
        "охуи",
        "офиги",
        &["тельно", "тельный", "тельная", "тельное", "тельные"],
    ),
    ("хуев", "фигов", ADJ_ADV),
    (
        "похуист",
        "пофигист",
        &["", "а", "у", "ом", "ы", "ов", "ка", "ки"],
    ),
    ("пиздат", "клёв", ADJ_ADV),
    (
        "спизд",
        "стыр",
        &["ил", "ила", "или", "ило", "ить", "ит", "ят"],
    ),
    (
        "распиздя",
        "раздолба",
        &["й", "я", "ю", "ем", "и", "ев", "йство", "йка"],
    ),
    (
        "заеба",
        "задолба",
        &["лся", "лась", "лись", "лось", "ться", "вшись"],
    ),
    ("наеба", "обману", PAST),
    ("ебанут", "долбанут", ADJ),
    ("ебанн", "долбанн", ADJ),
    ("ебан", "долбан", ADJ),
    ("ебану", "грохну", PAST),
    (
        "ебну",
        "грохну",
        &["л", "ла", "ли", "ло", "ть", "лся", "лась", "лись", "ться"],
    ),
    (
        "мудак",
        "придурк",
        &["а", "у", "ом", "е", "и", "ов", "ам", "ами", "ах"],
    ),
    ("мудил", "чудил", &["а", "ы", "е", "у", "ой", "о"]),
    (
        "выебыва",
        "выпендрива",
        &[
            "ться",
            "ется",
            "ются",
            "ешься",
            "юсь",
            "ешь",
            "ет",
            "ют",
            "лся",
            "лась",
            "лись",
        ],
    ),
    ("уебищ", "уродищ", &["е", "а", "у", "ем", "и"]),
    ("пиздюл", "люл", &["и", "ей", "ину"]),
];

struct Context {
    /// Set off by punctuation on both sides: `Сука, опять`, not `эта сука`.
    interjection: bool,
    /// Up to two folded words that follow in the same phrase.
    next: Vec<String>,
}

fn following_words(tokens: &[Token], mut i: usize) -> Vec<String> {
    let mut words = Vec::new();
    while words.len() < 2 && tokens.get(i).is_some_and(|t| t.is(Kind::Space)) {
        match tokens.get(i + 1) {
            Some(t) if t.is(Kind::Word) => words.push(fold(&t.text)),
            _ => break,
        }
        i += 2;
    }
    words
}

/// `fuck you` has no polite twin: `heck you` reads as a joke, so it gets masked.
const FUCK_OBJECTS: &[&str] = &[
    "you", "u", "off", "yourself", "him", "her", "them", "it", "me", "this", "that",
];

/// `ебёт мозги` -> `выносит мозги`, also with `мне` or `тебе` in between.
const BRAINS: &[(&str, &str)] = &[
    ("ебу", "выношу"),
    ("ебешь", "выносишь"),
    ("ебет", "выносит"),
    ("ебем", "выносим"),
    ("ебете", "выносите"),
    ("ебут", "выносят"),
    ("ебать", "выносить"),
    ("ебал", "выносил"),
    ("ебала", "выносила"),
    ("ебали", "выносили"),
    ("еби", "выноси"),
    ("ебите", "выносите"),
];

fn before_brains(next: &[String]) -> bool {
    let is_brains = |w: &String| matches!(w.as_str(), "мозги" | "мозг");
    let is_dative = |w: &String| {
        matches!(
            w.as_str(),
            "мне" | "тебе" | "ему" | "ей" | "нам" | "вам" | "им" | "всем" | "себе"
        )
    };
    match next {
        [first, ..] if is_brains(first) => true,
        [first, second] => is_dative(first) && is_brains(second),
        _ => false,
    }
}

/// A milder word in the same form, or `None` when there is no honest one and the caller
/// should mask instead.
fn soften(word: &str, context: &Context) -> Option<String> {
    let w = fold(word);
    if w == "fuck"
        && context
            .next
            .first()
            .is_some_and(|n| FUCK_OBJECTS.contains(&n.as_str()))
    {
        return None;
    }
    if before_brains(&context.next) {
        if let Some((_, good)) = BRAINS.iter().find(|(bad, _)| *bad == w) {
            return Some(match_case(word, good));
        }
    }
    let lookup = |table: &[(&str, &str)]| {
        table
            .iter()
            .find(|(bad, _)| *bad == w)
            .map(|(_, good)| good.to_string())
    };
    let soft = context
        .interjection
        .then(|| lookup(INTERJECTIONS))
        .flatten()
        .or_else(|| lookup(EXACT))
        .or_else(|| {
            STEMS.iter().find_map(|(stem, good, endings)| {
                let ending = w.strip_prefix(stem)?;
                endings.contains(&ending).then(|| format!("{good}{ending}"))
            })
        })?;
    Some(match_case(word, &soft))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_keep_first_and_last_letter() {
        assert_eq!(mask("блять"), "б**ть");
        assert_eq!(mask("блядь"), "б**дь");
        assert_eq!(mask("бля"), "б*я");
        assert_eq!(mask("хуй"), "х*й");
        assert_eq!(mask("Пиздец"), "П****ц");
        assert_eq!(mask("fuck"), "f**k");
        assert_eq!(mask("ёб"), "ё*");
    }
}
