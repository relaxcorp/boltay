use std::sync::OnceLock;

/// Russian case endings the model puts on a transliterated name: `Кубернетов`, `в Фигме`.
const ENDINGS: &[&str] = &[
    "ами", "ями", "ов", "ом", "ам", "ах", "ой", "ей", "ем", "а", "у", "е", "ы", "и", "ю", "я",
];

/// Puts the technical names of the English source back in Latin letters where the model
/// spelled them in Cyrillic: `Kubernetes` came out as `Кубернетов`, `API` as `АПИ`. Only
/// abbreviations, names with capitals inside and known brand names count, so `Alex` stays
/// `Алекс`. A Cyrillic word matches when, with a case ending cut off, its transliteration
/// is within an edit or two of the name.
pub(crate) fn restore(source: &str, translation: &str) -> String {
    let names = names(source);
    if names.is_empty() {
        return translation.to_string();
    }
    let mut out = String::with_capacity(translation.len());
    let mut rest = translation;
    while let Some(start) = rest.find(is_cyrillic) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !is_cyrillic(c)).unwrap_or(rest.len());
        let word = &rest[..end];
        match names.iter().find(|name| spells(word, name)) {
            Some(name) => out.push_str(name),
            None => out.push_str(word),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{400}'..='\u{4ff}')
}

/// Latin words of the source worth keeping as they are.
fn names(source: &str) -> Vec<&str> {
    let brands = brands();
    let mut out: Vec<&str> = source
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 2 && w.chars().any(|c| c.is_ascii_alphabetic()))
        .filter(|w| {
            let upper = w.chars().filter(char::is_ascii_uppercase).count();
            // `API`, `PR`; `GitHub`, `iPhone`; `Kubernetes` if it is a known brand.
            (upper == w.len() && w.len() <= 6)
                || w.chars().skip(1).any(|c| c.is_ascii_uppercase())
                || brands.iter().any(|b| b.eq_ignore_ascii_case(w))
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Single-word brand names written in Latin letters, from the dictation brand lists.
fn brands() -> &'static [String] {
    static BRANDS: OnceLock<Vec<String>> = OnceLock::new();
    BRANDS.get_or_init(|| {
        boltay_text::default_brands()
            .into_iter()
            .chain(boltay_text::default_ambiguous())
            .map(|t| t.name)
            .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric()))
            .collect()
    })
}

fn spells(word: &str, name: &str) -> bool {
    let target = squash(&name.to_lowercase());
    let budget = name.len() / 4;
    let lower = word.to_lowercase();
    std::iter::once(lower.as_str())
        .chain(ENDINGS.iter().filter_map(|e| lower.strip_suffix(e)))
        .filter(|stem| stem.chars().count() >= 2)
        .any(|stem| {
            let latin = squash(&transliterate(stem));
            latin.chars().next() == target.chars().next() && distance(&latin, &target) <= budget
        })
}

fn transliterate(word: &str) -> String {
    word.chars()
        .map(|c| match c {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' => "g",
            'д' => "d",
            'е' | 'ё' | 'э' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' | 'ы' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "ts",
            'ч' => "ch",
            'ш' => "sh",
            'щ' => "sch",
            'ю' => "yu",
            'я' => "ya",
            _ => "",
        })
        .collect()
}

/// Spelling differences that do not tell English and Russian apart: `c`/`k`, `j`/`дж`,
/// `y`/`и`, doubled letters.
fn squash(latin: &str) -> String {
    let latin = latin
        .replace("ph", "f")
        .replace("dzh", "j")
        .replace("zh", "j")
        .replace(['c', 'q'], "k")
        .replace('x', "ks")
        .replace('w', "v")
        .replace('y', "i");
    let mut out = String::with_capacity(latin.len());
    for c in latin.chars() {
        if !out.ends_with(c) {
            out.push(c);
        }
    }
    out
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (diag + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brings_names_back() {
        assert_eq!(
            restore(
                "IDK why the Kubernetes pod keeps crashing",
                "Не знаю, почему контейнер Кубернетов падает"
            ),
            "Не знаю, почему контейнер Kubernetes падает"
        );
        assert_eq!(
            restore("the API rate limit", "Предел запросов АПИ"),
            "Предел запросов API"
        );
        assert_eq!(
            restore("leave comments in Figma", "оставьте комментарии в Фигме"),
            "оставьте комментарии в Figma"
        );
        assert_eq!(
            restore("Open it on GitHub", "Открой его на Гитхабе"),
            "Открой его на GitHub"
        );
        assert_eq!(
            restore("join the Zoom call", "присоединятся к Зум-звонку"),
            "присоединятся к Zoom-звонку"
        );
        assert_eq!(
            restore("Docker runs on AWS", "Докер бежит по АВС"),
            "Docker бежит по AWS"
        );
    }

    #[test]
    fn leaves_people_and_plain_words() {
        for (source, translation) in [
            ("write to Alex", "напиши Алексу"),
            (
                "Anna said the build is flaky",
                "Анна сказала, что сборка нестабильна",
            ),
            ("Please review it", "Пожалуйста, проверь"),
            ("the API is down", "сервер упал"),
            (
                "Ask Maria to send it to John",
                "Попроси Марию отправить Джону",
            ),
            ("Anna and Mike will join", "Анна и Майк присоединятся"),
        ] {
            assert_eq!(restore(source, translation), translation, "{source}");
        }
    }
}
