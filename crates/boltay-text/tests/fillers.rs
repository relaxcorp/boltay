#[macro_use]
mod common;

use boltay_text::{Config, Fillers, SOFT_FILLERS};

fn only(soft: &[&str]) -> Config {
    Config {
        fillers: Fillers {
            soft: soft.iter().map(|s| s.to_string()).collect(),
            ..Fillers::default()
        },
        ..Config::disabled()
    }
}

fn hard() -> Config {
    only(&[])
}

fn soft() -> Config {
    only(SOFT_FILLERS)
}

mod hesitations {
    use super::*;

    cases! { hard();
        inside_sentence: "Я ээ пошёл домой" => "Я пошёл домой",
        opening_with_comma: "Ээ, я пошёл" => "Я пошёл",
        opening_without_comma: "Ээ я пошёл" => "Я пошёл",
        between_commas: "Я пошёл, ээ, домой" => "Я пошёл домой",
        before_full_stop: "Я пошёл домой, эээ." => "Я пошёл домой.",
        opening_second_sentence: "Я пришёл. Эм, потом ушёл." => "Я пришёл. Потом ушёл.",
        long_mmm: "Ммм, вкусно" => "Вкусно",
        hyphenated: "Э-э-э, я не помню" => "Я не помню",
        hyphenated_m: "М-м, нет" => "Нет",
        before_ellipsis: "Эээ… я не знаю" => "Я не знаю",
        several_in_a_row: "Ээ, ммм, я пошёл" => "Я пошёл",
        alone_with_full_stop: "Ээ." => "",
        alone_with_question: "Ээ?" => "",
        before_question: "Ты придёшь ээ?" => "Ты придёшь?",
        english_uh: "So, uh, I think" => "So I think",
        english_um_opening: "Um, what?" => "What?",
        english_umm: "I, umm, forgot" => "I forgot",
        english_er: "Er, hello" => "Hello",
        english_erm: "It was erm fine" => "It was fine",
        english_hmm: "Hmm, let me think" => "Let me think",
        keeps_comma_after_word: "Когда я пришёл ээ, все ушли" => "Когда я пришёл, все ушли",
        keeps_comma_before_chto: "Я знаю, ээ, что ты прав" => "Я знаю, что ты прав",
        keeps_comma_before_kotory: "Дом, эм, который мы видели" => "Дом, который мы видели",
        soft_list_is_off: "Ну, ээ, короче, я пошёл" => "Ну короче, я пошёл",
    }

    untouched! { hard();
        millimetres: "Толщина 5 мм",
        aa_batteries: "Батарейки АА",
        shouted_ee: "ЭЭ — это код",
        err_is_a_verb: "To err is human",
        er_in_caps: "The ER was full",
        single_e: "Э, нет, так не пойдёт",
        single_a: "А я пошёл",
        emal: "Эмаль и эмоции",
        mama: "Мама мыла раму",
        umbrella: "Take an umbrella",
        hm_short: "Hm, fine",
        uh_huh: "Uh-huh, sure",
    }

    cases! { Config { fillers: Fillers { hard: false, ..Fillers::default() }, ..Config::disabled() };
        hard_switch_off: "Ээ, я пошёл" => "Ээ, я пошёл",
    }

    cases! { Config { fillers: Fillers { enabled: false, ..Fillers::default() }, ..Config::disabled() };
        stage_switch_off: "Ээ, я пошёл" => "Ээ, я пошёл",
    }
}

mod soft_words {
    use super::*;

