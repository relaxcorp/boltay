use std::ops::Range;

use boltay_text::Replacement;

use crate::{Direction, SpanKind};

/// Russian chat abbreviations, spelled out before translation.
const RU_ABBREVIATIONS: &[(&str, &str)] = &[
    ("тз", "техническое задание"),
    ("спс", "спасибо"),
    ("пжл", "пожалуйста"),
    ("плз", "пожалуйста"),
    ("кст", "кстати"),
];

/// Chat slang both translation models get wrong, spelled out in plain Russian first. Word
/// forms are listed one by one: entries match whole words.
const PLAIN: &[(&str, &str)] = &[
    ("норм", "нормально"),
    ("нормас", "нормально"),
    ("отпишусь", "дам знать"),
    ("отпишись", "дай знать"),
    ("отпишитесь", "дайте знать"),
    ("созвон", "звонок"),
    ("созвона", "звонка"),
    ("созвону", "звонку"),
    ("созвоном", "звонком"),
    ("созвоне", "звонке"),
    ("созвоны", "звонки"),
    ("созвонов", "звонков"),
    ("зашквар", "позор"),
    ("зашкварно", "позорно"),
    ("капец", "ужас"),
    ("пипец", "ужас"),
    ("жесть", "кошмар"),
    ("щас", "сейчас"),
    ("ваще", "вообще"),
    ("инфа", "информация"),
    ("инфу", "информацию"),
    ("скинь", "отправь"),
    ("скиньте", "отправьте"),
    ("скину", "отправлю"),
    ("скинул", "отправил"),
    ("скинула", "отправила"),
    ("скинули", "отправили"),
    ("пофиксить", "исправить"),
    ("пофиксил", "исправил"),
    ("пофикшу", "исправлю"),
    ("фидбек", "отзыв"),
    ("фидбэк", "отзыв"),
    ("апрув", "одобрение"),
    ("заапрувить", "одобрить"),
    ("заапрувил", "одобрил"),
    ("косяк", "ошибка"),
    ("косяки", "ошибки"),
    ("накосячил", "ошибся"),
    ("чекнуть", "проверить"),
    ("чекни", "проверь"),
    ("глянь", "посмотри"),
    ("гляну", "посмотрю"),
    ("бабки", "деньги"),
    ("безнал", "безналичный расчёт"),
];

/// Work and IT slang, the base model makes a `rally` of a `митинг` and a `slipper` of a
/// `таска`. Plain words translate right with both models even when an adjective next to
/// them keeps the old gender: `моё проверка кода` still comes out as `my code check`.
const WORK: &[(&str, &str)] = &[
    // `Выкладка` comes out as `layout`. Spelled with `е`: with `ё` the base model
    // makes an `unwrap` of it.
    ("деплой", "развертывание"),
    ("деплоя", "развертывания"),
    ("деплое", "развертывании"),
    ("деплоем", "развертыванием"),
    ("деплои", "развертывания"),
    ("задеплоить", "выложить"),
    ("задеплоил", "выложил"),
    ("задеплоила", "выложила"),
    ("задеплоили", "выложили"),
    ("задеплою", "выложу"),
    ("задеплой", "выложи"),
    ("задеплойте", "выложите"),
    ("задеплоим", "выложим"),
    ("прод", "рабочий сервер"),
    ("прода", "рабочего сервера"),
    ("проду", "рабочему серверу"),
    ("продом", "рабочим сервером"),
    ("проде", "рабочем сервере"),
    ("смержить", "объединить"),
    ("смержи", "объедини"),
    ("смержил", "объединил"),
    ("смержу", "объединю"),
    ("мерж", "слияние"),
    ("закоммитить", "сохранить в репозиторий"),
    ("закоммить", "сохрани в репозиторий"),
    ("закоммитил", "сохранил в репозиторий"),
    ("закоммичу", "сохраню в репозиторий"),
    ("запушить", "отправить на сервер"),
    ("запушь", "отправь на сервер"),
    ("запушил", "отправил на сервер"),
    ("запушу", "отправлю на сервер"),
    ("баг", "ошибка"),
    ("бага", "ошибки"),
    ("багу", "ошибке"),
    ("багом", "ошибкой"),
    ("баге", "ошибке"),
    ("баги", "ошибки"),
    ("багов", "ошибок"),
    ("багами", "ошибками"),
    ("дейли", "ежедневная встреча"),
    ("дейлик", "ежедневная встреча"),
    ("дейлика", "ежедневной встречи"),
    ("дейлику", "ежедневной встрече"),
    ("дейлике", "ежедневной встрече"),
    ("стендап", "ежедневная встреча"),
    ("стендапе", "ежедневной встрече"),
    ("митинг", "встреча"),
    ("митинга", "встречи"),
    ("митингу", "встрече"),
    ("митингом", "встречей"),
    ("митинге", "встрече"),
    ("митинги", "встречи"),
    ("таска", "задача"),
    ("таску", "задачу"),
    ("таски", "задачи"),
    ("таской", "задачей"),
    ("таске", "задаче"),
    ("тасок", "задач"),
    ("таскам", "задачам"),
    ("тасками", "задачами"),
    ("фича", "функция"),
    ("фичу", "функцию"),
    ("фичи", "функции"),
    ("фичей", "функцией"),
    ("фиче", "функции"),
    ("фич", "функций"),
    ("юзер", "пользователь"),
    ("юзера", "пользователя"),
    ("юзеру", "пользователю"),
    ("юзером", "пользователем"),
    ("юзеры", "пользователи"),
    ("юзеров", "пользователей"),
    ("юзерам", "пользователям"),
    ("апдейт", "обновление"),
    ("апдейта", "обновления"),
    ("апдейте", "обновлении"),
    ("апдейты", "обновления"),
    ("ревью", "проверка кода"),
    ("легаси", "старый код"),
    ("блокер", "критическая проблема"),
    ("блокера", "критической проблемы"),
    ("блокеры", "критические проблемы"),
    ("хотфикс", "срочное исправление"),
    ("бэклог", "список задач"),
    ("бэклоге", "списке задач"),
];

