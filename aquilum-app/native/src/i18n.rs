//! Строки интерфейса — локали `assets/locales/*.json`: язык по настройке `ui.language` (`ru`, `es`, иначе `en`); ключ без
//! перевода ищется в английской локали, потом возвращается как есть; `{{имя}}` подставляется.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::Value;

/// Язык интерфейса.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En = 0,
    Ru = 1,
    Es = 2,
}

impl Language {
    /// Порядок в списке языков в настройках.
    pub const ALL: [Language; 3] = [Language::Ru, Language::En, Language::Es];

    /// Язык по значению настройки `ui.language`; пустое и незнакомое значение — английский.
    pub fn from_setting(setting: &str) -> Language {
        match setting {
            "ru" => Language::Ru,
            "es" => Language::Es,
            _ => Language::En,
        }
    }

    /// Значение для настройки `ui.language`.
    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Ru => "ru",
            Language::Es => "es",
        }
    }

    /// Название языка на нём самом — для списка в настройках.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Ru => "Русский",
            Language::Es => "Español",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Language::En => include_str!("../assets/locales/en.json"),
            Language::Ru => include_str!("../assets/locales/ru.json"),
            Language::Es => include_str!("../assets/locales/es.json"),
        }
    }
}

static LANGUAGE: AtomicU8 = AtomicU8::new(Language::En as u8);

fn locale(language: Language) -> &'static Value {
    static LOCALES: [OnceLock<Value>; 3] = [const { OnceLock::new() }; 3];
    LOCALES[language as usize].get_or_init(|| serde_json::from_str(language.source()).expect("локаль фронтенда — корректный JSON"))
}

/// Язык по настройке `ui.language`, как `resolveLocale`.
pub fn set_language(setting: &str) {
    LANGUAGE.store(Language::from_setting(setting) as u8, Ordering::Relaxed);
}

/// Текущий язык интерфейса.
pub fn language() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::Ru,
        2 => Language::Es,
        _ => Language::En,
    }
}

fn lookup(language: Language, key: &str) -> Option<&'static str> {
    key.split('.').try_fold(locale(language), |node, part| node.get(part))?.as_str()
}

fn translate_in(language: Language, key: &str) -> Option<&'static str> {
    lookup(language, key).or_else(|| lookup(Language::En, key))
}

fn translate(key: &str) -> Option<&'static str> {
    translate_in(language(), key)
}

/// Строка по ключу.
pub fn t(key: &str) -> String {
    translate(key).unwrap_or(key).to_owned()
}

/// Строка по ключу на заданном языке, а не на языке интерфейса.
pub fn t_in(language: Language, key: &str) -> String {
    translate_in(language, key).unwrap_or(key).to_owned()
}

/// Строка по ключу с подстановкой `{{имя}}`.
pub fn t_with(key: &str, params: &[(&str, &str)]) -> String {
    let mut text = t(key);
    for (name, value) in params {
        text = text.replace(&format!("{{{{{name}}}}}"), value);
    }
    text
}

/// Форма множественного числа по CLDR для целых.
fn plural_form(language: Language, count: u64) -> &'static str {
    match language {
        // 1, 21, 31… — one; 2–4, 22–24… — few; остальное — many.
        Language::Ru => match (count % 10, count % 100) {
            (1, n) if n != 11 => "one",
            (2..=4, n) if !(12..=14).contains(&n) => "few",
            _ => "many",
        },
        Language::En | Language::Es if count == 1 => "one",
        Language::En | Language::Es => "other",
    }
}

fn plural_in(language: Language, key: &str, count: u64) -> String {
    let form = plural_form(language, count);
    let template = translate_in(language, &format!("{key}.{form}")).or_else(|| translate_in(language, &format!("{key}.other"))).unwrap_or(key);
    template.replace("{{count}}", &count.to_string())
}

/// Форма множественного числа (`.one`, `.few`, `.many`, `.other`) с `{{count}}`.
pub fn plural(key: &str, count: u64) -> String {
    plural_in(language(), key, count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    // Тесты не переключают язык всего процесса: от него зависят строки и имена новых файлов в параллельных тестах.
    #[test]
    fn russian_plurals_and_params() {
        assert_eq!(plural_in(Language::Ru, "fileTree.dragCount", 1), "1 заметка");
        assert_eq!(plural_in(Language::Ru, "fileTree.dragCount", 3), "3 заметки");
        assert_eq!(plural_in(Language::Ru, "fileTree.dragCount", 12), "12 заметок");
        assert_eq!(translate_in(Language::Ru, "confirm.deleteMany").map(|text| text.replace("{{count}}", "5")).as_deref(), Some("Удалить 5"));
        assert_eq!(t_with("нет.такого.{{ключа}}", &[("ключа", "ключа")]), "нет.такого.ключа");
        assert_eq!(translate_in(Language::En, "common.delete"), Some("Delete"));
    }

    #[test]
    fn languages_round_trip_through_the_setting() {
        for language in Language::ALL {
            assert_eq!(Language::from_setting(language.code()), language);
        }
        assert_eq!(Language::from_setting(""), Language::En);
        assert_eq!(Language::from_setting("de"), Language::En);
    }

    #[test]
    fn spanish_plurals_and_lookup() {
        assert_eq!(plural_form(Language::Es, 1), "one");
        assert_eq!(plural_form(Language::Es, 0), "other");
        assert_eq!(plural_form(Language::Es, 21), "other");
        assert_eq!(translate_in(Language::Es, "common.delete"), Some("Eliminar"));
        assert_eq!(plural_in(Language::Es, "fileTree.dragCount", 1), "1 nota");
        assert_eq!(plural_in(Language::Es, "fileTree.dragCount", 3), "3 notas");
    }

    fn flat(prefix: &str, node: &Value, out: &mut BTreeMap<String, String>) {
        match node {
            Value::Object(map) => {
                for (key, value) in map {
                    flat(&if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") }, value, out);
                }
            }
            Value::String(text) => {
                out.insert(prefix.to_owned(), text.clone());
            }
            other => panic!("{prefix}: в локали не строка, а {other}"),
        }
    }

    fn params(text: &str) -> Vec<&str> {
        let mut found: Vec<&str> = text.match_indices("{{").filter_map(|(at, _)| text[at + 2..].split_once("}}").map(|(name, _)| name)).collect();
        found.sort_unstable();
        found
    }

    /// Каждая локаль переводит все английские ключи и сохраняет их подстановки `{{имя}}`.
    #[test]
    fn every_locale_covers_the_english_keys_with_the_same_params() {
        let mut english = BTreeMap::new();
        flat("", locale(Language::En), &mut english);
        for language in [Language::Ru, Language::Es] {
            let mut other = BTreeMap::new();
            flat("", locale(language), &mut other);
            for (key, text) in &english {
                // Формы множественного числа у языков разные: сверяется только наличие `.other` или `.many`.
                let Some(translated) = other.get(key).or_else(|| key.strip_suffix(".other").and_then(|base| other.get(&format!("{base}.many")))) else {
                    panic!("{}: нет ключа {key}", language.code());
                };
                assert_eq!(params(translated), params(text), "{}: подстановки в {key}", language.code());
            }
            for key in other.keys() {
                let base = key.rsplit_once('.').map_or(key.as_str(), |(base, _)| base);
                assert!(english.contains_key(key) || english.contains_key(&format!("{base}.other")), "{}: лишний ключ {key}", language.code());
            }
        }
    }
}
