use itertools::Itertools;
use std::collections::HashMap;

pub fn list_locales() -> Vec<String> {
    gnome_desktop::all_locales()
        .iter()
        .map(std::string::ToString::to_string)
        .collect_vec()
}

pub fn get_lang_from_locale(locale: &str) -> Option<(String, String)> {
    if let (Some(lang), Some(native_lang)) = (
        gnome_desktop::language_from_locale(locale, None),
        gnome_desktop::language_from_locale(locale, Some(locale)),
    ) {
        Some((lang.to_string(), native_lang.to_string()))
    } else {
        None
    }
}

fn list(f: impl Fn(&str) -> Option<(String, String)>) -> HashMap<String, (String, String)> {
    (list_locales().into_iter())
        .filter_map(|s| Some((f(&s)?, s))) // avoid clone ∴ flip here (rust bug?)
        .map(|(lang, locale)| (locale, lang))
        .collect() // かなりえぐっ
}

/// A list of `locale_id` -> name of language in English
pub fn list_langs() -> HashMap<String, (String, String)> {
    list(get_lang_from_locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "CI actually doesn't have locales, and the FFI library segfaults due to use-after-free from C"]
    fn test_list_locales() {
        let locales = list_locales();
        assert!(!locales.is_empty());
        assert_eq!(list_langs().len(), locales.len());
    }
    #[ignore = "CI actually doesn't have locales, and the FFI library segfaults due to use-after-free from C"]
    #[test]
    fn test_get_lang_from_locale() {
        assert_eq!(
            get_lang_from_locale("en_US.UTF-8"),
            Some((
                "English (United States)".to_owned(),
                "English (United States)".to_owned()
            ))
        );
    }
}
