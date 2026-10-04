#[macro_use]
mod common;

use boltay_text::{Config, Profanity};

fn mode(profanity: Profanity) -> Config {
    Config {
        profanity,
        ..Config::disabled()
    }
}

fn mask() -> Config {
    mode(Profanity::Mask)
}

fn soften() -> Config {
    mode(Profanity::Soften)
}

fn remove() -> Config {
    mode(Profanity::Remove)
}

/// Words that look like swearing to a naive filter. Checked in the most aggressive mode.
mod traps {
    use super::*;

    untouched! { mask();
        korablya: "Капитан корабля",
        rublya: "Без одного рубля",
        upotreblyat: "Не стоит употреблять",
        potreblenie: "Потребление выросло",
        oskorblyat: "Не надо оскорблять",
        strakhuy: "Страхуй меня",
        podstrakhuy: "Подстрахуй его",
        zastrakhuyu: "Я застрахую машину",
        sablya: "Острая сабля",
        ogloblya: "Сломалась оглобля",
        komanda: "Наша команда",
        skipidar: "Банка скипидара",
        khlebalo: "Закрой хлебало",
        ebenya: "Живёт в ебенях",
        ebeniya: "Уехал в ебеня",
        eburg: "Еду в Ебург",
        mandarin: "Сладкий мандарин",
        mandat: "Депутатский мандат",
        mandrazh: "Перед экзаменом мандраж",
        sebe: "Возьми себе",
        sebya: "Держи себя в руках",
        sebestoimost: "Себестоимость выросла",
        otsebyatina: "Это отсебятина",
        veb: "Веб-сайт и вебинар",
        vebka: "Включи вебку",
        sukno: "Зелёное сукно",
        sukin: "Сукин сын",
        suk: "Сук дерева",
        suchya: "Сухие сучья",
        suchka: "Сучка лает",
        blyakha: "Бляха ремня",
        blyakha_mukha: "Бляха-муха, опять",
        govno: "Это говно",
        manda: "Какая-то манда",
        pokhudet: "Хочу похудеть",
        khuligan: "Он хулиган",
        khutor: "Дальний хутор",
        khuan: "Его зовут Хуан",
        mudry: "Мудрый совет",
        pobeda: "Наша победа",
        nebesa: "Синие небеса",
        debil: "Тест на дебилизм",
        sedobny: "Гриб съедобный",
        razezd: "Разъезд закрыт",
        uchebnik: "Новый учебник",
        khleb: "Свежий хлеб",
        greben: "Гребень горы",
        oblako: "Белое облако",
        sukhuyu: "Проиграли всухую, сухую траву",
        yamashita: "Yamashita and Matsushita",
        assume: "I assume so",
        class: "First class pass",
        assist: "Can you assist me",
        assassin: "An assassin",
        shiitake: "Shiitake mushrooms",
        shitake: "Shitake soup",
        scunthorpe: "Scunthorpe United",
        cocktail: "A cocktail",
        dickens: "Charles Dickens",
    }
}

mod masking {
    use super::*;

    cases! { mask();
        blyat: "Блять, опять" => "Б**ть, опять",
        blyad: "Ну блядь" => "Ну б**дь",
        blya: "Бля" => "Б*я",
        caps: "БЛЯТЬ" => "Б**ТЬ",
        pizdets: "Это пиздец" => "Это п****ц",
        spizdil: "Кто спиздил?" => "Кто с*****л?",
        nakhuy: "Иди нахуй" => "Иди н***й",
        ni_khuya: "Ни хуя" => "Ни х*я",
        okhuenno: "Охуенно" => "О*****о",
        zaebal: "Ты заебал" => "Ты з****л",
        sebalsya: "Он съебался" => "Он с******я",
        vyblyadok: "Выблядок" => "В******к",
        dolboyob: "Долбоёб" => "Д*****б",
        yobany: "Ёбаный стыд" => "Ё****й стыд",
        ebat: "Ебать" => "Е**ть",
        suka: "Сука" => "С**а",
        mudak: "Мудак" => "М***к",
        zalupa: "Залупа" => "З****а",
        pokhuy: "Мне похуй" => "Мне п***й",
        hyphen_part: "Пиздец-то какой" => "П****ц-то какой",
        with_punctuation: "(блять)" => "(б**ть)",
        fuck: "Fuck" => "F**k",
        motherfucker: "Motherfucker" => "M**********r",
        bullshit: "Bullshit" => "B******t",
        bitch: "You bitch" => "You b***h",
        asshole: "An asshole" => "An a*****e",
        ass: "Kiss my ass" => "Kiss my a*s",
    }

