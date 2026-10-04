use crate::config::Term;
use crate::token::is_word_char;

/// Names as they are written, and how Russian speech models spell them when they mishear:
/// in Cyrillic, in a Latin transliteration, split or glued. Case endings are matched on
/// their own, `гитхабе` needs no entry. Every variant here is a word nobody uses otherwise;
/// one that is also an everyday word comes with the preposition it is said with: `на прот`
/// is the server, `прот` alone is protein.
const BRANDS: &[(&str, &[&str])] = &[
    (
        "GitHub",
        &[
            "гитхаб",
            "гитхап",
            "гит хаб",
            "гидхаб",
            "гитхуб",
            "githab",
            "git hub",
            "githap",
        ],
    ),
    (
        "Spotify",
        &[
            "спотифай",
            "спатифай",
            "спотифи",
            "spotyfy",
            "spatify",
            "spotifay",
            "spotifi",
        ],
    ),
    (
        "Faceit",
        &["фейсит", "фэйсит", "фейс ит", "фейсайт", "facet", "face it"],
    ),
    ("Discord", &["дискорд", "дискорт", "diskord", "discort"]),
    ("Telegram", &["телеграм", "telegramm"]),
    (
        "WhatsApp",
        &[
            "ватсап",
            "вотсап",
            "вацап",
            "воцап",
            "ватсапп",
            "вотсапп",
            "whats app",
            "whatsap",
            "watsap",
        ],
    ),
    ("Docker", &["докер", "доккер", "dokker", "doker"]),
    ("Python", &["пайтон", "phyton", "paiton"]),
    ("Svelte", &["свелт", "свельт", "svelt", "swelt", "svelta"]),
    (
        "бэкенд",
        &[
            "бекенд",
            "бэкэнд",
            "бэк энд",
            "bekend",
            "bekkend",
            "back end",
        ],
    ),
    (
        "фронтенд",
        &["фронтэнд", "фронт энд", "frontent", "front end"],
    ),
    ("на прод", &["на прот"]),
    (
        "хотфикс",
        &["хот фикс", "ход фикс", "hot fix", "hod fix", "hotfix"],
    ),
    ("окей", &["о'кей", "о’кей"]),
    (
        "pull request",
        &[
            "пулл реквест",
            "пул реквест",
            "пулреквест",
            "пуллреквест",
            "pull-request",
            "pulrequest",
        ],
    ),
    (
        "ChatGPT",
        &[
            "чатгпт",
            "чат гпт",
            "чат джипити",
            "чат жпт",
            "чат gpt",
            "chat gpt",
            "chat-gpt",
            "chad gpt",
        ],
    ),
    ("Claude", &["клод", "клоуд"]),
    (
        "Counter-Strike 2",
        &[
            "контр страйк 2",
            "каунтер страйк 2",
            "контра 2",
            "counterstrike 2",
            "counter strike 2",
        ],
    ),
    (
        "Counter-Strike",
        &[
            "контр страйк",
            "каунтер страйк",
            "контрстрайк",
            "counterstrike",
            "counter strike",
        ],
    ),
    ("Steam", &["стим"]),
    ("Twitch", &["твич", "твитч", "twich"]),
    (
        "Binance",
        &["бинанс", "байнанс", "бинанце", "binans", "bainance"],
    ),
    (
        "TikTok",
        &[
            "тикток",
            "тик ток",
            "тик-ток",
            "tik tok",
            "tiktok",
            "tick tock",
        ],
    ),
    (
        "YouTube",
        &["ютуб", "ютюб", "ю туб", "you tube", "youtub", "utube"],
    ),
    ("Avito", &["авито"]),
    ("Ozon", &["озон"]),
    (
        "Wildberries",
        &[
            "вайлдберриз",
            "вайлдбериз",
            "вайлдберис",
            "валдберис",
            "вайлберис",
            "вилдберис",
            "wild berries",
            "wildberis",
            "wildberies",
        ],
    ),
    (
        "Яндекс Маркет",
        &[
            "яндекс маркет",
            "яндекс-маркет",
            "яндексмаркет",
            "yandex market",
        ],
    ),
    ("USDT", &["юсдт", "юсдэти", "ю эс ди ти", "usdt"]),
    ("VK", &["вк"]),
    ("ВКонтакте", &["вконтакте", "вконтакт"]),
    ("Сбер", &["сбер"]),
    ("Сбербанк", &["сбербанк", "сбер банк"]),
    ("Т-Банк", &["т банк", "тбанк", "ти банк", "тибанк"]),
    ("Тинькофф", &["тинькофф", "тинькоф"]),
    ("Альфа-Банк", &["альфа банк", "альфабанк"]),
    ("Госуслуги", &["госуслуги", "гос услуги"]),
    ("Яндекс", &["яндекс", "yandex"]),
    ("Яндекс Такси", &["яндекс такси", "яндекстакси"]),
    ("Яндекс Еда", &["яндекс еда"]),
    ("Яндекс Музыка", &["яндекс музыка"]),
    ("Яндекс Диск", &["яндекс диск"]),
    ("Яндекс Go", &["яндекс го", "яндекс гоу"]),
    ("Яндекс Браузер", &["яндекс браузер"]),
    ("Кинопоиск", &["кинопоиск", "кино поиск"]),
    ("Kaspi", &["каспи", "каспе"]),
    (
        "Lamoda",
        &["ламода", "ламодо", "ла модо", "la modo", "lamodo"],
    ),
    ("Instagram", &["инстаграм", "инстаграмм", "инста"]),
    ("Notion", &["ноушн", "ноушен", "ноушин", "ношн"]),
    ("Figma", &["фигма"]),
    ("Jira", &["джира", "джайро"]),
    ("Trello", &["трелло", "трело", "трейло"]),
    ("в Trello", &["в трейле"]),
    ("Slack", &["слак", "слэк"]),
    (
        "Kubernetes",
        &["кубернетес", "кубернетис", "кубернейтс", "кубер"],
    ),
    ("GitLab", &["гитлаб", "гит лаб"]),
    ("Postman", &["постман"]),
    (
        "VS Code",
        &["вс код", "ви эс код", "вскод", "vs код", "vs cod"],
    ),
    ("README", &["ридми", "рид ми"]),
    ("Excel", &["эксель", "иксель", "ексель"]),
    ("Word", &["ворд"]),
    (
        "PowerPoint",
        &["пауэрпоинт", "пауэр поинт", "поверпоинт", "павер поинт"],
    ),
    ("Google Docs", &["гугл докс", "гугл док"]),
    ("Google Sheets", &["гугл таблиц", "гугл шитс"]),
    ("Google Drive", &["гугл драйв", "гугл диск", "google disk"]),
];

