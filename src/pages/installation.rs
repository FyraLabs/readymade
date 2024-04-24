use crate::{NavigationAction, INSTALLATION_STATE};
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, WidgetTemplate,
};

use super::destination::DiskInit;

pub struct InstallationPage {}

#[derive(Debug)]
pub enum InstallationPageMsg {
    Update,
    #[doc(hidden)]
    Navigate(NavigationAction),
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
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::Center,
                    set_spacing: 16,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 2,
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,

                        gtk::Image {
                            set_icon_name: Some("drive-harddisk"),
                            inline_css: "-gtk-icon-size: 128px"
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.disk_name).unwrap_or("".to_owned()),
                            inline_css: "font-size: 16px; font-weight: bold"
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.os_name).unwrap_or("".to_owned()),
                        }
                    },

                    gtk::Box {
                        set_spacing: 8,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::End,
                        set_homogeneous: true,
                        libhelium::PillButton {
                            set_label: "Erase & Install",
                            inline_css: "padding-left: 48px; padding-right: 48px"
                        },
                        libhelium::PillButton {
                            set_label: "Dual Boot",
                            inline_css: "padding-left: 48px; padding-right: 48px"

                        },
                        libhelium::PillButton {
                            set_label: "Custom",
                            inline_css: "padding-left: 48px; padding-right: 48px",
                        }
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::TextButton {
                        set_label: "Previous",
                        connect_clicked => InstallationPageMsg::Navigate(NavigationAction::Back)
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: "Next",
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => InstallationPageMsg::Navigate(NavigationAction::Forward),
                        #[watch]
                        set_sensitive: INSTALLATION_STATE.read().destination_disk.is_some()
                    }
                }
            }
        }
    }

    fn init(
        init: Self::Init,
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