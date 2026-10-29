use crate::prelude::*;
use crate::ui::LanguageState;
use i18n_embed::LanguageLoader;

#[derive(Clone, Debug)]
struct LanguageRow {
    locale: &'static str,
    name: &'static str,
    native_name: &'static str,
}

taidan_proc_macros::comptime_localedef_langrows!(LANGUAGE_ROWS);

impl LanguageRow {
    fn matches(&self, query: &str) -> bool {
        let query = query.to_ascii_lowercase();
        self.locale.to_ascii_lowercase().starts_with(&query)
            || self.native_name.to_ascii_lowercase().contains(&query)
            || self.name.to_ascii_lowercase().starts_with(&query)
    }

    fn to_ui(&self) -> crate::ui::LanguageRow {
        crate::ui::LanguageRow {
            locale: self.locale.into(),
            name: self.name.into(),
            native_name: self.native_name.into(),
        }
    }
}

/// The part of a locale before the codeset, e.g. `en_US` for `en_US.UTF-8`.
fn strip_codeset(locale: &str) -> &str {
    locale.split_once('.').map_or(locale, |(left, _)| left)
}

pub fn init(ui: &AppWindow) {
    let state = ui.global::<LanguageState<'_>>();
    state.set_rows(model(LANGUAGE_ROWS.iter().map(LanguageRow::to_ui)));

    let weak = ui.as_weak();
    state.on_search(move |query| {
        let Some(ui) = weak.upgrade() else { return };
        ui.global::<LanguageState<'_>>().set_rows(model(
            LANGUAGE_ROWS
                .iter()
                .filter(|lang| lang.matches(&query))
                .map(LanguageRow::to_ui),
        ));
    });

    let weak = ui.as_weak();
    state.on_select(move |locale| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(lang) = LANGUAGE_ROWS.iter().find(|lang| lang.locale == locale.as_str()) else {
            tracing::error!(%locale, "selected locale is not in the list");
            return;
        };
        ui.global::<LanguageState<'_>>().set_selected(locale.clone());
        set_lang(lang);
        crate::ui::relocalize(&ui, &locale);
    });

    // preselect the language the live session is running in
    let system = std::env::var("LANG").unwrap_or_default();
    if let Some(lang) = LANGUAGE_ROWS
        .iter()
        .find(|lang| strip_codeset(lang.locale) == strip_codeset(&system))
    {
        state.invoke_select(lang.locale.into());
    }
}

fn set_lang(lang: &LanguageRow) {
    tracing::info!(lang.locale, "Using selected locale");
    if lang.locale == "en-owo" {
        let loader = i18n_embed::fluent::fluent_language_loader!();
        loader
            .load_languages(&crate::Localizations, &["en-Xowo".parse().unwrap()])
            .expect("fail to load languages");
        *crate::LL.write() = loader;
        crate::APPLICATION_STATE.write().lang = Some("en-US".to_owned());
        return;
    }
    if let Ok(locale) = strip_codeset(lang.locale)
        .parse::<i18n_embed::unic_langid::LanguageIdentifier>()
        .inspect_err(|e| tracing::error!(?e, "Cannot apply language"))
    {
        let mut locales = crate::LOCALE_SOLVER
            .solve_locale(locale)
            .into_iter()
            .filter(|li| crate::AVAILABLE_LANGS.contains(li))
            .collect_vec();
        let loader = i18n_embed::fluent::fluent_language_loader!();
        if locales.is_empty() {
            locales.push("en-US".parse().unwrap());
        }
        loader
            .load_languages(&crate::Localizations, &locales)
            .expect("fail to load languages");
        tracing::debug!(lang=?loader.current_languages(), "new loader");
        *crate::LL.write() = loader;
        crate::APPLICATION_STATE.write().lang = Some(lang.locale.to_owned());
    }
}
