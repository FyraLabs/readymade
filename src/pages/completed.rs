use crate::prelude::*;
use crate::NavigationAction;
use gettextrs::gettext;
use libhelium::prelude::{ButtonExt, HeButtonExt, ViewExt};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Default)]
pub struct CompletedPage;

#[derive(Debug)]
pub enum CompletedPageMsg {
    Reboot,
    Close,
}

#[derive(Debug)]
pub enum CompletedPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for CompletedPage {
    type Init = ();
    type Input = CompletedPageMsg;
    type Output = CompletedPageOutput;

    view! {
        libhelium::ViewMono {
            #[watch]
            set_title: &gettext("Completed"),
            set_vexpand: true,

            add = &gtk::Box {
                gtk::Label {
                    #[watch]
                    set_label: &gettext("Installation complete. You may reboot now and enjoy your fresh system."),
                    set_justify: gtk::Justification::Center,
                    set_max_width_chars: 60,
                    set_wrap: true
                },

                gtk::Box {
                    set_spacing: 8,
                    set_halign: gtk::Align::Center,

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Close"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => CompletedPageMsg::Close,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Reboot"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => CompletedPageMsg::Reboot,
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            CompletedPageMsg::Reboot => _ = crate::util::run_as_root("systemctl reboot").unwrap(),
            CompletedPageMsg::Close => sender
                .output(CompletedPageOutput::Navigate(NavigationAction::Quit))
                .unwrap(),
        }
    }
}
