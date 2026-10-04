#[macro_use]
mod common;

use boltay_text::{Config, Dictionary, Replacement};

fn dictionary() -> Config {
    let entries = [
        ("релакс лаб", "Relax Lab"),
        ("гит хаб", "GitHub"),
        ("задеплой", "задеплой (deploy)"),
        ("пулл реквест", "pull request"),
        ("пулл", "pull"),
        ("джава скрипт", "JavaScript"),
        ("ёлка", "tree"),
        ("  си   плюс плюс ", "C++"),
        ("тз", "техническое задание"),
        ("", "пусто"),
    ];
    Config {
        dictionary: Dictionary {
            enabled: true,
            entries: entries
                .iter()
                .map(|(from, to)| Replacement::new(*from, *to))
                .collect(),
        },
        ..Config::disabled()
    }
}

cases! { dictionary();
    two_words: "Зайди на гит хаб" => "Зайди на GitHub",
    opening: "Гит хаб лежит" => "GitHub лежит",
    caps: "ГИТ ХАБ" => "GitHub",
    hyphen_instead_of_space: "Зайди на гит-хаб" => "Зайди на GitHub",
    extra_spaces: "Зайди на гит   хаб" => "Зайди на GitHub",
    brand: "Студия релакс лаб на связи" => "Студия Relax Lab на связи",
    lowercase_target_keeps_sentence_case: "Открой пулл реквест" => "Открой pull request",
    lowercase_target_capitalized_at_start: "Пулл реквест готов" => "Pull request готов",
    longest_wins: "Сделай пулл и пулл реквест" => "Сделай pull и pull request",
    yo_in_entry: "Наряжаем елку и ёлка" => "Наряжаем елку и tree",
    yo_in_text: "Ёлка" => "Tree",
    messy_entry: "Пишу на си плюс плюс" => "Пишу на C++",
    next_to_punctuation: "(гит хаб), гит хаб." => "(GitHub), GitHub.",
    target_with_own_caps: "Джава скрипт" => "JavaScript",
    several_in_a_row: "гит хаб гит хаб" => "GitHub GitHub",
    replacement_not_rescanned: "Задеплой" => "Задеплой (deploy)",
    abbreviation_mid_sentence: "Клиент поменял ТЗ" => "Клиент поменял техническое задание",
    abbreviation_at_start: "ТЗ готово. ТЗ отправил" => "Техническое задание готово. Техническое задание отправил",
    abbreviation_lowercase: "скинь тз" => "скинь техническое задание",
}

untouched! { dictionary();
    glued_word: "Открой гитхаб",
    inflected: "Работаю в релакс лабе",
    word_inside_word: "Магит хаб",
    word_prefix: "Пуллинг",
    across_line_break: "Гит\nхаб",
    empty_entry_ignored: "Здесь пусто",
    empty_text: "",
}

cases! { Config { dictionary: Dictionary { enabled: false, ..dictionary().dictionary }, ..Config::disabled() };
    stage_switch_off: "гит хаб" => "гит хаб",
}

#[test]
fn spans_point_at_both_texts() {
    let entries = [
        Replacement::new("тз", "техническое задание"),
        Replacement::new("гит хаб", "GitHub"),
    ];
    let text = "Скинь ТЗ, а код — на Гит хаб.";
    let (out, spans) = boltay_text::replace_spans(&entries, text);
    assert_eq!(out, "Скинь техническое задание, а код — на GitHub.");
    let pairs: Vec<(usize, &str, &str)> = spans
        .iter()
        .map(|s| (s.entry, &text[s.from.clone()], &out[s.to.clone()]))
        .collect();
    assert_eq!(
        pairs,
        [(0, "ТЗ", "техническое задание"), (1, "Гит хаб", "GitHub")]
    );
    assert_eq!(boltay_text::replace(&entries, text), out);
}

#[test]
fn no_spans_without_matches() {
    let entries = [Replacement::new("тз", "техническое задание")];
    let (out, spans) = boltay_text::replace_spans(&entries, "Всё готово");
    assert_eq!(out, "Всё готово");
    assert!(spans.is_empty());
}
