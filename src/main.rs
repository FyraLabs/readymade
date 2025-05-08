#![warn(rust_2018_idioms)]
mod backend;
pub mod cfg;
mod consts;
mod disks;
mod pages;
pub mod prelude;
mod util;

use parking_lot::{Mutex, RwLock};
use std::sync::LazyLock;

use crate::prelude::*;
use backend::install::{InstallationState, InstallationType, IPC_CHANNEL};
use gtk::glib::translate::FromGlibPtrNone;
use i18n_embed::LanguageLoader as _;
use ipc_channel::ipc::IpcSender;
use pages::installation::InstallationPageMsg;
use relm4::SharedState;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();
static CONFIG: SharedState<cfg::ReadymadeConfig> = SharedState::new();

pub static LL: LazyLock<RwLock<i18n_embed::fluent::FluentLanguageLoader>> =
    LazyLock::new(|| RwLock::new(handle_l10n()));

#[derive(rust_embed::RustEmbed)]
#[folder = "po/"]
#[exclude = "en-owo/*.ftl"]
struct Localizations;

static LOCALE_SOLVER: LazyLock<poly_l10n::LocaleFallbackSolver> = LazyLock::new(Default::default);

const APPID: &str = "com.fyralabs.Readymade";

macro_rules! generate_pages {
    ($Page:ident $AppModel:ident $AppMsg:ident: $($page:ident $($forward:expr)?),+$(,)?) => {paste::paste! {
        use pages::{$([<$page:lower>]::[<$page:camel Page>]),+};
        use pages::{$([<$page:lower>]::[<$page:camel PageOutput>]),+};


        #[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
        pub enum $Page {
            #[default]
            $([< $page:camel >]),+
        }

        struct $AppModel {
            page: $Page,
            $(
                [<$page:snake _page>]: relm4::Controller<[<$page:camel Page>]>,
            )+
        }

        impl $AppModel {
            fn _default(sender: ComponentSender<Self>) -> Self {Self {
                page: $Page::default(),
                $(
                    [<$page:snake _page>]: [<$page:camel Page>]::builder()
                        .launch(())
                        .forward(sender.input_sender(), generate_pages!(@$page $AppMsg $($forward)?)),
                )+
            }}
        }
    }};
    (@$page:ident $AppMsg:ident) => {paste::paste! {
        |msg| match msg {
            [<$page:camel PageOutput>]::Navigate(action) => $AppMsg::Navigate(action),
        }
    }};
    (@$page:ident $AppMsg:ident $forward:expr) => { $forward };
}

generate_pages!(Page AppModel AppMsg:
    Language,
    Welcome,
    Destination,
    InstallationType,
    Confirmation |msg| {
        tracing::debug!("ConfirmationPage emitted {msg:?}");
        match msg {
            ConfirmationPageOutput::StartInstallation => AppMsg::StartInstallation,
            ConfirmationPageOutput::Navigate(action) => AppMsg::Navigate(action),
        }
    },
    Installation |msg| {
        tracing::debug!("InstallationPage emitted {msg:?}");
        match msg {
            InstallationPageOutput::Navigate(action) => AppMsg::Navigate(action),
            InstallationPageOutput::SendErr(s) => AppMsg::SendErr(s),
        }
    },
    InstallDual,
    InstallCustom,
    Completed,
    Failure,
);

#[derive(Clone, Debug)]
pub enum NavigationAction {
    GoTo(Page),
    Quit,
}

#[derive(Debug)]
enum AppMsg {
    StartInstallation,
    Navigate(NavigationAction),
    SendErr(String),
}