    cases! { Config::default();
        keep_is_default: "Блять, опять" => "Блять, опять",
    }
}

mod softening {
    use super::*;

    cases! { soften();
        blyat_interjection: "Блять, опять" => "Блин, опять",
        blya_inside: "Я бля не знаю" => "Я блин не знаю",
        pizdets: "Это пиздец" => "Это капец",
        pizdets_case: "Справились с пиздецом" => "Справились с капцом",
        pizdets_caps: "ПИЗДЕЦ" => "КАПЕЦ",
        khuynya: "Это хуйня" => "Это фигня",
        khuynyu: "Хуйню несёшь" => "Фигню несёшь",
        okhuenno: "Охуенно!" => "Офигенно!",
        okhuenny: "Охуенный вид" => "Офигенный вид",
        okhuel: "Он охуел" => "Он офигел",
        okhuitelno: "Охуительно" => "Офигительно",
        nakhuy: "Иди нахуй" => "Иди нафиг",
        pokhuy: "Мне похуй" => "Мне пофиг",
        do_khuya: "До хуя денег" => "До фига денег",
        nikhuya: "Нихуя себе" => "Нифига себе",
        zaebal: "Он меня заебал" => "Он меня достал",
        zaebalsya: "Я заебался" => "Я задолбался",
        zaebis: "Заебись!" => "Зашибись!",
        ebat_interjection: "Ебать, как холодно" => "Ёлки, как холодно",
        ebat_verb_masks: "Ебать его" => "Е**ть его",
        suka_interjection: "Сука, опять" => "Блин, опять",
        suka_exclamation: "Опять опоздал, сука!" => "Опять опоздал, блин!",
        suka_noun_masks: "Эта сука" => "Эта с**а",
        pizdaty: "Пиздатая тачка" => "Клёвая тачка",
        spizdil: "Кто спиздил?" => "Кто стырил?",
        ebany: "Ебаный дождь" => "Долбаный дождь",
        ebanutyi: "Ебанутый" => "Долбанутый",
        ebanulsya: "Ты ебанулся?" => "Ты рехнулся?",
        mudak: "Этот мудак" => "Этот придурок",
        mudakom: "С мудаком" => "С придурком",
        uebok: "Уёбок" => "Урод",
        raspizdyay: "Распиздяй" => "Раздолбай",
        hyphen_part: "Пиздец-то какой" => "Капец-то какой",
        no_soft_form_masks: "Залупа" => "З****а",
        wtf: "What the fuck" => "What the heck",
        fuck_you_masks: "Fuck you" => "F**k you",
        fuck_off_masks: "Just fuck off" => "Just f**k off",
        fuck_it_masks: "Fuck it, let's go" => "F**k it, let's go",
        fuck_interjection_softens: "Fuck, I forgot" => "Heck, I forgot",
        ebet_mozgi: "Он ебёт мозги" => "Он выносит мозги",
        ebesh_mozgi: "Не еби мне мозги, ты ебёшь мозги" => "Не выноси мне мозги, ты выносишь мозги",
        ebut_mozgi: "Они ебут мозги" => "Они выносят мозги",
        ebet_mne_mozgi: "Он ебёт мне мозги" => "Он выносит мне мозги",
        ebal_mozg: "Ебал мозг весь день" => "Выносил мозг весь день",
        ebet_without_brains_masks: "Кого это ебёт" => "Кого это е**т",
        fucking: "Fucking great" => "Freaking great",
        holy_shit: "Holy shit" => "Holy crap",
        bitch: "He is a bitch" => "He is a jerk",
        motherfucker_masks: "Motherfucker" => "M**********r",
    }

    untouched! { soften();
        trap_korablya: "Капитан корабля",
        trap_sebe: "Возьми себе",
        clean_text: "Всё хорошо, спасибо",
    }
}

mod removal {
    use super::*;

    cases! { remove();
        opening: "Блять, опять опоздал" => "Опять опоздал",
        between_commas: "Я, блять, опоздал" => "Я опоздал",
        inside: "Это пиздец какой-то" => "Это какой-то",
        closing: "Опоздал, сука." => "Опоздал.",
        whole_phrase: "Блять." => "",
        several: "Блять, сука, опять" => "Опять",
        second_sentence: "Опять. Блять, опоздал." => "Опять. Опоздал.",
        hyphen_part: "Пиздец-то" => "то",
        english_opening: "Fuck, I forgot" => "I forgot",
        english_inside: "It is fucking great" => "It is great",
        english_closing: "Oh, shit." => "Oh.",
    }

    untouched! { remove();
        trap_skipidar: "Банка скипидара",
        trap_class: "First class pass",
    }
}
