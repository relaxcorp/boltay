#[macro_use]
mod common;

use boltay_text::{Commands, Config};

fn commands() -> Config {
    Config {
        commands: Commands::default(),
        ..Config::disabled()
    }
}

fn spoken() -> Config {
    Config {
        commands: Commands {
            enabled: true,
            spoken_punctuation: true,
        },
        ..Config::disabled()
    }
}

mod line_breaks {
    use super::*;

    cases! { commands();
        new_line_as_sentence: "Привет. Новая строка. Как дела?" => "Привет.\nКак дела?",
        from_new_line: "Привет. С новой строки. Пока." => "Привет.\nПока.",
        keeps_comma_before: "Дорогой Иван, новая строка, как дела?" => "Дорогой Иван,\nкак дела?",
        paragraph: "Привет. Новый абзац. Пока." => "Привет.\n\nПока.",
        english_new_line: "Hello. New line. Bye." => "Hello.\nBye.",
        english_newline_word: "Hello. Newline. Bye." => "Hello.\nBye.",
        english_paragraph: "Hello. New paragraph. Bye." => "Hello.\n\nBye.",
        at_the_start: "Новая строка. Привет" => "\nПривет",
        at_the_end: "Привет. Новая строка." => "Привет.\n",
        several: "Раз. Новая строка. Два. Новая строка. Три." => "Раз.\nДва.\nТри.",
        capitalizes_new_sentence: "привет. новая строка. как дела" => "привет.\nКак дела",
        caps_command: "Привет. НОВАЯ СТРОКА. Пока" => "Привет.\nПока",
        with_yo: "Привет. Новая строка! Пока" => "Привет.\nПока",
        line_then_paragraph: "А. Новая строка. Б. Новый абзац. В." => "А.\nБ.\n\nВ.",
        two_in_a_row_add_up: "Раз. Новая строка. Новая строка. Два." => "Раз.\n\nДва.",
        paragraph_then_line_add_up: "Раз. Абзац. Новая строка. Два." => "Раз.\n\n\nДва.",
        pause_between_two_commands: "Раз. Новая строка.\n\nНовая строка. Два." => "Раз.\n\nДва.",
        abzac: "Конец текста. Абзац. Так, и идём дальше." => "Конец текста.\n\nТак, и идём дальше.",
        abzac_between_commas: "Первое, абзац, второе." => "Первое,\n\nвторое.",
        comma_before_a_closing_abzac: "Скидываю отчёт, абзац. Жду фидбек." => "Скидываю отчёт.\n\nЖду фидбек.",
        greeting_keeps_its_comma: "Привет, Иван, абзац. Пишу по поводу встречи." => "Привет, Иван,\n\nПишу по поводу встречи.",
        sign_off_keeps_its_comma: "С уважением, новая строка. Иван." => "С уважением,\nИван.",
        english_greeting_keeps_its_comma: "Dear John, new paragraph. I am writing." => "Dear John,\n\nI am writing.",
        business_greeting_keeps_its_comma: "Добрый день, Анна Сергеевна, абзац. Пишу по поводу встречи." => "Добрый день, Анна Сергеевна,\n\nПишу по поводу встречи.",
        greeting_at_the_end_keeps_its_comma: "Коллеги, всем привет, абзац. Новости." => "Коллеги, всем привет,\n\nНовости.",
        title_keeps_its_comma: "Уважаемая Анна Сергеевна Иванова, абзац. Пишу." => "Уважаемая Анна Сергеевна Иванова,\n\nПишу.",
        thanks_line_keeps_its_comma: "Спасибо большое, новая строка. Иван." => "Спасибо большое,\nИван.",
        respect_at_the_end_keeps_its_comma: "Жду ответа, с уважением, новая строка. Иван." => "Жду ответа, с уважением,\nИван.",
        greeting_then_a_sentence: "Привет, я тут подумал, абзац. Давай созвонимся." => "Привет, я тут подумал.\n\nДавай созвонимся.",
        greeting_after_a_name: "Иван, добрый день, абзац. Пишу." => "Иван, добрый день,\n\nПишу.",
        title_and_addressee: "Уважаемые клиенты, абзац. Сообщаем." => "Уважаемые клиенты,\n\nСообщаем.",
        greeting_and_a_word_of_address: "Привет, мам, абзац. Как дела?" => "Привет, мам,\n\nКак дела?",
        closing_said_alone: "Всего доброго, новая строка. Иван." => "Всего доброго,\nИван.",
        thanks_with_a_pronoun: "Спасибо вам большое, новая строка. Иван." => "Спасибо вам большое,\nИван.",
        regards_passed_on: "Передай Ане привет, абзац. И не забудь документы." => "Передай Ане привет.\n\nИ не забудь документы.",
        respect_as_an_ordinary_word: "Мы относимся к клиентам с уважением, абзац. Это наш принцип." => "Мы относимся к клиентам с уважением.\n\nЭто наш принцип.",
        title_word_as_an_adjective: "Дорогой телефон, абзац. Но классный." => "Дорогой телефон.\n\nНо классный.",
        greeting_from_a_place: "Привет из Москвы, абзац. Мы приехали." => "Привет из Москвы.\n\nМы приехали.",
        greeting_then_a_question: "Привет, у меня вопрос, абзац. Ты завтра свободен?" => "Привет, у меня вопрос.\n\nТы завтра свободен?",
        greeting_then_an_introduction: "Здравствуйте, меня зовут Анна, абзац. Я по поводу вакансии." => "Здравствуйте, меня зовут Анна.\n\nЯ по поводу вакансии.",
        greeting_then_a_request: "Добрый день, коллеги, высылаю отчёт, абзац. Жду замечаний." => "Добрый день, коллеги, высылаю отчёт.\n\nЖду замечаний.",
        greeting_to_everyone: "Всем добрый день, абзац. Новости." => "Всем добрый день,\n\nНовости.",
        title_with_two_addressees: "Дорогие друзья и коллеги, абзац. Рад видеть." => "Дорогие друзья и коллеги,\n\nРад видеть.",
        addressee_stem_is_not_another_word: "Дорогая гостиница, абзац. Но красивая." => "Дорогая гостиница.\n\nНо красивая.",
        regards_to_mum: "У нас всё хорошо. Маме привет, абзац. Целую." => "У нас всё хорошо. Маме привет.\n\nЦелую.",
        regards_after_the_greeting_word: "Привет маме, абзац. Скоро приеду." => "Привет маме.\n\nСкоро приеду.",
        english_introduction: "Hi, I'm Anna, new line. Nice to meet you." => "Hi, I'm Anna.\nNice to meet you.",
        english_greeting_to_a_team: "Hi team, new line. Quick update." => "Hi team,\nQuick update.",
        english_dear_in_a_reply: "Yes, dear, new line. I'll buy milk." => "Yes, dear.\nI'll buy milk.",
        greeting_in_reported_speech: "Он говорит, привет, абзац. Я молчу." => "Он говорит, привет.\n\nЯ молчу.",
        thanks_in_a_sentence_is_no_sign_off: "Спасибо за отчёт, абзац. Жду ответа." => "Спасибо за отчёт.\n\nЖду ответа.",
        greeting_word_mid_sentence: "Это был очень дорогой подарок, абзац. Спасибо ещё раз." => "Это был очень дорогой подарок.\n\nСпасибо ещё раз.",
        abzac_alone: "Абзац." => "\n\n",
        abzac_lowercase: "раз. абзац. два" => "раз.\n\nДва",
        abzac_at_the_end: "Всё сказал. Абзац." => "Всё сказал.\n\n",
        abzac_question: "Готово? Абзац! Дальше." => "Готово?\n\nДальше.",
        // Pauses around the command already made paragraphs of their own.
        pause_abzac_pause: "Что ещё?\n\nАбзац.\n\nКороче, дальше." => "Что ещё?\n\nКороче, дальше.",
        pause_before_abzac: "Что ещё?\n\nАбзац. Дальше." => "Что ещё?\n\nДальше.",
        abzac_before_pause: "Что ещё? Абзац.\n\nДальше." => "Что ещё?\n\nДальше.",
        pause_new_line_pause: "Раз.\n\nНовая строка.\n\nДва." => "Раз.\nДва.",
        new_line_then_abzac: "Раз. Новая строка. Абзац. Два." => "Раз.\n\n\nДва.",
    }

