#[macro_use]
mod common;

use boltay_text::{Cleanup, Config, Fillers, Snippet, Snippets};

const REQUISITES: &str = "ИНН 7700000000\nКПП 770001001\n  р/с 40702810000000000000";
const MAIL: &str = "me@example.com";
const WORK_MAIL: &str = "work@example.com";
const SIGNATURE: &str = "С уважением,\nИван";

fn entries() -> Vec<Snippet> {
    vec![
        Snippet::new("мои реквизиты", REQUISITES),
        Snippet::new("моя почта", MAIL),
        Snippet::new("моя почта рабочая", WORK_MAIL),
        Snippet::new("подпись", SIGNATURE),
        Snippet::new("моё резюме", "cv.pdf"),
        Snippet::new("", "never"),
    ]
}

fn snippets() -> Config {
    Config {
        snippets: Snippets {
            entries: entries(),
            ..Snippets::default()
        },
        ..Config::disabled()
    }
}

fn exact() -> Config {
    Config {
        snippets: Snippets {
            threshold: 1.0,
            entries: entries(),
            ..Snippets::default()
        },
        ..Config::disabled()
    }
}

cases! { snippets();
    exact_phrase: "мои реквизиты" => REQUISITES,
    punctuated: "Мои реквизиты." => REQUISITES,
    caps: "МОИ РЕКВИЗИТЫ!" => REQUISITES,
    one_letter_off: "Мой реквизиты" => REQUISITES,
    misheard_vowel: "Мои реквезиты" => REQUISITES,
    extra_space: "мои  реквизиты" => REQUISITES,
    short_trigger: "Подпись." => SIGNATURE,
    yo_folded: "Мое резюме" => "cv.pdf",
    closest_wins: "Моя почта рабочая" => WORK_MAIL,
    other_closest: "Моя почта" => MAIL,
}

untouched! { snippets();
    part_of_phrase: "Пришли мои реквизиты",
    phrase_continues: "Мои реквизиты и почта",
    too_different: "Подпиши",
    unrelated: "Привет",
    empty: "",
}

cases! { exact();
    exact_threshold_rejects_typo: "Мой реквизиты" => "Мой реквизиты",
    exact_threshold_accepts_punctuation: "Мои реквизиты." => REQUISITES,
}

cases! { Config { snippets: Snippets { threshold: 0.0, entries: entries(), ..Snippets::default() }, ..Config::disabled() };
    zero_threshold_is_clamped: "Привет" => "Привет",
    clamped_threshold_still_fuzzy: "Мои реквезиты" => REQUISITES,
}

cases! { Config { snippets: Snippets { enabled: false, entries: entries(), ..Snippets::default() }, ..Config::disabled() };
    stage_switch_off: "Мои реквизиты" => "Мои реквизиты",
}

cases! { Config { snippets: Snippets { entries: entries(), ..Snippets::default() }, ..Config::default() };
    cleanup_does_not_touch_snippet: "мои реквизиты" => REQUISITES,
    fillers_cut_before_matching: "Ээ, мои реквизиты" => REQUISITES,
}

cases! {
    Config {
        snippets: Snippets { entries: entries(), ..Snippets::default() },
        cleanup: Cleanup { enabled: true, trailing_space: true },
        fillers: Fillers::default(),
        ..Config::disabled()
    };
    trailing_space_still_applies: "Моя почта" => "me@example.com ",
}

#[test]
fn detailed_result_marks_snippets() {
    let config = snippets();
    let hit = boltay_text::process_detailed(&config, "Моя почта.");
    assert_eq!(hit.text, MAIL);
    assert!(hit.snippet);
    let miss = boltay_text::process_detailed(&config, "Пишу письмо");
    assert!(!miss.snippet);
    assert_eq!(miss.text, boltay_text::process(&config, "Пишу письмо"));
}

fn with_variables(entries: Vec<Snippet>, clipboard: &str) -> Config {
    Config {
        snippets: Snippets {
            entries,
            variables: boltay_text::Variables {
                date: "02.10.2026".into(),
                time: "14:05".into(),
                clipboard: clipboard.into(),
            },
            ..Snippets::default()
        },
        ..Config::disabled()
    }
}

cases! { with_variables(vec![
        Snippet::new("отчёт", "Отчёт за {дата}, {время}"),
        Snippet::new("ссылка", "Вот ссылка: {буфер}"),
        Snippet::new("english", "Sent {DATE} at {Time}: {clipboard}"),
        Snippet::new("скобки", "Шаблон {имя} и {дата"),
        Snippet::new("пусто", "Буфер: [{буфер}]"),
    ], "https://example.com");
    date_and_time: "Отчёт." => "Отчёт за 02.10.2026, 14:05",
    clipboard: "Ссылка" => "Вот ссылка: https://example.com",
    english_names_any_case: "English" => "Sent 02.10.2026 at 14:05: https://example.com",
    unknown_and_unclosed_stay: "Скобки" => "Шаблон {имя} и {дата",
}

cases! { with_variables(vec![Snippet::new("пусто", "Буфер: [{буфер}]")], "");
    empty_clipboard: "Пусто" => "Буфер: []",
}

#[test]
fn only_snippets_fill_variables() {
    let config = with_variables(vec![Snippet::new("отчёт", "{дата}")], "x");
    // Dictated text that is not a snippet keeps its braces.
    assert_eq!(
        boltay_text::process(&config, "скажи {дата}"),
        "скажи {дата}"
    );
}

#[test]
fn knows_when_the_clipboard_is_needed() {
    assert!(boltay_text::uses_clipboard("Вот: {Буфер}"));
    assert!(boltay_text::uses_clipboard("{clipboard}"));
    assert!(!boltay_text::uses_clipboard("{дата} {время}"));
}

#[test]
fn variables_are_not_saved() {
    let config = with_variables(Vec::new(), "secret");
    let json = serde_json::to_string(&config).unwrap();
    assert!(
        !json.contains("secret") && !json.contains("variables"),
        "{json}"
    );
}
