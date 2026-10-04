#[macro_use]
mod common;

use boltay_text::{Config, Dictionary, Replacement, Snippet, Term, Terms};

fn brands() -> Config {
    Config {
        brands: Config::default().brands,
        ..Config::disabled()
    }
}

fn with_ambiguous() -> Config {
    Config {
        brands: Config::default().brands,
        ambiguous: Terms {
            enabled: true,
            ..Config::default().ambiguous
        },
        ..Config::disabled()
    }
}

// Misrecognitions from real dictation, and the usual ways to say the same.
cases! { brands();
    github_misheard: "Код лежит на гитхап" => "Код лежит на GitHub",
    github_latin: "Код лежит на githab" => "Код лежит на GitHub",
    github_split: "Залей на гит хаб" => "Залей на GitHub",
    github_case_ending: "Код лежит на гитхабе" => "Код лежит на GitHub",
    github_instrumental: "Пользуюсь гитхабом" => "Пользуюсь GitHub",
    github_wrong_case: "Код на Github" => "Код на GitHub",
    spotify_spotyfy: "Слушаю в Spotyfy" => "Слушаю в Spotify",
    spotify_spatify: "Слушаю в spatify" => "Слушаю в Spotify",
    spotify_ending_after_y: "Слушаю в спотифае" => "Слушаю в Spotify",
    faceit_facet: "Играю на Facet" => "Играю на Faceit",
    faceit_cyrillic: "Играю на фейсите" => "Играю на Faceit",
    backend: "Bekend на пайтоне, фронтэнд на Swelt" => "Бэкенд на Python, фронтенд на Svelte",
    backend_hyphen: "Бэк-энд готов" => "Бэкенд готов",
    backend_mid_sentence: "У нас Bekend упал" => "У нас бэкенд упал",
    backend_keeps_its_ending: "Упало на бекенде" => "Упало на бэкенде",
    chatgpt_split: "Спроси у Chat GPT" => "Спроси у ChatGPT",
    claude_cyrillic: "Спроси у клода" => "Спроси у Claude",
    counter_strike_2: "Играю в CounterStrike 2" => "Играю в Counter-Strike 2",
    counter_strike_without_number: "Играю в контр страйк" => "Играю в Counter-Strike",
    pull_request: "Открой пулл реквест" => "Открой pull request",
    lowercase_name_opens_sentence: "Пулл реквест готов." => "Pull request готов.",
    lowercase_name_mid_sentence: "Открой Пулл реквест, там всё" => "Открой pull request, там всё",
    messengers: "Скинь в телеграм или в ватсап" => "Скинь в Telegram или в WhatsApp",
    shops: "Авито, вайлдберриз, озон, яндекс маркет" => "Avito, Wildberries, Ozon, Яндекс Маркет",
    market_case_ending: "Продаю на яндекс маркете" => "Продаю на Яндекс Маркете",
    market_glued: "Цены яндексмаркета" => "Цены Яндекс Маркета",
    svelte_soft_sign: "Пишу на свельте" => "Пишу на Svelte",
    streaming: "Стримлю на твич и ютуб" => "Стримлю на Twitch и YouTube",
    tiktok: "Видео в тик ток" => "Видео в TikTok",
    binance: "Оплата через бинанс" => "Оплата через Binance",
    usdt_spoken: "Оплата в юсдт" => "Оплата в USDT",
    with_yo: "Докёр упал" => "Docker упал",
    next_to_punctuation: "(гитхаб), докер." => "(GitHub), Docker.",
    sentence_end_is_not_an_address: "Код на гитхабе. Смотри" => "Код на GitHub. Смотри",
    plural: "Посмотри мои пулл реквесты" => "Посмотри мои pull requests",
    plural_genitive: "Пять пулл реквестов висят" => "Пять pull requests висят",
    plural_cases: "По пулл реквестам и в пулл реквестах" => "По pull requests и в pull requests",
    singular_stays: "В пулл реквесте ошибка" => "В pull request ошибка",
    no_plural_for_a_brand: "Два гитхаба" => "Два GitHub",
    quote_opens_sentence: "«пулл реквест» готов" => "«Pull request» готов",
    bracket_opens_sentence: "Готово. (пулл реквест открыт)" => "Готово. (Pull request открыт)",
    after_that_is: "Задача, т.е. пулл реквест, готова" => "Задача, т.е. pull request, готова",
    after_see: "Подробности см. пулл реквест" => "Подробности см. pull request",
    after_ellipsis: "Ну и вот… пулл реквест" => "Ну и вот… pull request",
    after_dots: "Ну и вот... пулл реквест" => "Ну и вот... pull request",
    capital_after_ellipsis: "Подумал… Пулл реквест готов" => "Подумал… Pull request готов",
    capital_after_abbreviation: "Ждём ревью и т.д. Пулл реквест готов" => "Ждём ревью и т.д. Pull request готов",
    hyphen_with_russian: "Создай гитхаб-репозиторий и ютуб-канал" => "Создай GitHub-репозиторий и YouTube-канал",
    address_next_to_a_word: "Смотри github.com, гитхаб лучше" => "Смотри github.com, GitHub лучше",
}

