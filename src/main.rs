#[warn(clippy::nursery)]
#[warn(clippy::pedantic)]
mod backend;
mod disks;
mod install;
mod pages;
mod util;

use color_eyre::Result;
use gtk::gio::ApplicationFlags;
use gtk::glib::translate::FromGlibPtrNone;
use gtk::prelude::GtkWindowExt;
use install::InstallationType;
use pages::destination::DiskInit;
use pages::installation::InstallationPageMsg;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, RelmApp, SharedState,
    SimpleComponent,
};
use tracing_subscriber::prelude::*;

#[derive(Debug, Default)]
struct InstallationState {
    pub timezone: Option<String>,
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
}

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();

// todo: lazy_static const variables for the setup params

const APPID: &str = "com.fyralabs.Readymade";

macro_rules! generate_pages {
    ($Page:ident $AppModel:ident $AppMsg:ident: $($page:ident),+$(,)?) => {paste::paste!{
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
                        .forward(sender.input_sender(), |msg| match msg {
                            [<$page:camel PageOutput>]::Navigate(action) => $AppMsg::Navigate(action),
                            #[allow(unreachable_patterns)]
                            _ => unimplemented!("{}PageOutput has unimplemented enum variants", stringify!($page))
                        }),
                )+
            }}
        }
    }};
}

generate_pages!(Page AppModel AppMsg:
    Region,
    Language,
    Welcome,
    Destination,
    InstallationType,
    Confirmation,
    Installation,
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
            #[transition = "SlideLeftRight"]
            set_child = match model.page {
                Page::Region => *model.region_page.widget(),
                Page::Language => *model.language_page.widget(),
                Page::Welcome => *model.welcome_page.widget(),
                Page::Destination => *model.destination_page.widget(),
                Page::InstallationType => *model.installation_type_page.widget(),
                Page::Confirmation => *model.confirmation_page.widget(),
                Page::Installation => *model.installation_page.widget(),
            }
        }
    }

    // Initialize the UI.
    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // TODO: make libhelium force this
        let settings = gtk::Settings::for_display(&gtk::gdk::Display::default().unwrap());
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let model = AppModel {
            confirmation_page: ConfirmationPage::builder().launch(()).forward(
                sender.input_sender(),
                |msg| {
                    tracing::debug!("ConfirmationPage emitted {:?}", msg);
                    match msg {
                        ConfirmationPageOutput::StartInstallation => AppMsg::StartInstallation,
                        ConfirmationPageOutput::Navigate(action) => AppMsg::Navigate(action),
                    }
                },
            ),
            ..Self::_default(sender)
        };

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

fn main() -> Result<()> {
    color_eyre::install()?;

    // Log to a file for debugging
    let tempdir = tempfile::Builder::new().prefix("readymade").tempdir()?;
    let file_appender = tracing_appender::rolling::never(tempdir.path(), "readymade.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let sub_builder = tracing_subscriber::fmt()
        // .with_writer(non_blocking)
        // .with_writer(std::io::stderr)
        .with_env_filter("trace")
        .with_ansi(true)
        .pretty().finish()
        .with(
            tracing_subscriber::fmt::Layer::default().with_writer(non_blocking)
        )
        .with(
            tracing_subscriber::fmt::Layer::default().with_writer(std::io::stderr)
        );

    // let subscriber = Registry::default();

    tracing::subscriber::set_global_default(sub_builder).expect("unable to set global subscriber");

    // we probably want to escalate the process to root on release builds

    #[cfg(not(debug_assertions))]
    karen::builder()
    // .with_env("DISPLAY")
    .wrapper("pkexec")
    .with_env(&[])
    // .escalate_if_needed()
    .unwrap();

    #[cfg(debug_assertions)]
    {
        tracing::info!("Running in debug mode");
    }

    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
    );

    tracing::info!(
        "Logging to {tempdir}/readymade.log",
        tempdir = tempdir.path().display()
    );
    gettextrs::textdomain(APPID)?;
    gettextrs::bind_textdomain_codeset(APPID, "UTF-8")?;

    let app = libhelium::Application::builder()
        .application_id(APPID)
        .flags(ApplicationFlags::default())
        .default_accent_color(unsafe {
            &libhelium::ColorRGBColor::from_glib_none(&mut libhelium::ffi::HeColorRGBColor {
                // todo: fix this upstream
                r: 0.0,
                g: 7.0 / 255.0,
                b: 143.0 / 255.0,
            } as *mut _)
        })
        .build();

    tracing::debug!("Starting Readymade");
    RelmApp::from_app(app).run::<AppModel>(());
    Ok(())
}