/// Worth fixing for someone who talks crypto and games all day, wrong for everyone else:
/// `тонн` is a weight, `USD` is a dollar, `Face` and `питон` are plain words, `КС-2` is
/// a construction paperwork form, `в контакте` means keeping in touch, `жира` is fat, `зум`
/// is a camera zoom, `фотошоп` any edited picture, `Тиньков` and `Миро` are surnames.
const AMBIGUOUS: &[(&str, &[&str])] = &[
    ("TON", &["тонн", "тонкоин"]),
    ("Counter-Strike 2", &["кс 2"]),
    ("USDT", &["usd", "юсд"]),
    ("Faceit", &["face", "фейс"]),
    ("Python", &["питон"]),
    ("pull request", &["реквест"]),
    ("ВКонтакте", &["в контакте"]),
    ("Тинькофф", &["тиньков"]),
    ("Zoom", &["зум"]),
    ("Jira", &["жира"]),
    ("Photoshop", &["фотошоп"]),
    ("Miro", &["миро"]),
];

/// The recognizer spells Yandex services the way they were written before the rename,
/// `Яндекс.Музыка`, or glued, `ЯндексМузыке`. Split into two words, the list knows them in
/// every case.
pub(crate) fn split_yandex(text: &str) -> String {
    const NAME: &str = "Яндекс";
    let mut out = String::with_capacity(text.len() + 4);
    let mut rest = text;
    while let Some(at) = rest.find(NAME) {
        let after = &rest[at + NAME.len()..];
        let word_start = rest[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let service = after.strip_prefix('.').unwrap_or(after);
        out.push_str(&rest[..at + NAME.len()]);
        rest = after;
        if word_start && service.chars().next().is_some_and(char::is_uppercase) {
            out.push(' ');
            rest = service;
        }
    }
    out.push_str(rest);
    out
}

fn terms(list: &[(&str, &[&str])]) -> Vec<Term> {
    list.iter()
        .map(|(name, variants)| Term::new(*name, variants))
        .collect()
}

pub fn default_brands() -> Vec<Term> {
    terms(BRANDS)
}

pub fn default_ambiguous() -> Vec<Term> {
    terms(AMBIGUOUS)
}