// Services and work tools people across the CIS name every day, in the cases they take.
cases! { brands();
    vk: "Напиши мне в вк" => "Напиши мне в VK",
    vkontakte: "Есть страница вконтакте" => "Есть страница ВКонтакте",
    sber_case: "Перевёл через сбер, в сбере очередь" => "Перевёл через Сбер, в Сбере очередь",
    sberbank_split: "Карта сбер банка" => "Карта Сбербанка",
    t_bank: "Счёт в т банке" => "Счёт в Т-Банке",
    t_bank_glued: "Открыл тбанк" => "Открыл Т-Банк",
    tinkoff: "Карта тинькоф" => "Карта Тинькофф",
    alfa_bank: "Кредит в альфа банке" => "Кредит в Альфа-Банке",
    alfa_bank_glued: "Альфабанк одобрил" => "Альфа-Банк одобрил",
    gosuslugi: "Записался через гос услуги" => "Записался через Госуслуги",
    yandex: "Ищи в яндексе" => "Ищи в Яндексе",
    yandex_taxi: "Вызови яндекс такси" => "Вызови Яндекс Такси",
    yandex_food: "Заказал в яндекс еде" => "Заказал в Яндекс Еде",
    yandex_music: "Слушаю яндекс музыку" => "Слушаю Яндекс Музыку",
    yandex_disk: "Файл на яндекс диске" => "Файл на Яндекс Диске",
    yandex_go: "Открой яндекс гоу" => "Открой Яндекс Go",
    yandex_browser: "Скачай яндекс браузер" => "Скачай Яндекс Браузер",
    kinopoisk: "Смотрю на кино поиске" => "Смотрю на Кинопоиске",
    kaspi: "Оплатил через каспи" => "Оплатил через Kaspi",
    lamoda: "Заказал на ламоде" => "Заказал на Lamoda",
    instagram: "Выложил в инсту и в инстаграмм" => "Выложил в Instagram и в Instagram",
    notion: "Задачи в ноушене" => "Задачи в Notion",
    figma: "Макет в фигме" => "Макет в Figma",
    jira: "Заведи таску в джире" => "Заведи таску в Jira",
    trello: "Доска в трело" => "Доска в Trello",
    slack: "Напиши в слаке" => "Напиши в Slack",
    kubernetes: "Подняли в кубернетисе" => "Подняли в Kubernetes",
    kubernetes_short: "Деплой в кубер" => "Деплой в Kubernetes",
    gitlab: "Код на гит лабе" => "Код на GitLab",
    postman: "Проверь в постмане" => "Проверь в Postman",
    vs_code: "Открой в вс код" => "Открой в VS Code",
    excel: "Таблица в экселе" => "Таблица в Excel",
    word: "Документ в ворде" => "Документ в Word",
    powerpoint: "Презентация в пауэр поинте" => "Презентация в PowerPoint",
    google_docs: "Скинь в гугл доки" => "Скинь в Google Docs",
    google_sheets: "Посчитай в гугл таблицах" => "Посчитай в Google Sheets",
    google_drive: "Лежит на гугл диске" => "Лежит на Google Drive",
}

