#[macro_use]
mod common;

use boltay_text::Config;

fn whitespace() -> Config {
    Config {
        whitespace: true,
        ..Config::disabled()
    }
}

cases! { whitespace();
    trims: "  привет мир  " => "привет мир",
    collapses_runs: "привет    мир" => "привет мир",
    tabs: "привет\t\tмир" => "привет мир",
    nbsp: "привет\u{a0}мир" => "привет мир",
    narrow_nbsp: "10\u{202f}000" => "10 000",
    thin_space: "а\u{2009}б" => "а б",
    zero_width_inside_word: "при\u{200b}вет" => "привет",
    bom: "\u{feff}привет" => "привет",
    soft_hyphen: "пере\u{ad}нос" => "перенос",
    crlf: "раз\r\nдва" => "раз\nдва",
    lone_cr: "раз\rдва" => "раз\nдва",
    spaces_around_line_break: "раз  \n  два" => "раз\nдва",
    blank_lines_collapse: "раз\n\n\n\nдва" => "раз\n\nдва",
    blank_line_of_spaces: "раз\n   \n   \nдва" => "раз\n\nдва",
    leading_and_trailing_lines: "\n\n раз \n\n" => "раз",
    only_spaces: "   \t  " => "",
    empty: "" => "",
    punctuation_untouched: "Привет , мир" => "Привет , мир",
}

cases! { Config::disabled();
    stage_switch_off: "  привет    мир  " => "  привет    мир  ",
}
