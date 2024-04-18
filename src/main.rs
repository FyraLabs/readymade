mod albius;
mod pages;
mod util;
mod disks;
use std::ops::{Deref, Index};

use gtk::gio::ApplicationFlags;
use gtk::glib::translate::FromGlibPtrNone;
use gtk::prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt};
use libhelium::prelude::*;
use pages::destination::DestinationPageOutput;
use pages::welcome::WelcomePageOutput;
use pages::{destination::DestinationPage, welcome::WelcomePage};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, ContainerChild, Controller,
    RelmApp, RelmSetChildExt, RelmWidgetExt, SimpleComponent,
};

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
}

const PAGES: [Page; 2] = [Page::Welcome, Page::Destination];

struct AppModel {
    page: Page,

    welcome_page: Controller<WelcomePage>,
    destination_page: Controller<DestinationPage>,
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
            },
        }
    }

    // Initialize the UI.
    fn init(
        counter: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
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

fn main() {
    crate::disks::detect_os();
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
}