// More misrecognitions from real dictation.
cases! { brands();
    yandex_food_dotted: "Закажи в Яндекс.Еде, а потом вызови Яндекс Go" => "Закажи в Яндекс Еде, а потом вызови Яндекс Go",
    yandex_music_glued: "Музыку включи в ЯндексМузыке" => "Музыку включи в Яндекс Музыке",
    yandex_music_dotted: "Музыку включи в Яндекс.Музыке" => "Музыку включи в Яндекс Музыке",
    yandex_disk_dotted: "Закинь файл на Яндекс.Диск" => "Закинь файл на Яндекс Диск",
    yandex_other_service: "Посмотри в Яндекс.Картах" => "Посмотри в Яндекс Картах",
    yandex_opens_sentence: "ЯндексМаркет привёз" => "Яндекс Маркет привёз",
    google_disk: "Закинь в Google Disk" => "Закинь в Google Drive",
    lamoda_misheard: "Купила кроссовки на Ламодо" => "Купила кроссовки на Lamoda",
    lamoda_split: "Купила на La Modo" => "Купила на Lamoda",
    kaspi_case: "В Каспе дороже" => "В Kaspi дороже",
    jira_misheard: "Задача в Джайро висит" => "Задача в Jira висит",
    trello_misheard: "Закинь таску в трейло" => "Закинь таску в Trello",
    trello_with_preposition: "Закинь таску в трейле" => "Закинь таску в Trello",
    prod: "Мы задеплоили на прот, но там баг" => "Мы задеплоили на прод, но там баг",
    prod_case: "На проте всё упало" => "На проде всё упало",
    hotfix_split: "Нужен ход Фикс до вечера" => "Нужен хотфикс до вечера",
    hotfix_hyphen: "Нужен ход-фикс" => "Нужен хотфикс",
    hotfix_latin: "Нужен Hod Fix до завтра, Hotfix готов" => "Нужен хотфикс до завтра, хотфикс готов",
    hotfix_case: "Без ход фикса не обойтись" => "Без хотфикса не обойтись",
    vs_code_half_latin: "Проверь через Postman и VS Cod" => "Проверь через Postman и VS Code",
    vs_code_half_cyrillic: "Открой в VS код" => "Открой в VS Code",
    readme: "Надо сделать нормальный ридми" => "Надо сделать нормальный README",
    okay: "О'кей, договорились" => "Окей, договорились",
    okay_mid_sentence: "Ну о’кей, давай" => "Ну окей, давай",
    okay_opens_a_quote: "Он ответил: «О'кей, жду»." => "Он ответил: «Окей, жду».",
    lowercase_name_keeps_a_capital: "Тема письма: «Хотфикс для логина»." => "Тема письма: «Хотфикс для логина».",
    recognizer_capital_mid_sentence: "Готово, мы перешли на Бэкенд" => "Готово, мы перешли на бэкенд",
}

untouched! { brands();
    yandex_address: "Зайди на yandex.ru или Яндекс.ру",
    yandex_alone: "Найди в Яндексе. Музыку не включай",
    yandex_inside_a_word: "ПроЯндексМузыку не слышал",
    prototype: "Прототип готов, протокол подписан",
    trailer: "Посмотри трейлер, в трейлере всё видно",
    move_then_fix: "Сделай ход, фикс цены потом",
    fixing_a_move: "Я фиксирую ход",
    versus: "Сравни Python vs Go",
    say_okay: "Он сказал окей",
    protein: "Выпил прот после тренировки, проты лучше брать большие",
    protein_inside: "Сколько белка в проте? Добавь в прот банан",
    trail: "Бегал вчера на трейле, ноги болят",
    going_to_yandex: "Завтра в Яндекс иду на собеседование, ты в Яндекс иди тоже",
    sell_in_a_quote: "Объявление: «Продам гараж». Магазин «Продам» на углу",
    okay_after_a_dash: "— Окей, — сказал он",
    prod_in_a_quote: "Сообщение в чат: «Прод лежит, чиним»",
}

untouched! { brands();
    keep_in_touch: "Давай будем в контакте",
    pig_fat: "Убери лишнего жира",
    camera_zoom: "Сделай зум на лицо",
    edited_photo: "Это явный фотошоп",
    surname_tinkov: "Олег Тиньков написал книгу",
    surname_miro: "Картины Миро в музее",
    caspian_sea: "Отдыхали на Каспийском море",
    words_inside: "Сберегательный счёт и вкладка браузера",
    cube: "Кубок достался им",
    ordinary_word_disk: "Вставь диск в дисковод",
    music_alone: "Включи музыку погромче",
    taxi_alone: "Вызови такси к дому",
    go_inside_a_word: "Яндекс говорит, что пробки",
    slack_inside_a_word: "Слаксы и куртка",
    word_inside_a_word: "Вордпресс устарел",
}