    cases! { soft();
        between_commas: "Я, короче, пошёл" => "Я пошёл",
        opening: "Короче, я пошёл" => "Я пошёл",
        closing: "Я пошёл, короче." => "Я пошёл.",
        closing_no_mark: "Я пошёл, короче" => "Я пошёл",
        multiword_between_commas: "Это было, как бы, во сне" => "Это было во сне",
        nu_with_comma: "Ну, я пошёл" => "Я пошёл",
        vot_closing: "Я пришёл, вот." => "Я пришёл.",
        vot_as_sentence: "Я пришёл. Вот. Потом ушёл." => "Я пришёл. Потом ушёл.",
        adjacent_fillers: "Ну вот, опять" => "Опять",
        eto_samoe: "Дай, это самое, отвёртку" => "Дай отвёртку",
        tipa: "Типа, так и надо" => "Так и надо",
        znachit: "Значит, завтра" => "Завтра",
        v_obshchem: "В общем, всё" => "Всё",
        second_sentence: "Я пришёл. Короче, всё." => "Я пришёл. Всё.",
        after_colon: "Сказал: короче, иди" => "Сказал: иди",
        caps: "КОРОЧЕ, Я ПОШЁЛ" => "Я ПОШЁЛ",
        repeated: "Короче короче, я пошёл" => "Я пошёл",
        mixed_with_hesitation: "Ну, ээ, короче, я пошёл" => "Я пошёл",
        hesitation_before_soft: "Ээ ну, я пошёл" => "Я пошёл",
        whole_phrase: "Короче." => "",
        in_brackets: "Он пришёл (ну, как обычно)" => "Он пришёл (как обычно)",
        dash_pause: "Я — короче — пошёл" => "Я пошёл",
        keeps_comma_after_clause: "Когда я пришёл, ну, все ушли" => "Когда я пришёл, все ушли",
        keeps_comma_after_long_clause: "Он опять пропал и не отвечает, короче, работу я пока остановил."
            => "Он опять пропал и не отвечает, работу я пока остановил.",
        three_words_keep_comma: "Мы всё сделали, типа, клиент доволен" => "Мы всё сделали, клиент доволен",
        two_word_subject_glued: "Мой брат, типа, пришёл" => "Мой брат пришёл",
        one_word_subject_glued: "Я, короче, пошёл" => "Я пошёл",
        clause_after_comma_counts_from_it: "Всё готово, мой брат, типа, пришёл" => "Всё готово, мой брат пришёл",
        clause_after_period_counts_from_it: "Пришёл. Мой брат, короче, ушёл." => "Пришёл. Мой брат ушёл.",
        long_clause_after_period: "Пришёл. Мы с ним поговорили, короче, всё решили." => "Пришёл. Мы с ним поговорили, всё решили.",
        keeps_comma_before_a: "Хорошо, вот, а потом ушли" => "Хорошо, а потом ушли",
        keeps_comma_before_no: "Я хотел, типа, но не смог" => "Я хотел, но не смог",
        keeps_comma_before_kogda: "Я пришёл, короче, когда все ушли" => "Я пришёл, когда все ушли",
        chain_after_clause: "Я не знаю, это самое, короче, всё." => "Я не знаю, всё.",
        chain_of_three: "Я не знаю, ну, типа, короче, как быть" => "Я не знаю, как быть",
        chain_after_subject: "Мы, типа, ну, договорились" => "Мы договорились",
        chain_at_start: "Ну, типа, я не знаю" => "Я не знаю",
        chain_with_hesitation: "Я не знаю, ээ, короче, всё" => "Я не знаю, всё",
        chain_before_question: "Короче, ну?" => "Ну?",
        aside_keeps_its_comma: "Проверь, пожалуйста, ну, этот код." => "Проверь, пожалуйста, этот код.",
        address_keeps_its_comma: "Слушай, Серёга, короче, надо сделать" => "Слушай, Серёга, надо сделать",
        opening_address_keeps_comma: "Слушай, короче, надо сделать" => "Слушай, надо сделать",
        by_the_way_keeps_comma: "Кстати, типа, я пришёл" => "Кстати, я пришёл",
        enumeration_keeps_comma: "Я пришёл, поел, ну, лёг спать" => "Я пришёл, поел, лёг спать",
        pronoun_after_comma_is_subject: "Блин, я, ээ, опоздал" => "Блин, я опоздал",
        two_word_aside_keeps_comma: "Честно говоря, типа, не знаю" => "Честно говоря, не знаю",
        aside_mid_sentence: "Он прав, конечно, ну, но не во всём" => "Он прав, конечно, но не во всём",
        // A name opening the sentence looks exactly like a subject without morphology.
        name_at_start_is_a_known_miss: "Вася, ну, иди сюда" => "Вася иди сюда",
        subject_loses_both: "Мы, типа, договорились" => "Мы договорились",
        conjunction_before_does_not_keep: "Он сказал, что, ну, придёт" => "Он сказал, что придёт",
        capital_moves_inside_quotes: "Он сказал: «Ну, ладно»." => "Он сказал: «Ладно».",
        adjacent_three: "Слушай, ну это самое, ты придёшь?" => "Слушай, ты придёшь?",
        before_ellipsis_run: "Ээ... ну... короче... я не знаю." => "Я не знаю.",
        like_between_commas: "I was, like, tired" => "I was tired",
        like_opening: "Like, what is this?" => "What is this?",
        you_know_opening: "You know, it is fine" => "It is fine",
        you_know_inside: "Things like, you know, stuff" => "Things like stuff",
        basically: "Basically, yes." => "Yes.",
        i_mean: "I mean, it works" => "It works",
    }

    untouched! { soft();
        kak_by_inside: "Это было как бы во сне",
        kak_by_opening: "Как бы не так",
        nu_without_comma: "Ну я пошёл",
        nu_question: "Ну?",
        nu_question_after_comma: "Ты придёшь, ну?",
        vot_exclamation: "Вот!",
        vot_noun_phrase: "Вот дом, который построил Джек",
        nu_vot_i_vsyo: "Ну вот и всё",
        eto_samoe_glavnoe: "Это самое главное",
        eto_samoe_chto: "Это самое, что у меня есть",
        tipa_inside: "Он типа знает",
        znachit_verb: "Это значит, что всё хорошо",
        korotche_comparative: "Сделай короче",
        like_verb: "I like pizza",
        like_preposition: "Like I said, it works",
        you_know_question: "Do you know the way?",
        you_know_what: "You know what, forget it",
        tipa_kotoryi: "Типа, который мы видели",
        vot_chto: "Вот, что я скажу",
    }

    cases! { only(&["короче"]);
        only_enabled_are_cut: "Ну, короче, я пошёл" => "Ну я пошёл",
        disabled_word_stays: "Ну, я пошёл" => "Ну, я пошёл",
    }

    cases! { only(&["блин", "Как Бы"]);
        custom_word: "Блин, опять" => "Опять",
        custom_phrase_case_insensitive: "Это, как бы, всё" => "Это всё",
    }

    cases! { Config::default();
        off_by_default: "Короче, я пошёл" => "Короче, я пошёл",
    }
}
