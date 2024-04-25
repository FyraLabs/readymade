use crate::{NavigationAction, INSTALLATION_STATE};
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

pub struct InstallationPage {}

#[derive(Debug)]
pub enum InstallationPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
}

#[derive(Debug)]
pub enum InstallationPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallationPage {
    type Init = ();
    type Input = InstallationPageMsg;
    type Output = InstallationPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: "Installation",
            set_vexpand: true,

            add = &gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_vexpand: true,
                    gtk::Label {
                        set_label: "Some sort of ad/feature thing here idk."
                    },
                },

                gtk::Label {
                  set_label: "Installing base system..."
                },

                libhelium::ProgressBar {
                }
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
        }
    }
}
