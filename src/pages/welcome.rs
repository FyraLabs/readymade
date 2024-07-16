use crate::NavigationAction;
use gettextrs::gettext;
use gtk::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

pub const DISTRO: &str = "Ultramarine Linux";

pub struct WelcomePage {}

#[derive(Debug)]
pub enum WelcomePageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum WelcomePageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for WelcomePage {
    type Init = ();
    type Input = WelcomePageMsg;
    type Output = WelcomePageOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 4,
            set_margin_all: 16,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 16,
                set_vexpand: true,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Center,

                gtk::Image {
                    set_from_icon_name: Some("fedora-logo-icon"),
                    inline_css: "-gtk-icon-size: 128px",
                },

                gtk::Label {
                    set_label: &*gettext("Welcome to %s").replace("%s", DISTRO),
                    inline_css: "font-weight: bold; font-size: 1.75rem",
                },

                gtk::Label {
                    set_label: &*gettext(r#"Either test %s from this installer or start the installation now. You can always return to this screen by selecting "Installer" in the menu."#).replace("%s", DISTRO),
                    set_justify: gtk::Justification::Center,
                    set_max_width_chars: 60,
                    set_wrap: true
                },
            },

            gtk::Box {
                set_spacing: 8,
                set_halign: gtk::Align::Center,

                libhelium::PillButton {
                    set_label: &gettext("Try"),
                    inline_css: "padding-left: 48px; padding-right: 48px",
                    connect_clicked => WelcomePageMsg::Navigate(NavigationAction::Quit)
                },

                libhelium::PillButton {
                    set_label: &gettext("Install"),
                    inline_css: "padding-left: 48px; padding-right: 48px",
                    connect_clicked => WelcomePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = WelcomePage {};
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            WelcomePageMsg::Navigate(action) => {
                sender.output(WelcomePageOutput::Navigate(action)).unwrap()
            }
        }
    }
}