untouched! { brands();
    tons_of_cargo: "Привезли десять тонн груза",
    tone_of_voice: "Не говори таким тоном",
    face: "Face ID не работает",
    face_russian: "У него фейс недовольный",
    python_snake: "В зоопарке живёт питон",
    cloud: "Храним всё в cloud",
    request_word: "Отправил реквест в поддержку",
    dollars: "Курс USD вырос",
    inside_a_word: "Стимулировать продажи",
    derived_word: "Докерский кран",
    telegram_message: "Пришла телеграмма от бабушки",
    telegrams: "Много телеграмм за день",
    construction_form: "Подпиши акт КС-2 до пятницы",
    url: "Скачай с https://github.com/relaxcorp/boltay",
    email: "Пиши на steam@mail.ru",
    domain: "Открой github.com и youtube.com",
    subdomain: "Запрос на api.github.com",
    file_name: "Поправь docker-compose.yml",
    latin_compound: "Запусти docker-compose up",
    windows_path: r"Лежит в C:\github\boltay",
    query_string: "Ссылка site.ru/?ref=telegram",
    mail_address: "Пиши на telegram@example.ru",
}

cases! { with_ambiguous();
    in_touch_as_vk: "Пиши мне в контакте" => "Пиши мне ВКонтакте",
    jira_as_fat: "Заведи таску в жире" => "Заведи таску в Jira",
    zoom_call: "Созвон в зуме" => "Созвон в Zoom",
    photoshop_app: "Открой в фотошопе" => "Открой в Photoshop",
    tons_as_coin: "Оплата в тонн" => "Оплата в TON",
    usd_as_usdt: "Оплата в USD через бинанс" => "Оплата в USDT через Binance",
    face_as_faceit: "Играю на Face" => "Играю на Faceit",
    python_case_ending: "Пишу на питоне" => "Пишу на Python",
    counter_strike_short: "Катаю в КС 2" => "Катаю в Counter-Strike 2",
}

untouched! { with_ambiguous();
    tone_with_ambiguous: "Не говори таким тоном",
    cloud_with_ambiguous: "Храним всё в Google Cloud",
    merge_request: "Отправь merge request",
}

cases! { Config { brands: Terms { enabled: false, ..Config::default().brands }, ..Config::disabled() };
    switched_off: "Код на гитхабе" => "Код на гитхабе",
}

cases! {
    Config {
        dictionary: Dictionary {
            enabled: true,
            entries: vec![Replacement::new("гитхаб", "Гитхаб"), Replacement::new("гитхаб проект", "репо")],
        },
        ..brands()
    };
    user_entry_wins: "Код на гитхаб" => "Код на Гитхаб",
    brand_leaves_the_users_word_alone: "Код на гитхабе" => "Код на гитхабе",
    longer_user_entry: "Мой гитхаб проект" => "Мой репо",
    brand_not_rescanned_by_user: "Код на гитхап" => "Код на GitHub",
}

cases! {
    Config {
        brands: Terms {
            enabled: true,
            entries: vec![Term::new("iPhone", &["айфон"])],
            ..Terms::default()
        },
        ..Config::disabled()
    };
    edited_list: "Купил айфон" => "Купил iPhone",
    keeps_inner_capitals_at_start: "Айфон сломался" => "iPhone сломался",
}

cases! {
    Config {
        brands: Terms {
            enabled: true,
            entries: vec![Term::new("", &["гитхаб"]), Term::new("Telegram", &["телеграм"])],
            ..Terms::default()
        },
        ..Config::disabled()
    };
    name_not_typed_yet: "Код на гитхабе, скинь в телеграм" => "Код на гитхабе, скинь в Telegram",
}

cases! {
    Config {
        brands: Terms {
            enabled: true,
            entries: vec![
                Term::new("YouTube", &["ютуб"]),
                Term::new(" Ютуб ", &["ютуб"]),
                Term::new("Сбер", &["сбер"]),
            ],
            ..Terms::default()
        },
        ..Config::disabled()
    };
    added_row_wins: "Посмотри на ютубе" => "Посмотри на Ютубе",
    own_cyrillic_name_keeps_ending: "Работаю в сбере" => "Работаю в Сбере",
}

