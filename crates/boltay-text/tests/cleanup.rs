#[macro_use]
mod common;

use boltay_text::{Cleanup, Config};

fn cleanup() -> Config {
    Config {
        cleanup: Cleanup::default(),
        ..Config::disabled()
    }
}

fn trailing() -> Config {
    Config {
        cleanup: Cleanup {
            enabled: true,
            trailing_space: true,
        },
        ..Config::disabled()
    }
}

cases! { cleanup();
    double_spaces: "Привет  мир" => "Привет мир",
    number_sign_after_a_russian_word: "Кабинет No 312." => "Кабинет № 312.",
    number_sign_not_after_english: "Room No 5 is free." => "Room No 5 is free.",
    no_without_a_number_stays: "Сказал No comment." => "Сказал No comment.",
    space_before_a_glued_amount: "Перевёл$200." => "Перевёл $200.",
    amount_after_a_space_stays: "Перевёл $200 и €50." => "Перевёл $200 и €50.",
    currency_prefix_stays_glued: "Цена US$200, в Гонконге HK$300." => "Цена US$200, в Гонконге HK$300.",
    breaks_collapse_to_one_empty_line: "Раз.\n\n\n\n\n\nДва." => "Раз.\n\nДва.",
    three_breaks: "Раз.\n\n\nДва." => "Раз.\n\nДва.",
    breaks_with_spaces_between: "Раз.\n \n \n Два." => "Раз.\n\nДва.",
    paragraph_kept: "Раз.\n\nДва.\nТри." => "Раз.\n\nДва.\nТри.",
    space_before_comma: "Привет , мир" => "Привет, мир",
    space_before_marks: "Что ? Да ! Ладно ." => "Что? Да! Ладно.",
    space_before_colon: "Итак : всё" => "Итак: всё",
    glued_comma: "Привет,мир" => "Привет, мир",
    decimal_comma: "3,5 литра" => "3,5 литра",
    double_comma: "Привет,, мир" => "Привет, мир",
    comma_before_stop: "Привет, ." => "Привет.",
    comma_after_stop: "Привет. , мир" => "Привет. мир",
    leading_comma: ", привет" => "Привет",
    leading_stop: ". привет" => "Привет",
    brackets: "( текст )" => "(текст)",
    capital_at_start: "привет" => "Привет",
    capital_yo: "ёлки" => "Ёлки",
    capital_after_line_with_stop: "привет.\nкак дела" => "Привет.\nКак дела",
    no_capital_after_comma_line: "дорогой Иван,\nкак дела" => "Дорогой Иван,\nкак дела",
    capital_after_blank_line: "раз.\n\nдва" => "Раз.\n\nДва",
    mixed_case_word: "iPhone в кармане" => "iPhone в кармане",
    leading_ellipsis: "...и тогда" => "...и тогда",
    abbreviation: "т.е. так" => "Т.е. так",
    keeps_line_breaks: "\nпривет\n" => "\nПривет\n",
    dash_spaced: "Москва — столица" => "Москва — столица",
    empty: "" => "",
}

cases! { trailing();
    trailing_space: "Привет." => "Привет. ",
    no_trailing_space_on_empty: "" => "",
    no_trailing_space_after_line_break: "Привет.\n" => "Привет.\n",
}

cases! { Config::disabled();
    stage_switch_off: "привет , мир" => "привет , мир",
}