    untouched! { commands();
        single_abzac: "Прочитай этот абзац",
        abzac_in_sentence: "Этот абзац слишком длинный.",
        abzac_opens_sentence: "Абзац, который ты прислал, хороший.",
        abzac_before_comma: "Прочитай абзац, пожалуйста.",
        new_line_in_phrase: "Я написал новая строка кода",
        new_line_unpunctuated: "Привет новая строка как дела",
        new_paragraph_as_object: "Добавь новый абзац.",
        english_in_phrase: "Start a new paragraph here",
        new_line_in_word: "Новая строкаа",
        stroka_alone: "Эта строка новая",
        spoken_punctuation_off: "Привет запятая как дела",
    }

    cases! { Config { commands: Commands { enabled: false, ..Commands::default() }, ..Config::disabled() };
        stage_switch_off: "Привет. Новая строка. Пока." => "Привет. Новая строка. Пока.",
    }
}

mod delete_last {
    use super::*;

    cases! { commands();
        previous_sentence: "Привет. Я опоздаю. Удали последнее. Буду вовремя." => "Привет. Буду вовремя.",
        mid_sentence: "Я приду в пять, удали последнее, в шесть." => "В шесть.",
        mid_second_sentence: "Привет. Я приду в пять, удали последнее, в шесть." => "Привет. В шесть.",
        nothing_before: "Удали последнее." => "",
        twice: "Первое. Второе. Удали последнее. Удали последнее. Третье." => "Третье.",
        long_form: "Привет. Удали последнее предложение. Пока." => "Пока.",
        erase_form: "Привет. Сотри последнее. Пока." => "Пока.",
        at_the_end: "Привет. Я опоздаю. Удали последнее." => "Привет.",
        after_ellipsis: "Эмм, я... удали последнее." => "",
        after_ellipsis_keeps_earlier: "Привет. Я... удали последнее. Пока." => "Привет. Пока.",
        after_mixed_marks: "Первое! Второе?! Удали последнее. Третье." => "Первое! Третье.",
        question_before: "Ты придёшь? Удали последнее. Жду." => "Жду.",
        not_across_spoken_lines: "Первая строка. Новая строка. Удали последнее. Вторая." => "Первая строка.\nВторая.",
        across_a_pause: "Привет. Я опоздаю.\n\nУдали последнее. Буду вовремя." => "Привет. Буду вовремя.",
        across_a_pause_at_the_end: "Привет. Я опоздаю.\n\nУдали последнее." => "Привет.",
        across_a_pause_without_a_mark: "Привет. Я опоздаю\n\nудали последнее" => "Привет.",
        pause_before_the_first_sentence: "Я опоздаю.\n\nУдали последнее. Буду." => "Буду.",
        scratch_that: "I will be late. Scratch that. I will be on time." => "I will be on time.",
        scratch_that_mid: "Meet at five, scratch that, at six." => "At six.",
    }

