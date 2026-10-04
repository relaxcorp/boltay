use crate::settings::UiLanguage;

/// The interface language: the user's choice, otherwise Russian for a Russian-speaking
/// system, otherwise English.
pub fn resolve(choice: Option<UiLanguage>) -> UiLanguage {
    choice.unwrap_or_else(|| from_locale(sys_locale::get_locale().as_deref()))
}

fn from_locale(locale: Option<&str>) -> UiLanguage {
    let lang = locale
        .unwrap_or("")
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    // Belarusian, Ukrainian and Kazakh systems usually have Russian readers too.
    match lang.as_str() {
        "ru" | "be" | "uk" | "kk" => UiLanguage::Ru,
        _ => UiLanguage::En,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    Ready,
    Recording,
    Recognizing,
    NoModel,
    Loading,
    Downloading,
    Settings,
    History,
    CopyLast,
    Translator,
    Transcribe,
    Logs,
    /// `{v}` stands for the version.
    Update,
    Quit,
}

pub fn tr(lang: UiLanguage, text: Text) -> &'static str {
    use Text::*;
    match (lang, text) {
        (UiLanguage::En, Ready) => "Ready",
        (UiLanguage::Ru, Ready) => "Готов",
        (UiLanguage::En, Recording) => "Listening…",
        (UiLanguage::Ru, Recording) => "Слушаю…",
        (UiLanguage::En, Recognizing) => "Recognizing…",
        (UiLanguage::Ru, Recognizing) => "Распознаю…",
        (UiLanguage::En, NoModel) => "Model not found",
        (UiLanguage::Ru, NoModel) => "Модель не найдена",
        (UiLanguage::En, Loading) => "Loading the model…",
        (UiLanguage::Ru, Loading) => "Загружаю модель…",
        (UiLanguage::En, Downloading) => "Downloading the model…",
        (UiLanguage::Ru, Downloading) => "Скачиваю модель…",
        (UiLanguage::En, Settings) => "Settings…",
        (UiLanguage::Ru, Settings) => "Настройки…",
        (UiLanguage::En, History) => "History…",
        (UiLanguage::Ru, History) => "История…",
        (UiLanguage::En, CopyLast) => "Copy last dictation",
        (UiLanguage::Ru, CopyLast) => "Скопировать последнее",
        (UiLanguage::En, Translator) => "Translator",
        (UiLanguage::Ru, Translator) => "Переводчик",
        (UiLanguage::En, Transcribe) => "Transcribe audio…",
        (UiLanguage::Ru, Transcribe) => "Расшифровать аудио…",
        (UiLanguage::En, Update) => "Version {v} is out",
        (UiLanguage::Ru, Update) => "Доступна версия {v}",
        (UiLanguage::En, Logs) => "Open logs folder",
        (UiLanguage::Ru, Logs) => "Открыть папку логов",
        (UiLanguage::En, Quit) => "Quit Boltay",
        (UiLanguage::Ru, Quit) => "Выйти из Boltay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_choice_wins() {
        assert_eq!(resolve(Some(UiLanguage::En)), UiLanguage::En);
        assert_eq!(resolve(Some(UiLanguage::Ru)), UiLanguage::Ru);
    }

    #[test]
    fn russian_locales() {
        for locale in [
            "ru",
            "ru-RU",
            "ru_RU.UTF-8",
            "RU",
            "be-BY",
            "uk-UA",
            "kk-KZ",
        ] {
            assert_eq!(from_locale(Some(locale)), UiLanguage::Ru, "{locale}");
        }
    }

    #[test]
    fn everything_else_is_english() {
        for locale in ["en-US", "de-DE", "C", "", "rus"] {
            assert_eq!(from_locale(Some(locale)), UiLanguage::En, "{locale}");
        }
        assert_eq!(from_locale(None), UiLanguage::En);
    }
}