// Names people add for their own tools, ending in a vowel or a soft sign.
cases! {
    Config {
        brands: Terms {
            enabled: true,
            entries: vec![
                Term::new("Figma", &["фигма"]),
                Term::new("Jira", &["джира"]),
                Term::new("Excel", &["эксель"]),
                Term::new("Тильда", &["тильда"]),
            ],
            ..Terms::default()
        },
        ..Config::disabled()
    };
    feminine_ending: "Макет в фигме, задача в джире" => "Макет в Figma, задача в Jira",
    instrumental: "Работаю с фигмой" => "Работаю с Figma",
    soft_sign: "Таблица в экселе" => "Таблица в Excel",
    cyrillic_name_with_vowel: "Сайт на тильде" => "Сайт на Тильде",
}

#[test]
fn snippet_trigger_may_name_a_brand() {
    let mut config = Config::default();
    config.snippets.entries = vec![
        Snippet::new("ссылка на гитхаб", "https://github.com/relaxcorp"),
        Snippet::new("мой Telegram", "t.me/relaxdev"),
    ];
    assert_eq!(
        boltay_text::process(&config, "Ссылка на гитхаб."),
        "https://github.com/relaxcorp"
    );
    assert_eq!(
        boltay_text::process(&config, "Мой телеграм."),
        "t.me/relaxdev"
    );
}

#[test]
fn saved_list_gets_only_new_builtin_names() {
    // Saved by an older version: GitHub renamed by the user, Spotify deleted.
    let mut saved = Terms {
        enabled: true,
        entries: vec![Term::new("Гитхаб", &["гитхаб"])],
        seen: vec!["GitHub".into(), "Spotify".into()],
        ..Terms::default()
    };
    saved.add_new(vec![
        Term::new("GitHub", &["гитхап"]),
        Term::new("Spotify", &["спотифай"]),
        Term::new("Twitch", &["твич"]),
    ]);
    let names: Vec<&str> = saved.entries.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Twitch", "Гитхаб"]);
    assert_eq!(saved.seen, ["GitHub", "Spotify", "Twitch"]);
}

#[test]
fn new_builtin_variant_reaches_the_users_row() {
    // Saved by a version that knew only `джира` and did not track variants.
    let mut saved = Terms {
        enabled: true,
        entries: vec![
            Term::new("Jira", &["джира"]),
            Term::new("Гитхаб", &["гитхаб"]),
        ],
        seen: vec!["Jira".into(), "GitHub".into()],
        ..Terms::default()
    };
    saved.add_new(vec![
        Term::new("Jira", &["джира", "джайро"]),
        Term::new("GitHub", &["гитхаб", "гитхап"]),
    ]);
    assert_eq!(saved.entries[0].variants, ["джира", "джайро"]);
    assert_eq!(saved.entries[1].variants, ["гитхаб"], "renamed by the user");
    assert_eq!(saved.seen_variants.len(), 4);
}

#[test]
fn deleted_builtin_variant_stays_deleted() {
    let mut saved = Terms {
        enabled: true,
        entries: vec![Term::new("Jira", &["джира"])],
        seen: vec!["Jira".into()],
        seen_variants: vec![
            ("Jira".into(), "джира".into()),
            ("Jira".into(), "джайро".into()),
        ],
    };
    let before = saved.clone();
    saved.add_new(vec![Term::new("Jira", &["джира", "джайро"])]);
    assert_eq!(saved, before);
}

#[test]
fn a_new_builtin_name_does_not_outrank_the_users_row() {
    let mut saved = Terms {
        enabled: true,
        entries: vec![Term::new("Ютуб", &["ютуб"])],
        ..Terms::default()
    };
    saved.add_new(vec![Term::new("YouTube", &["ютуб"])]);
    let config = Config {
        brands: saved,
        ..Config::disabled()
    };
    assert_eq!(
        boltay_text::process(&config, "Посмотри на ютубе"),
        "Посмотри на Ютубе"
    );
}

#[test]
fn default_list_has_seen_every_builtin_name() {
    let mut brands = Config::default().brands;
    let before = brands.clone();
    brands.add_new(boltay_text::default_brands());
    assert_eq!(brands, before);
}

#[test]
fn full_pipeline_fixes_brands_before_cleanup() {
    let text = "короче, код лежит на гитхабе, открой пулл реквест.";
    assert_eq!(
        boltay_text::process(&Config::default(), text),
        "Короче, код лежит на GitHub, открой pull request."
    );
}

#[test]
fn ambiguous_group_is_off_by_default() {
    let config = Config::default();
    assert!(config.brands.enabled);
    assert!(!config.ambiguous.enabled);
    assert_eq!(
        boltay_text::process(&config, "Десять тонн, курс USD"),
        "Десять тонн, курс USD"
    );
}
