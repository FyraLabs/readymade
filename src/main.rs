mod albius;
mod disks;
mod mksys;
mod pages;
mod util;

use color_eyre::Result;
use gtk::gio::ApplicationFlags;
use gtk::glib::translate::FromGlibPtrNone;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt};
use libhelium::prelude::*;
use pages::destination::{DestinationPageOutput, DiskInit};
use pages::installation::{InstallationPage, InstallationPageMsg};
use pages::welcome::WelcomePageOutput;
use pages::{destination::DestinationPage, welcome::WelcomePage};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, ContainerChild, Controller,
    RelmApp, RelmSetChildExt, RelmWidgetExt, SharedState, SimpleComponent,
};

use crate::pages::installation::InstallationPageOutput;

#[derive(Debug, Default)]
struct InstallationState {
    pub destination_disk: Option<DiskInit>,
}

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();

// todo: lazy_static const variables for the setup params

// todo: GtkStack for paging

// todo: wizard

// the code is non-existent, but the boilerplate is there

const APPID: &str = "com.fyralabs.Readymade";

#[derive(Debug)]
pub enum NavigationAction {
    Back,
    Forward,
    Quit,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Page {
    Welcome,
    Destination,
    Installation,
}

const PAGES: [Page; 3] = [Page::Welcome, Page::Destination, Page::Installation];

struct AppModel {
    page: Page,

    welcome_page: Controller<WelcomePage>,
    destination_page: Controller<DestinationPage>,
    installation_page: Controller<InstallationPage>,
}

#[derive(Debug)]
enum AppMsg {
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
                Page::Welcome => *model.welcome_page.widget(),
                Page::Destination => *model.destination_page.widget(),
                Page::Installation => *model.installation_page.widget(),
            }
        }
    }

    // Initialize the UI.
    fn init(
        counter: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // TODO: make libhelium force this
        let settings = gtk::Settings::for_display(&gtk::gdk::Display::default().unwrap());
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let model = AppModel {
            page: Page::Welcome,
            welcome_page: WelcomePage::builder()
                .launch(())
                .forward(sender.input_sender(), |msg| match msg {
                    WelcomePageOutput::Navigate(action) => AppMsg::Navigate(action),
                }),
            destination_page: DestinationPage::builder().launch(()).forward(
                sender.input_sender(),
                |msg| match msg {
                    DestinationPageOutput::Navigate(action) => AppMsg::Navigate(action),
                },
            ),
            installation_page: InstallationPage::builder().launch(()).forward(
                sender.input_sender(),
                |msg| match msg {
                    InstallationPageOutput::Navigate(action) => AppMsg::Navigate(action),
                },
            ),
        };

        // Insert the macro code generation here
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Navigate(NavigationAction::Forward) => {
                self.page = PAGES[PAGES.iter().position(|&p| p == self.page).unwrap() + 1];
            }
            AppMsg::Navigate(NavigationAction::Back) => {
                self.page = PAGES[PAGES.iter().position(|&p| p == self.page).unwrap() - 1];
            }
            AppMsg::Navigate(NavigationAction::Quit) => relm4::main_application().quit(),
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_ansi(true)
        .pretty()
        .init();

    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
    );

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
    Ok(app.run::<AppModel>(0))
}
