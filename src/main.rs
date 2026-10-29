#![warn(rust_2018_idioms)]
mod cfg;
mod pages;
mod prelude;
mod state;
mod ui;

use parking_lot::RwLock;
use std::sync::LazyLock;

use crate::{
    prelude::*,
    state::{APPLICATION_STATE, ApplicationState},
};
use i18n_embed::LanguageLoader as _;
use libreadymade::playbook::Playbook;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// State related to the user's installation configuration
static CONFIG: LazyLock<RwLock<cfg::ReadymadeConfig>> = LazyLock::new(Default::default);

pub static LL: LazyLock<RwLock<i18n_embed::fluent::FluentLanguageLoader>> =
    LazyLock::new(|| RwLock::new(handle_l10n()));

#[derive(rust_embed::RustEmbed)]
#[folder = "po/"]
#[exclude = "en-owo/*.ftl"]
struct Localizations;

static LOCALE_SOLVER: LazyLock<poly_l10n::LocaleFallbackSolver> = LazyLock::new(Default::default);

static AVAILABLE_LANGS: LazyLock<Vec<i18n_embed::unic_langid::LanguageIdentifier>> =
    LazyLock::new(|| {
        i18n_embed::fluent::fluent_language_loader!()
            .available_languages(&Localizations)
            .unwrap()
    });

fn handle_l10n() -> i18n_embed::fluent::FluentLanguageLoader {
    use i18n_embed::LanguageLoader;
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let mut langs = poly_l10n::system_want_langids()
        .flat_map(|li| LOCALE_SOLVER.solve_locale(li))
        .filter(|li| AVAILABLE_LANGS.contains(li))
        .collect_vec();
    if langs.is_empty() {
        langs = vec![loader.fallback_language().clone()];
    }
    loader.load_languages(&Localizations, &langs).unwrap();
    loader
}

#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
fn main() -> Result<()> {
    let _guard = sentry::init((
        "https://d5f6d7f57fee8ac5deac757e05d1b0bd@o271654.ingest.us.sentry.io/4510871707713536",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            send_default_pii: false,
            auto_session_tracking: true,
            session_mode: sentry::SessionMode::Application,
            enable_logs: true,
            ..Default::default()
        },
    ));
    // PERF: this is probably premature optimisation but hey it kinda helps
    let langs_th = std::thread::spawn(|| LazyLock::force(&AVAILABLE_LANGS));
    let _guard = setup_hooks();

    if let Some((i, _)) = std::env::args().find_position(|arg| arg == "--non-interactive") {
        tracing::info!("Running in non-interactive mode");
        let playbook: Playbook = serde_json::from_reader(std::io::stdin())?;
        let progress = if let Some(channel_id) = std::env::args().nth(i.wrapping_add(1)) {
            let sender = ipc_channel::ipc::IpcSender::connect(channel_id)?;
            let (tx, rx) = Playbook::channel();
            std::thread::spawn(move || {
                if let Err(err) = rx.into_iter().try_for_each(|msg| sender.send(msg)) {
                    tracing::error!("Failed to send progress to parent process: {err:?}");
                }
            });
            tx
        } else {
            let (tx, _rx) = Playbook::channel();
            tx
        };

        *LL.write() = handle_l10n();
        langs_th.join().expect("cannot join available_langs_th");
        return playbook
            .play(progress)
            .inspect_err(|e| _ = sentry_eyre::capture_report(e));
    }

    *CONFIG.write() = cfg::get_cfg()?;
    *APPLICATION_STATE.write() = ApplicationState::from(&*CONFIG.read());

    tracing::debug!("Starting Readymade");
    *LL.write() = handle_l10n();
    langs_th.join().expect("cannot join available_langs_th");
    ui::run()
}

/// Returns a logging guard.
///
/// # Panics
/// - cannot install `color_eyre`
/// - cannot create readymade tempdir
#[allow(clippy::cognitive_complexity)]
fn setup_hooks() -> impl std::any::Any {
    for arg in std::env::args() {
        if arg.starts_with("READYMADE_")
            || arg.starts_with("REPART_COPY_SOURCE")
            || arg.starts_with("NO_COLOR")
        {
            let (key, value) = arg.split_once('=').unwrap();
            println!("Setting env var {key} to {value}");
            unsafe { std::env::set_var(key, value) };
        }
    }

    let is_non_interactive = std::env::args().any(|arg| arg == "--non-interactive");

    let readymade_log_file = if is_non_interactive {
        "readymade-non-interactive.log"
    } else {
        "readymade.log"
    };

    color_eyre::install().expect("install color_eyre");
    let temp_dir = tempfile::Builder::new()
        .prefix("readymade-logs")
        .tempdir()
        .expect("create readymade logs tempdir")
        .keep();
    // create dir
    std::fs::create_dir_all(&temp_dir).expect("create readymade logs tempdir");
    let file_appender = tracing_appender::rolling::never(&temp_dir, readymade_log_file);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().pretty().with_ansi(!no_color::is_no_color()))
        .with(sentry::integrations::tracing::layer())
        .with(EnvFilter::from_env("READYMADE_LOG"))
        .with(
            tracing_journald::layer()
                .unwrap()
                .with_syslog_identifier("readymade".to_owned()),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .compact(),
        )
        .init();

    if cfg!(debug_assertions) {
        tracing::info!("Running in debug mode");
    }
    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
    );
    tracing::info!("Logging to journald");
    tracing::info!(
        "Logging to {tmp}/readymade.log",
        tmp = temp_dir.to_string_lossy()
    );
    guard
}
