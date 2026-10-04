#[macro_use]
mod common;

use boltay_text::{process, Config, Dictionary, Fillers, Profanity, Replacement, SOFT_FILLERS};

fn everything() -> Config {
    Config {
        fillers: Fillers {
            soft: SOFT_FILLERS.iter().map(|s| s.to_string()).collect(),
            ..Fillers::default()
        },
        profanity: Profanity::Soften,
        dictionary: Dictionary {
            enabled: true,
            entries: vec![Replacement::new("гит хаб", "GitHub")],
        },
        ..Config::default()
    }
}

cases! { Config::default();
    default_trims_and_fixes_spaces: "  ээ,   привет  , мир  " => "Привет, мир",
    default_keeps_meaning_words: "Ну, я пошёл" => "Ну, я пошёл",
    default_new_line: "привет. новая строка. пока" => "Привет.\nПока",
    empty: "" => "",
    only_hesitation: "Эээ..." => "",
    // A pause, "Абзац", a pause used to give six empty lines.
    pause_abzac_pause: "Что ещё?\n\nАбзац.\n\nКороче, едем дальше." => "Что ещё?\n\nКороче, едем дальше.",
    paragraph_then_line: "Раз. Абзац. Новая строка. Два." => "Раз.\n\nДва.",
    hesitation_before_a_pause: "Эээ.\n\nПривет." => "Привет.",
    hesitation_after_a_pause: "Привет.\n\nЭээ." => "Привет.",
    hesitations_only: "Эээ.\n\nМмм." => "",
    hesitation_between_pauses: "Раз.\n\nЭээ.\n\nДва." => "Раз.\n\nДва.",
    lone_break_still_pasted: "Абзац." => "\n\n",
    mark_after_a_pause: "Как дела\n\nвопросительный знак" => "Как дела?",
    mark_after_a_pause_keeps_the_next_paragraph: "Итак\n\nдвоеточие.\n\nТри правки." => "Итак:\n\nТри правки.",
}

cases! { everything();
    all_stages: "Короче, блять, я, ээ, опоздал. Новая строка. Гит хаб лежит." => "Блин, я опоздал.\nGitHub лежит.",
    filler_chain_then_profanity: "Ну, типа, я не знаю, это самое, короче, пиздец какой-то." => "Я не знаю, капец какой-то.",
    aside_survives_every_stage: "Проверь, пожалуйста, ну, этот код." => "Проверь, пожалуйста, этот код.",
    filler_then_profanity: "Ну, пиздец, опять" => "Капец, опять",
    delete_after_cleanup_of_fillers: "Ээ, я опоздаю. Удали последнее. Буду." => "Буду.",
    dictionary_after_new_line: "Смотри. Новая строка. гит хаб" => "Смотри.\nGitHub",
    filler_before_a_break: "Так, короче, абзац. Дальше про деньги." => "Так.\n\nДальше про деньги.",
    filler_before_a_new_line: "Всё, короче, новая строка. Второй пункт." => "Всё.\nВторой пункт.",
}

cases! { Config { profanity: Profanity::Remove, ..Config::default() };
    remove_then_capitalize: "блять, опять опоздал" => "Опять опоздал",
    remove_mid_sentence: "я, сука, опоздал." => "Я опоздал.",
}

#[test]
fn disabled_returns_input_verbatim() {
    for text in [
        "",
        "  ээ,  блять  ",
        "раз\r\n\n\nдва",
        "Новая строка. Удали последнее.",
    ] {
        assert_eq!(process(&Config::disabled(), text), text);
    }
}

#[test]
fn processing_twice_changes_nothing() {
    let config = everything();
    for text in [
        "Короче, блять, я, ээ, опоздал. Новая строка. Гит хаб лежит.",
        "  ээ,   привет  , мир  ",
        "Я, типа, не знаю, ну?",
        "Hello, uh, new paragraph, you know, fucking great",
        "Первое. Второе. Удали последнее. Третье.",
    ] {
        let once = process(&config, text);
        assert_eq!(process(&config, &once), once, "input: {text:?}");
    }
}

mod config {
    use super::*;

    #[test]
    fn empty_json_is_default() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_json_fills_defaults() {
        let config: Config = serde_json::from_str(r#"{"profanity": "soften"}"#).unwrap();
        assert_eq!(config.profanity, Profanity::Soften);
        assert!(config.fillers.hard);
        assert!(config.cleanup.enabled);
    }

    #[test]
    fn partial_nested_json_fills_defaults() {
        let config: Config = serde_json::from_str(r#"{"fillers": {"soft": ["ну"]}}"#).unwrap();
        assert!(config.fillers.enabled);
        assert!(config.fillers.hard);
        assert_eq!(config.fillers.soft, ["ну"]);
        let config: Config =
            serde_json::from_str(r#"{"snippets": {"entries": [{"trigger": "x"}]}}"#).unwrap();
        assert_eq!(config.snippets.threshold, 0.85);
        assert_eq!(config.snippets.entries[0].text, "");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{"future_stage": true, "fillers": {"hard": false, "new_flag": 1}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.fillers.hard);
    }

    #[test]
    fn modes_are_lowercase() {
        for (json, mode) in [
            (r#""keep""#, Profanity::Keep),
            (r#""mask""#, Profanity::Mask),
            (r#""remove""#, Profanity::Remove),
            (r#""soften""#, Profanity::Soften),
        ] {
            assert_eq!(serde_json::from_str::<Profanity>(json).unwrap(), mode);
            assert_eq!(serde_json::to_string(&mode).unwrap(), json);
        }
        assert!(serde_json::from_str::<Profanity>(r#""Soften""#).is_err());
    }

    #[test]
    fn round_trips() {
        let config = everything();
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), config);
    }

    #[test]
    fn soft_fillers_off_by_default() {
        assert!(Config::default().fillers.soft.is_empty());
        assert_eq!(Config::default().profanity, Profanity::Keep);
        assert!(!Config::default().commands.spoken_punctuation);
    }
}