/// Text as the translator gets it: chat abbreviations and slang spelled out.
pub(crate) struct Prepared {
    pub text: String,
    pub terms: Vec<Term>,
}

/// One spelled-out abbreviation or slang word.
pub(crate) struct Term {
    pub kind: SpanKind,
    /// Where it is in the original text, in bytes.
    pub original: Range<usize>,
    /// Where its replacement is in [`Prepared::text`].
    pub prepared: Range<usize>,
    /// What it means, in the reader's language.
    pub meaning: String,
}

impl Prepared {
    /// Maps a position in the prepared text back to the original one. Positions inside a
    /// replacement have no counterpart.
    pub fn original(&self, pos: usize) -> Option<usize> {
        let mut shift = 0isize;
        for term in &self.terms {
            if pos < term.prepared.start {
                break;
            }
            if pos < term.prepared.end {
                return None;
            }
            shift += term.prepared.len() as isize - term.original.len() as isize;
        }
        usize::try_from(pos as isize - shift).ok()
    }
}

/// Rewrites text for translation. Applies only on the way to the translator, the Russian
/// output keeps the user's words.
pub fn prepare(text: &str) -> String {
    prepare_detailed(text, Direction::RuEn).text
}

pub(crate) fn prepare_detailed(text: &str, direction: Direction) -> Prepared {
    // (from, to, meaning, kind)
    let mut list: Vec<(&str, &str, &str, SpanKind)> = Vec::new();
    match direction {
        Direction::RuEn => {
            let ru = |kind| move |&(from, to): &(&'static str, &'static str)| (from, to, to, kind);
            list.extend(RU_ABBREVIATIONS.iter().map(ru(SpanKind::Abbreviation)));
            list.extend(PLAIN.iter().map(ru(SpanKind::Slang)));
            list.extend(WORK.iter().map(ru(SpanKind::Slang)));
        }
        Direction::EnRu => return crate::english::prepare(text),
    }
    let entries: Vec<Replacement> = list
        .iter()
        .map(|(from, to, ..)| Replacement::new(*from, *to))
        .collect();
    let (text, replaced) = boltay_text::replace_spans(&entries, text);
    let terms = replaced
        .into_iter()
        .map(|r| Term {
            kind: list[r.entry].3,
            original: r.from,
            prepared: r.to,
            meaning: list[r.entry].2.to_string(),
        })
        .collect();
    Prepared { text, terms }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_out_slang() {
        assert_eq!(
            prepare("Клиент поменял ТЗ, всё норм, отпишусь вечером."),
            "Клиент поменял техническое задание, всё нормально, дам знать вечером."
        );
        assert_eq!(prepare("Капец какой-то"), "Ужас какой-то");
        assert_eq!(
            prepare("Перенесём созвон на завтра"),
            "Перенесём звонок на завтра"
        );
    }

    #[test]
    fn spells_out_english_chat_abbreviations() {
        let text = "Can you send me the creds ASAP? BTW the PR is janky, lmk.";
        let prepared = prepare_detailed(text, Direction::EnRu);
        assert_eq!(
            prepared.text,
            "Can you send me the passwords as soon as possible? By the way the merge request \
             is unreliable, let me know."
        );
        let terms: Vec<(&str, &str, SpanKind)> = prepared
            .terms
            .iter()
            .map(|t| {
                (
                    &text[t.original.clone()],
                    &prepared.text[t.prepared.clone()],
                    t.kind,
                )
            })
            .collect();
        assert_eq!(
            terms,
            [
                ("creds", "passwords", SpanKind::Abbreviation),
                ("ASAP", "as soon as possible", SpanKind::Abbreviation),
                ("BTW", "By the way", SpanKind::Abbreviation),
                ("PR", "merge request", SpanKind::Abbreviation),
                ("janky", "unreliable", SpanKind::Slang),
                ("lmk", "let me know", SpanKind::Abbreviation),
            ]
        );
        assert_eq!(prepared.terms[1].meaning, "как можно скорее");
    }

    #[test]
    fn slang_with_slashes_and_phrases() {
        let prepared = prepare_detailed("tl;dr: w/o tests, no cap. Ping me", Direction::EnRu);
        assert_eq!(
            prepared.text,
            "in short: without tests, no kidding. Write to me"
        );
    }

    #[test]
    fn english_words_that_look_like_abbreviations_stay() {
        for text in [
            "Mr Smith will be there till Friday.",
            "Wait til the end, he said.",
            "He ain't here, the env file is fine.",
            "I bet it works, the U.S. office agrees.",
        ] {
            assert_eq!(prepare_detailed(text, Direction::EnRu).text, text);
        }
    }

    #[test]
    fn russian_terms_have_their_kind() {
        let prepared = prepare_detailed("Спс, ТЗ норм", Direction::RuEn);
        let kinds: Vec<SpanKind> = prepared.terms.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [
                SpanKind::Abbreviation,
                SpanKind::Abbreviation,
                SpanKind::Slang
            ]
        );
        assert_eq!(prepared.text, "Спасибо, техническое задание нормально");
    }

    #[test]
    fn positions_map_back_to_the_original() {
        let text = "FYI ok, BTW fine";
        let prepared = prepare_detailed(text, Direction::EnRu);
        assert_eq!(prepared.text, "For your information ok, by the way fine");
        let at = |word: &str| prepared.text.find(word).unwrap();
        assert_eq!(prepared.original(at("ok")), text.find("ok"));
        assert_eq!(prepared.original(at("fine")), text.find("fine"));
        assert_eq!(prepared.original(at("your")), None);
        assert_eq!(prepared.original(0), None);
        assert_eq!(prepared.original(prepared.text.len()), Some(text.len()));
    }

    #[test]
    fn spells_out_work_slang() {
        assert_eq!(
            prepare("Задеплоил на прод, но на проде баг"),
            "Выложил на рабочий сервер, но на рабочем сервере ошибка"
        );
        assert_eq!(
            prepare("После деплоя на прод нужен хотфикс"),
            "После развертывания на рабочий сервер нужен срочное исправление"
        );
        assert_eq!(prepare("Задеплой на прод"), "Выложи на рабочий сервер");
        assert_eq!(
            prepare("Смержи и запушь, обсудим на дейлике"),
            "Объедини и отправь на сервер, обсудим на ежедневной встрече"
        );
        assert_eq!(
            prepare("Юзеры ждут фичу из этой таски"),
            "Пользователи ждут функцию из этой задачи"
        );
        // The accusative of `митинг` looks like the nominative; the model still reads it
        // right: `We'll move the meeting`.
        assert_eq!(prepare("Перенесём митинг"), "Перенесём встреча");
    }

    #[test]
    fn work_slang_leaves_plain_words() {
        for text in [
            "Багаж потерялся в аэропорту",
            "Не таскай тяжёлое",
            "Продолжение следует",
            "Продукт готов к продаже",
            "Фичер-фильм вышел",
            "Это блокировка счёта",
        ] {
            assert_eq!(prepare(text), text);
        }
    }

    #[test]
    fn leaves_known_words_alone() {
        let text = "Можно перенести дедлайн? Нормально, я согласен.";
        assert_eq!(prepare(text), text);
        // Whole words only: `норма` is not `норм`.
        assert_eq!(prepare("Это норма"), "Это норма");
    }
}
