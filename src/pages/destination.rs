use crate::NavigationAction;
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

pub struct DestinationPage {}

#[derive(Debug)]
pub enum DestinationPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum DestinationPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for DestinationPage {
    type Init = ();
    type Input = DestinationPageMsg;
    type Output = DestinationPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: "Destination",
            set_vexpand: true,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 4,
                set_vexpand: true,
                set_hexpand: true,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 2,

                    gtk::Image {
                        set_icon_name: Some("drive-harddisk"),
                        inline_css: "-gtk-icon-size: 128px"
                    },

                    gtk::Label {
                        set_label: "Seagate HDD",
                        inline_css: "font-size: 16px; font-weight: bold"
                    },

                    gtk::Label {
                        set_label: "Ubuntu 20.04"
                    }
                }
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 4,

                libhelium::TextButton {
                    set_label: "Previous",
                    connect_clicked => DestinationPageMsg::Navigate(NavigationAction::Back)
                },

                gtk::Box {
                    set_hexpand: true,
                },

                libhelium::PillButton {
                    set_label: "Next",
                                        inline_css: "padding-left: 48px; padding-right: 48px",
                }
            }
            }
            // insert logo here i guess, branding time
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            DestinationPageMsg::Navigate(action) => sender
                .output(DestinationPageOutput::Navigate(action))
                .unwrap(),
        }
    }
}