    untouched! { commands();
        followed_by_object: "Он сказал удали последнее фото",
        scratch_that_itch: "Scratch that itch",
        last_without_verb: "Это последнее предупреждение.",
    }
}

mod spoken_marks {
    use super::*;

    cases! { commands();
        ellipsis_eats_model_marks: "Идём дальше, троеточие." => "Идём дальше…",
        ellipsis_mid_text: "Подумал многоточие и решил" => "Подумал… и решил",
        question_default: "Как дела, вопросительный знак." => "Как дела?",
        exclamation_default: "Ура, восклицательный знак." => "Ура!",
        colon_default: "Внимание, двоеточие, всё." => "Внимание: всё.",
        semicolon_default: "Раз, точка с запятой, два." => "Раз; два.",
        dash_default: "Москва, тире, столица." => "Москва — столица.",
        hyphen: "Кто дефис то пришёл" => "Кто-то пришёл",
        hyphen_punctuated: "Кто, дефис, то пришёл." => "Кто-то пришёл.",
        quotes: "Он сказал, открыть кавычки, привет, закрыть кавычки." => "Он сказал «привет».",
        quotes_after_colon: "Он сказал: открой кавычки привет закрой кавычки" => "Он сказал: «привет»",
        quotes_as_sentences: "Привет. Открыть кавычки. Цитата. Закрыть кавычки. Дальше." => "Привет. «Цитата». Дальше.",
        quotes_the_recognizer_put_already: "Он сказал открыть кавычки:«Всё будет завтра». Закрыть кавычки. Но я не верю." => "Он сказал «Всё будет завтра». Но я не верю.",
        mark_put_already_keeps_the_space: "Он сказал открыть кавычки «Всё будет завтра» закрыть кавычки и ушёл." => "Он сказал «Всё будет завтра» и ушёл.",
        question_put_already: "Как дела? Вопросительный знак. Всё нормально?" => "Как дела? Всё нормально?",
        dash_put_already: "Москва — тире столица России." => "Москва — столица России.",
        bracket_put_already: "Цена (без НДС) закрыть скобку составляет сто рублей." => "Цена (без НДС) составляет сто рублей.",
        open_without_a_spoken_close: "Тема письма открыть кавычки «Отчёт за сентябрь»." => "Тема письма «Отчёт за сентябрь».",
        open_put_already_then_words: "Он сказал открыть кавычки «Всё будет завтра» и ушёл." => "Он сказал «Всё будет завтра» и ушёл.",
        pause_before_the_close: "Он сказал открыть кавычки «Всё будет завтра»\n\nЗакрыть кавычки. И ушёл." => "Он сказал «Всё будет завтра». И ушёл.",
        question_before_the_close: "Он спросил открыть кавычки «Ты придёшь»? Закрыть кавычки." => "Он спросил «Ты придёшь»?",
        two_quotes_one_after_another: "Он написал открыть кавычки «Скоро буду». Абзац. Я ответил открыть кавычки «Жду» закрыть кавычки." => "Он написал «Скоро буду».\n\nЯ ответил «Жду».",
        quote_inside_a_quote: "Он сказал открыть кавычки смотрел фильм «Брат» закрыть кавычки." => "Он сказал «смотрел фильм «Брат»».",
        quote_opens_with_a_quote: "Открыть кавычки «Брат» мой любимый фильм закрыть кавычки." => "««Брат» мой любимый фильм».",
        bracket_inside_a_bracket: "Текст открыть скобку см. рисунок (снизу) закрыть скобку и дальше." => "Текст (см. рисунок (снизу)) и дальше.",
        comma_after_a_quote: "Он сказал открыть кавычки всё будет завтра, закрыть кавычки, но я не верю." => "Он сказал «всё будет завтра», но я не верю.",
        no_comma_after_a_bracket: "Цена открыть скобку без НДС, закрыть скобку, составляет сто рублей." => "Цена (без НДС) составляет сто рублей.",
        brackets: "Текст открыть скобку в скобках закрыть скобку и дальше" => "Текст (в скобках) и дальше",
        brackets_before_full_stop: "Текст, открыть скобку, пояснение, закрыть скобку." => "Текст (пояснение).",
        english_quotes: "He said open quote hi close quote" => "He said \"hi\"",
        english_ellipsis: "Wait ellipsis" => "Wait…",
    }

