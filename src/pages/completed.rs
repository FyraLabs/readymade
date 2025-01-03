use crate::prelude::*;
use crate::NavigationAction;
use crate::INSTALLATION_STATE;
use gettextrs::gettext;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

#[derive(Debug, Default)]
pub struct CompletedPage;

#[derive(Debug)]
pub enum CompletedPageMsg {
    Reboot,
    Close,
    Update,
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
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &gettext("Completed"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,

            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                set_vexpand: true,
                set_hexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 16,
                    set_vexpand: true,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,

                    gtk::Image {
                        set_icon_name: Some(&crate::CONFIG.read().distro.icon),
                        inline_css: "-gtk-icon-size: 128px",
                    },

                    gtk::Label {
                        #[watch]
                        set_label: &gettext("Installation complete. You may reboot now and enjoy your fresh system."),
                        set_justify: gtk::Justification::Center,
                        set_max_width_chars: 60,
                        set_wrap: true
                    },
                },

                gtk::Box {
                    set_spacing: 4,

                    libhelium::Button {
                        set_is_textual: true,
                        #[watch]
                        set_label: &gettext("Close"),
                        add_css_class: "large-button",
                        connect_clicked => CompletedPageMsg::Close,
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Reboot"),
                        add_css_class: "large-button",
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

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| CompletedPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            CompletedPageMsg::Reboot => {
                // supposedly it should run pkexec automatically?
                _ = std::process::Command::new("systemctl")
                    .arg("reboot")
                    .status();
            }
            CompletedPageMsg::Close => sender
                .output(CompletedPageOutput::Navigate(NavigationAction::Quit))
                .unwrap(),
            CompletedPageMsg::Update => {}
        }
    }
}
