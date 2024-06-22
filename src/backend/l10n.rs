extern crate alloc;
use alloc::ffi::CString;
use std::collections::HashMap;
use std::ffi::CStr;

pub fn list_locales() -> Vec<String> {
    let ptr = unsafe { gnome_desktop::ffi::gnome_get_all_locales() };
    let mut p = ptr;
    let mut res = vec![];
    while !unsafe { p.read().is_null() } {
        res.push(
            unsafe { CStr::from_ptr(p.read()) }
                .to_string_lossy()
                .to_string(),
        );
        p = unsafe { p.add(1) };
    }
    unsafe { gtk::glib::ffi::g_strfreev(ptr) };
    res
}

#[inline]
unsafe fn _get_ffi(locale: &str, f: impl Fn(*const i8, *const i8) -> *mut i8) -> Option<String> {
    CString::from_raw(f(
        // SAFETY:
        // `locale` is from list_locales() which uses `CStr::from_ptr()`, guaranteeing no `\0`s
        CString::new(locale).unwrap_unchecked().as_ptr(),
        std::ptr::null(), // after thorough testing, Mado has confirmed this param doesn't work
    ))
    .into_string()
    .ok()
}
pub fn get_lang_from_locale(locale: &str) -> Option<String> {
    // this is as simple as how it can be, there's no way to further refactor it
    // blame rust for lack of something like `impl unsafe Fn()`?
    unsafe {
        _get_ffi(locale, |x, y| {
            gnome_desktop::ffi::gnome_get_language_from_locale(x, y)
        })
    }
}
pub fn get_region_from_locale(locale: &str) -> Option<String> {
    unsafe {
        _get_ffi(locale, |x, y| {
            gnome_desktop::ffi::gnome_get_country_from_locale(x, y)
        })
    }
}

fn _list(f: impl Fn(&str) -> Option<String>) -> HashMap<String, String> {
    (list_locales().into_iter())
        .filter_map(|s| Some((f(&s)?, s))) // avoid clone ∴ flip here (rust bug?)
        .map(|(lang, locale)| (locale, lang))
        .collect() // かなりえぐっ
}
/// A list of locale_id -> name of region
pub fn list_regions() -> HashMap<String, String> {
    _list(get_region_from_locale)
}
/// A list of locale_id -> name of language in English
pub fn list_langs() -> HashMap<String, String> {
    _list(get_lang_from_locale)
}

pub fn list_timezones() -> Vec<&'static str> {
    chrono_tz::TZ_VARIANTS.iter().map(|tz| tz.name()).collect()
}


#[test]
fn test_list_locales() {
    let locales = list_locales();
    println!("{:?}", locales);
    assert!(!locales.is_empty());
    assert_eq!(list_regions().len(), locales.len());
    assert_eq!(list_langs().len(), locales.len());
}

#[test]
fn test_get_lang_from_locale() {
    assert_eq!(
        get_lang_from_locale("en_US.UTF-8"),
        Some("English (United States)".into())
    );
}