    untouched! { commands();
        comma_needs_option: "Привет запятая как дела",
        full_stop_needs_option: "Готово точка отправляю",
        english_comma_needs_option: "Hello comma world",
    }

    cases! { spoken();
        comma: "Привет запятая как дела" => "Привет, как дела",
        comma_already_punctuated: "Привет, запятая, как дела" => "Привет, как дела",
        question: "Как дела вопросительный знак" => "Как дела?",
        exclamation: "Ура восклицательный знак" => "Ура!",
        full_stop_capitalizes: "Готово точка отправляю" => "Готово. Отправляю",
        full_stop_at_end: "Готово точка" => "Готово.",
        semicolon_not_stop: "Раз точка с запятой два" => "Раз; два",
        colon: "Внимание двоеточие всё" => "Внимание: всё",
        dash: "Москва тире столица" => "Москва — столица",
        ellipsis: "И так далее многоточие" => "И так далее…",
        english: "Hello comma world exclamation mark" => "Hello, world!",
        english_full_stop: "Done full stop next" => "Done. Next",
        english_question: "Ready question mark" => "Ready?",
        with_new_line: "Привет запятая, новая строка, как дела" => "Привет,\nкак дела",
    }

    untouched! { spoken();
        point_of_view: "Это точка зрения автора",
        reference_point: "Нужна точка отсчёта",
        period_of_time: "A long period of time",
        other_case: "Поставь точку в конце",
    }
}