#[allow(clippy::str_to_string)]
#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();

    type Input = AppMsg;
    type Output = ();

    view! {
        libhelium::ApplicationWindow {
            set_title: Some("Readymade Installer"),
            set_default_width: 550,
            set_default_height: 600,
            set_vexpand: true,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_vexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                #[transition = "SlideLeftRight"]
                match model.page {
                    Page::Language => *model.language_page.widget(),
                    Page::Welcome => *model.welcome_page.widget(),
                    Page::Destination => *model.destination_page.widget(),
                    Page::InstallationType => *model.installation_type_page.widget(),
                    Page::InstallDual => *model.install_dual_page.widget(),
                    Page::InstallCustom => *model.install_custom_page.widget(),
                    Page::Confirmation => *model.confirmation_page.widget(),
                    Page::Installation => *model.installation_page.widget(),
                    Page::Completed => *model.completed_page.widget(),
                    Page::Failure => *model.failure_page.widget(),
                }
            }
        }
    }

    // Initialize the UI.
    fn init(
        (): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // TODO: make libhelium force this
        let display = gtk::gdk::Display::default().unwrap();
        let settings = gtk::Settings::for_display(&display);

        let theme = gtk::IconTheme::for_display(&display);
        theme.add_resource_path("/com/FyraLabs/Readymade/icons");
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let mut model = Self::_default(sender);

        if CONFIG.read().no_langpage {
            model.page = Page::Welcome;
        }

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::StartInstallation => {
                let value = INSTALLATION_STATE.read().installation_type;
                if let Some(InstallationType::Custom) = value {
                    INSTALLATION_STATE.write().mounttags =
                        Some(crate::backend::custom::MountTargets(
                            self.install_custom_page
                                .model()
                                .choose_mount_factory
                                .iter()
                                .cloned()
                                .collect(),
                        ));
                }
                self.installation_page
                    .emit(InstallationPageMsg::StartInstallation);
            }
            AppMsg::Navigate(NavigationAction::GoTo(page)) => self.page = page,

            // FIXME: The following code is commented out because it'd trigger relm4 to drop the
            // RegionPage and LanguagePage, and somehow during quitting, it triggers the signal
            // `:selected_children_changed` which causes the program to crash upon accessing the
            // dropped components.
            //
            // AppMsg::Navigate(NavigationAction::Quit) => relm4::main_application().quit(),
            AppMsg::Navigate(NavigationAction::Quit) => std::process::exit(0),
            AppMsg::SendErr(s) => self
                .failure_page
                .emit(pages::failure::FailurePageMsg::Err(s)),
        }
    }
}

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
    // PERF: this is probably premature optimisation but hey it kinda helps
    let langs_th = std::thread::spawn(|| LazyLock::force(&AVAILABLE_LANGS));
    let _guard = setup_hooks();

    if let Some((i, _)) = std::env::args().find_position(|arg| arg == "--non-interactive") {
        tracing::info!("Running in non-interactive mode");
        // Get installation state from stdin json instead

        let channel = IpcSender::connect(
            std::env::args()
                .nth(i.wrapping_add(1))
                .context("No IPC channel ID passed")?,
        )?;

        IPC_CHANNEL.set(Mutex::new(channel)).unwrap();
        let install_state: backend::install::FinalInstallationState =
            serde_json::from_reader(std::io::stdin())?;

        *LL.write() = handle_l10n();
        langs_th.join().expect("cannot join available_langs_th");
        return install_state.install();
    }

    *CONFIG.write() = cfg::get_cfg()?;
    *INSTALLATION_STATE.write() = InstallationState::from(&*CONFIG.read());

    gtk::gio::resources_register_include!("resources.gresource")?;

    // Load external gresource files for downstream overrides
    // #78
    let gresources = std::fs::read_dir("/usr/share/readymade/resources");
    _ = gresources.into_iter().flatten().try_for_each(|f| {
        let file = f?;
        if file.file_name().as_encoded_bytes().ends_with(b"gresource") {
            gtk::gio::resources_register(&gtk::gio::Resource::load(file.path())?);
        }
        Result::<()>::Ok(())
    });

    let app = libhelium::Application::builder()
        .application_id(APPID)
        .flags(libhelium::gtk::gio::ApplicationFlags::default())
        // SAFETY: placeholder
        .default_accent_color(unsafe {
            &libhelium::RGBColor::from_glib_none(std::ptr::from_mut(
                &mut libhelium::ffi::HeRGBColor {
                    r: 0.0,
                    g: 7.0,
                    b: 143.0,
                },
            ))
        })
        .build();

    tracing::debug!("Starting Readymade");
    *LL.write() = handle_l10n();
    langs_th.join().expect("cannot join available_langs_th");
    RelmApp::from_app(app).run::<AppModel>(());
    Ok(())
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
            std::env::set_var(key, value);
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
        .into_path();
    // create dir
    std::fs::create_dir_all(&temp_dir).expect("create readymade logs tempdir");
    let file_appender = tracing_appender::rolling::never(&temp_dir, readymade_log_file);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().pretty().with_ansi(!no_color::is_no_color()))
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
