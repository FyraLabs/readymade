use crate::prelude::*;
use crate::state;
use crate::ui::{Bento, InstallationState, Nav};
use l10n::{BENTO_ASSETS_PATH, BENTO_LOADER as L};
use libreadymade::playbook::PlaybookProgress;

mod l10n {
    use const_format::formatcp;
    use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
    use i18n_embed::{FileSystemAssets, LanguageLoader as _, unic_langid::LanguageIdentifier};
    use itertools::Itertools;
    use std::str::FromStr;
    use std::sync::{Arc, LazyLock};

    /// NOTE: This is sed-ed by the install.sh script!
    #[cfg(not(debug_assertions))]
    pub(super) const BENTO_ASSETS_PATH: &str = "/usr/share/readymade/bento/";

    #[cfg(debug_assertions)]
    pub(super) const BENTO_ASSETS_PATH: &str = "";

    type B = Box<dyn i18n_embed::I18nAssets + Send + Sync>;

    static BENTO_ASSETS: LazyLock<Arc<B>> = LazyLock::new(|| {
        Arc::new(
            FileSystemAssets::try_new(formatcp!("{BENTO_ASSETS_PATH}po/"))
                .inspect_err(|e| tracing::error!(?e, "Cannot load assets in {BENTO_ASSETS_PATH}"))
                .inspect_err(|_| tracing::warn!("Falling back to global compile-time assets"))
                .map_or_else(
                    |_| Box::new(crate::Localizations) as B,
                    |a| Box::new(a) as B,
                ),
        )
    });

    static BENTO_AVAILABLE_LANGS: LazyLock<Vec<LanguageIdentifier>> = LazyLock::new(|| {
        fluent_language_loader!()
            .available_languages(&***BENTO_ASSETS)
            .unwrap()
    });

    // WARN: this is written under the assumption that the language is not changed.
    // This assumption is true as long as Readymade doesn't handle the language selections.
    pub(super) static BENTO_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
        let loader = fluent_language_loader!();
        let mut langs = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE", "LANGUAGES"]
            .into_iter()
            .flat_map(|env| {
                std::env::var(env).ok().into_iter().flat_map(|locales| {
                    locales
                        .split(':')
                        .filter_map(|locale| LanguageIdentifier::from_str(locale).ok())
                        .collect_vec()
                })
            })
            .flat_map(|li| crate::LOCALE_SOLVER.solve_locale(li))
            .filter(|li| BENTO_AVAILABLE_LANGS.contains(li))
            .collect_vec();
        if langs.is_empty() {
            langs = vec![loader.fallback_language().clone()];
        }
        loader.load_languages(&***BENTO_ASSETS, &langs).unwrap();
        loader
    });
}

/// Load bento background `n` (and its dark variant) installed by `install.sh`.
///
/// Returns empty images when they aren't installed, in which case the UI falls back to the ones
/// embedded at build time.
fn bento_background(n: usize) -> (slint::Image, slint::Image) {
    let load = |name: String| {
        let path = std::path::Path::new(BENTO_ASSETS_PATH).join(name);
        if !path.exists() {
            return slint::Image::default();
        }
        slint::Image::load_from_path(&path)
            .inspect_err(|e| tracing::debug!(?path, ?e, "cannot load bento background"))
            .unwrap_or_default()
    };
    (load(n.to_string()), load(format!("{n}-dark")))
}

pub fn init(ui: &AppWindow) {
    let cfg = crate::CONFIG.read();
    let bentos = cfg.bentos.iter().enumerate().map(|(i, bento)| {
        let (background, background_dark) = bento_background(i + 1);
        Bento {
            title: L
                .get_args(&bento.title, [("distro", &cfg.distro.name)].into())
                .into(),
            desc: L.get(&bento.desc).into(),
            link: bento.link.to_shared_string(),
            icon: crate::ui::icon(&bento.icon, 28),
            background,
            background_dark,
        }
    });
    ui.global::<InstallationState<'_>>().set_bentos(model(bentos));
}

/// Build the playbook from the user's choices and run it in the background.
pub fn start(ui: &AppWindow) {
    let playbook = {
        let custom_mounts = (APPLICATION_STATE.read().installation_type
            == Some(crate::cfg::InstallationType::Custom))
        .then(crate::pages::installcustom::mounts);
        APPLICATION_STATE
            .read()
            .to_playbook(&crate::CONFIG.read(), custom_mounts)
    };
    let playbook = match playbook {
        Ok(playbook) => playbook,
        Err(err) => return crate::pages::failure::show(ui, &format!("{err:?}")),
    };

    ui.global::<InstallationState<'_>>()
        .set_status(t!("page-installation-progress").into());
    ui.global::<Nav<'_>>().set_page(Page::Installation);

    let weak = ui.as_weak();
    std::thread::spawn(move || {
        tracing::debug!(?playbook, "Starting installation...");
        let progress_weak = weak.clone();
        let res = state::install_using_subprocess(&playbook, move |progress| {
            let status = match progress {
                PlaybookProgress::Stage(status) | PlaybookProgress::StageProgress(status) => {
                    status
                }
                PlaybookProgress::PostModule(module, index, total) => {
                    format!("Post-install {} / {}: {}", index + 1, total, module)
                }
            };
            _ = progress_weak.upgrade_in_event_loop(move |ui| {
                ui.global::<InstallationState<'_>>()
                    .set_status(status.into());
            });
        });
        _ = weak.upgrade_in_event_loop(move |ui| match res {
            Ok(()) => ui.global::<Nav<'_>>().set_page(Page::Completed),
            Err(e) => crate::pages::failure::show(&ui, &format!("{e:?}")),
        });
    });
}
