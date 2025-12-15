// This is a list of languages sorted by total speakers:
// https://en.wikipedia.org/wiki/List_of_languages_by_total_number_of_speakers
// (2024-08-17)
//
// These are filtered by our Ultramarine website plausible statistics and the 5 most popular
// langauges in the world.
pub const POPULAR_LANGS: [&str; 9] = [
    "en_US", "zh_CN", "zh_TW", "hi_IN", "es_ES", "ar_AE", "fr_FR", "pt_BR", "de_DE",
];
#[derive(Clone, Debug)]
pub struct LanguageRow {
    pub locale: String,
    pub name: String,
    pub native_name: String,
}
impl LanguageRow {
    pub fn list() -> Vec<Self> {
        let c_uloc = rust_icu_uloc::ULoc::try_from("en-US").expect("no ULoc for en-US");
        // FIXME: maybe use some C API instead?
        let mut cmd = std::process::Command::new("localedef");
        cmd.arg("--list-archive")
            .stdout(std::process::Stdio::piped());
        let stdout = cmd.output().expect("cannot run localedef").stdout;
        (stdout.split(|&b| b == b'\n'))
            .filter(|v| !v.contains(&b'.') && !v.contains(&b'@') && !v.is_empty())
            .filter_map(|locale| {
                let locale = core::str::from_utf8(locale).ok()?.to_owned();
                let uloc = rust_icu_uloc::ULoc::try_from(&*locale)
                    .unwrap_or_else(|_| panic!("cannot make ULoc for {locale}"));
                Some(Self {
                    locale,
                    name: (&uloc.display_name(&c_uloc).ok()?).try_into().ok()?,
                    native_name: (&uloc.display_name(&uloc).ok()?).try_into().ok()?,
                })
            })
            .collect()
    }
}
