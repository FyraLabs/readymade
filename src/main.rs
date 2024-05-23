#[warn(clippy::nursery)]
#[warn(clippy::pedantic)]
mod albius;
mod backend;
mod disks;
mod install;
mod pages;
mod setup;
mod util;

use color_eyre::Result;
use gtk::gio::ApplicationFlags;
use gtk::glib::translate::FromGlibPtrNone;
use gtk::prelude::GtkWindowExt;
use libhelium::prelude::*;
use pages::destination::{DestinationPageOutput, DiskInit};
use pages::installation::InstallationPageMsg;
use pages::installationtype::InstallationTypePageOutput;
use pages::welcome::WelcomePageOutput;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, RelmApp, SharedState,
    SimpleComponent,
};

use crate::pages::confirmation::ConfirmationPageOutput;
use crate::pages::installation::InstallationPageOutput;
use crate::pages::language::LanguagePageOutput;
use crate::pages::region::RegionPageOutput;

// TODO: move this to somewhere else in backend
#[derive(Debug)]
enum InstallationType {
    WholeDisk,
    DualBoot, //??
    Custom,   // config???
}

#[derive(Debug, Default)]
struct InstallationState {
    pub timezone: Option<&'static str>,
    pub langlocale: Option<String>,
    pub destination_disk: Option<DiskInit>,
    pub installation_type: Option<InstallationType>,
}

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();

// todo: lazy_static const variables for the setup params

// todo: GtkStack for paging

// todo: wizard

// the code is non-existent, but the boilerplate is there

const APPID: &str = "com.fyralabs.Readymade";

macro_rules! generate_pages {
    ($Page:ident $AppModel:ident: $($page:ident),+$(,)?) => {paste::paste!{
        use pages::{$([<$page:lower>]::[<$page:camel Page>]),+};


        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub enum $Page {
            $([< $page:camel >]),+
        }

        struct $AppModel {
            page: $Page,
            $(
                [<$page:snake _page>]: relm4::Controller<[<$page:camel Page>]>,
            )+
        }

        // FIXME: this doesn't work. See the match statement in `impl SimpleComponent  for AppModel`.
        //
        // macro_rules! model_page_mapping {
        //     ($model:ident) => {{
        //         match $model.page {$(
        //             $Page::[<$page:camel>] => *$model.[<$page:snake _page>].widget(),
        //         )+}
        //     }};
        // }
    }};
}

macro_rules! make_page_init {
    ($Page:ident, $sender:ident, $AppMsg:ident) => {{
        $Page::builder()
            .launch(())
            .forward($sender.input_sender(), |msg| match msg {
                paste::paste! {[<$Page Output>]::Navigate(action)} => $AppMsg::Navigate(action),
            })
    }};
}

generate_pages!(Page AppModel:
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
    type Init = u8;

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
        _counter: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // TODO: make libhelium force this
        let settings = gtk::Settings::for_display(&gtk::gdk::Display::default().unwrap());
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let model = AppModel {
            page: Page::Region, // first screen
            region_page: make_page_init!(RegionPage, sender, AppMsg),
            language_page: make_page_init!(LanguagePage, sender, AppMsg),
            welcome_page: make_page_init!(WelcomePage, sender, AppMsg),
            destination_page: make_page_init!(DestinationPage, sender, AppMsg),
            installation_type_page: make_page_init!(InstallationTypePage, sender, AppMsg),
            confirmation_page: ConfirmationPage::builder().launch(()).forward(
                sender.input_sender(),
                |msg| match msg {
                    ConfirmationPageOutput::StartInstallation => AppMsg::StartInstallation,
                    ConfirmationPageOutput::Navigate(action) => AppMsg::Navigate(action),
                },
            ),
            installation_page: make_page_init!(InstallationPage, sender, AppMsg),
        };

        // Insert the macro code generation here
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
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_ansi(true)
        .pretty()
        .init();

    // we probably want to escalate the process to root on release builds

    #[cfg(not(debug_assertions))]
    karen::builder().wrapper("pkexec").escalate_if_needed()?;

    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
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

    let app = RelmApp::from_app(app);
    app.run::<AppModel>(0);
    Ok(())
}
