#![warn(rust_2018_idioms)]
mod backend;
mod disks;
mod install;
mod pages;
pub mod prelude;
mod util;

use crate::prelude::*;
use color_eyre::Result;
use install::{InstallationState, InstallationType};
use libhelium::glib::translate::FromGlibPtrNone;
use pages::installation::InstallationPageMsg;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, RelmApp, SharedState,
    SimpleComponent,
};
use tracing_subscriber::prelude::*;

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();

// todo: lazy_static const variables for the setup params

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
    Installation,
    Completed,
    Failure,
);

#[derive(Debug)]
pub enum NavigationAction {
    GoTo(Page),
    Quit,
}

#[derive(Debug)]
enum AppMsg {
    StartInstallation,
    Navigate(NavigationAction),
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
            set_default_height: 400,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                libhelium::AppBar {
                },
                #[transition = "SlideLeftRight"]
                match model.page {
                    Page::Language => *model.language_page.widget(),
                    Page::Welcome => *model.welcome_page.widget(),
                    Page::Destination => *model.destination_page.widget(),
                    Page::InstallationType => *model.installation_type_page.widget(),
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
        let settings = gtk::Settings::for_display(&gtk::gdk::Display::default().unwrap());
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let model = Self::_default(sender);

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::StartInstallation => self
                .installation_page
                .emit(InstallationPageMsg::StartInstallation),
            AppMsg::Navigate(NavigationAction::GoTo(page)) => {
                self.page = page;
                // FIXME: welcome page doesn't automatically update under diff language
                if page == Page::Welcome {
                    self.welcome_page
                        .emit(pages::welcome::WelcomePageMsg::Refresh);
                }
            }
            // FIXME: The following code is commented out because it'd trigger relm4 to drop the
            // RegionPage and LanguagePage, and somehow during quitting, it triggers the signal
            // `:selected_children_changed` which causes the program to crash upon accessing the
            // dropped components.
            //
            // AppMsg::Navigate(NavigationAction::Quit) => relm4::main_application().quit(),
            AppMsg::Navigate(NavigationAction::Quit) => std::process::exit(0),
        }
    }
}
// todo: non-interactive mode?
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
fn main() -> Result<()> {
    setup_logs_and_install_panic_hook();

    if std::env::args().any(|arg| arg == "--non-interactive") {
        // Get installation state from stdin json instead

        let install_state: InstallationState = serde_json::from_reader(std::io::stdin())?;

        return install_state.install();
    }

    gettextrs::textdomain(APPID)?;
    gettextrs::bind_textdomain_codeset(APPID, "UTF-8")?;

    let app = libhelium::Application::builder()
        .application_id(APPID)
        .flags(libhelium::gtk::gio::ApplicationFlags::default())
        // SAFETY: just doing weird low-level pointer stuff
        .default_accent_color(unsafe {
            &libhelium::ColorRGBColor::from_glib_none(std::ptr::from_mut(&mut libhelium::ffi::HeColorRGBColor {
                // todo: fix this upstream
                r: 0.0,
                g: 7.0 / 255.0,
                b: 143.0 / 255.0,
            }))
        })
        .build();

    tracing::debug!("Starting Readymade");
    RelmApp::from_app(app).run::<AppModel>(());
    Ok(())
}

/// # Panics
/// - cannot install `color_eyre`
/// - cannot create readymade tempdir
fn setup_logs_and_install_panic_hook() {
    color_eyre::install().expect("install color_eyre");
    let file_appender = tracing_appender::rolling::never(std::env::temp_dir(), "readymade.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let sub_builder = tracing_subscriber::fmt()
        .with_env_filter("trace")
        // todo: somehow make the non-blocking writer also log levels higher than info? or something
        .with_writer(non_blocking)
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .pretty()
        .finish()
        // Log to journald, to
        .with(tracing_journald::layer()
            .expect("unable to create journald layer")
            .with_syslog_identifier("readymade".to_string())
            // todo: log trace too??? why does it only log info
            // make layers log levels higher than info
            );
    tracing::subscriber::set_global_default(sub_builder).expect("unable to set global subscriber");
    if cfg!(debug_assert) {
        tracing::info!("Running in debug mode");
    }
    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
    );
    tracing::info!("Logging to journald");
    tracing::info!(
        "Logging to {tmp}/readymade.log",
        tmp = std::env::temp_dir().to_string_lossy()
    );

    let _guard = guard;
}
