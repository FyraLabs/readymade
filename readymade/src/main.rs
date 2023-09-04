mod release;
mod util;
use color_eyre::eyre::{eyre, Result};
use gettextrs::{bind_textdomain_codeset, gettext, LocaleCategory};
use gtk::gio::ffi::{g_resources_register, GResource};
use gtk::glib;
use gtk::{prelude::*, subclass::prelude::*};
// TODO: Do a GUI and CLI for this, maybe.

mod views;
mod window;

const APP_ID: &str = "com.fyralabs.Readymade";

fn main() -> Result<()> {
    color_eyre::install()?;
    // set up tracing for logfmt output
    tracing_subscriber::fmt()
        .pretty()
        .with_level(true)
        .with_max_level(tracing::Level::DEBUG)
        .init();
    tracing::debug!("Hello, world!");
    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::textdomain("readymade")?;
    bind_textdomain_codeset("readymade", "UTF-8")?;
    let os = crate::release::release_root("/")?;
    tracing::debug!(?os, "/etc/os-release");

    let app = he::Application::new(Some(APP_ID), Default::default());

    app.connect_activate(|app| {
        app.active_window()
            .unwrap_or(window::ApplicationWindow::new(&app).upcast())
            .present();
    });

    Err(eyre!("Application exited with code {}", app.run().value()))
}
