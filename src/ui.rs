use std::path::Path;

use crate::prelude::*;

slint::include_modules!();

/// Icon themes to look in, in order. Hydrogen is what the live ISO ships.
fn icon_themes() -> impl Iterator<Item = String> {
    ["Hydrogen".to_owned()]
        .into_iter()
        .chain(freedesktop_icons::default_theme_gtk())
        .chain(["Adwaita".to_owned(), "hicolor".to_owned()])
}

/// Load an icon by its freedesktop name (or an absolute path, as some desktop entries do).
///
/// Returns an empty image if it cannot be found, which just renders as nothing.
pub fn icon(name: &str, size: u16) -> slint::Image {
    let path = if Path::new(name).is_absolute() {
        Some(Path::new(name).to_path_buf())
    } else {
        icon_themes().find_map(|theme| {
            freedesktop_icons::lookup(name)
                .with_theme(&theme)
                .with_size(size)
                .with_cache()
                .find()
        })
    };
    let Some(path) = path else {
        tracing::warn!(name, "cannot find icon");
        return slint::Image::default();
    };
    slint::Image::load_from_path(&path)
        .inspect_err(|e| tracing::warn!(?path, ?e, "cannot load icon"))
        .unwrap_or_default()
}

/// The family fontconfig would use for `monospace`, since Slint doesn't resolve generic families.
fn monospace_family() -> Option<String> {
    let out = std::process::Command::new("fc-match")
        .args(["--format=%{family[0]}", "monospace"])
        .output()
        .inspect_err(|e| tracing::warn!(?e, "cannot run fc-match"))
        .ok()?;
    let family = String::from_utf8(out.stdout).ok()?;
    (out.status.success() && !family.is_empty()).then_some(family)
}

pub fn open_url(url: &str) {
    if let Err(e) = open::that_detached(url) {
        tracing::error!(url, ?e, "cannot open url");
    }
}

fn init_l10n(ui: &AppWindow) {
    ui.global::<Lang<'_>>().on__t(|id, args, _current_locale| {
        let mut fargs = fluent::FluentArgs::new();
        for FluentArg { key, value } in args.iter() {
            // numbers need to stay numbers so that plural selectors work
            match value.parse::<f64>() {
                Ok(num) => fargs.set(key.to_string(), num),
                Err(_) => fargs.set(key.to_string(), value.to_string()),
            }
        }
        crate::LL.read().get_args_fluent(&id, Some(&fargs)).into()
    });
}

/// Re-evaluate every translated string in the UI after [`crate::LL`] has been swapped out.
pub fn relocalize(ui: &AppWindow, locale: &str) {
    ui.global::<Lang<'_>>().set_current_locale(locale.into());
}

fn init_nav(ui: &AppWindow) {
    let nav = ui.global::<Nav<'_>>();
    // NOTE: just exit, the installer has nothing to clean up at this point
    nav.on_quit(|| std::process::exit(0));
    nav.on_open_url(|url| open_url(&url));
}

pub fn run() -> Result<()> {
    let ui = AppWindow::new()?;
    let cfg = crate::CONFIG.read();

    let distro = ui.global::<Distro<'_>>();
    distro.set_name(cfg.distro.name.to_shared_string());
    distro.set_icon(icon(&cfg.distro.icon, 128));
    ui.global::<Icons<'_>>()
        .set_drive(icon("drive-harddisk", 128));
    if let Some(family) = monospace_family() {
        ui.global::<Fonts<'_>>().set_mono(family.into());
    }

    init_l10n(&ui);
    init_nav(&ui);
    crate::pages::init(&ui);

    let nav = ui.global::<Nav<'_>>();
    nav.set_page(if cfg.no_langpage {
        Page::Welcome
    } else {
        Page::Language
    });

    if !cfg.distro.bios_support && !std::fs::exists("/sys/firmware/efi").is_ok_and(|x| x) {
        crate::pages::failure::show(&ui, &t!("err-no-bios"));
    }
    drop(cfg);

    // closing the window mid-install would leave a half-written disk behind
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if weak
            .upgrade()
            .is_some_and(|ui| ui.global::<Nav<'_>>().get_page() == Page::Installation)
        {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            slint::CloseRequestResponse::HideWindow
        }
    });

    ui.run()?;
    Ok(())
}
