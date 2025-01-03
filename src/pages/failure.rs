use crate::prelude::*;
use crate::NavigationAction;
use crate::INSTALLATION_STATE;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};
use std::fmt::Write;

const BUG_REPORT_LINK: &str = "https://github.com/FyraLabs/readymade/issues";

#[derive(Debug, Default)]
pub struct FailurePage {
    buffer: gtk::TextBuffer,
}

#[derive(Debug)]
pub enum FailurePageMsg {
    Navigate(NavigationAction),
    ReportBug,
    Err(String),
    Update,
}

#[derive(Debug)]
pub enum FailurePageOutput {
    Navigate(NavigationAction),
}

// TODO: Logs should be hidden behind a dropdown or other button

#[relm4::component(pub)]
impl SimpleComponent for FailurePage {
    type Init = ();
    type Input = FailurePageMsg;
    type Output = FailurePageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &gettext("Installation Failure"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,
            set_hexpand: false,

            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::ScrolledWindow {
                    gtk::TextView {
                        set_vexpand: true,

                        inline_css: "monospace: true",
                        add_css_class: "text-view",

                        set_buffer: Some(&model.buffer),
                    },
                },

                // TODO: box for displaying logs

                gtk::Box {
                    set_spacing: 4,
                    set_orientation: gtk::Orientation::Horizontal,

                    libhelium::Button {
                        set_is_textual: true,
                        #[watch]
                        set_label: &gettext("Close"),
                        add_css_class: "large-button",
                        connect_clicked => FailurePageMsg::Navigate(NavigationAction::Quit)
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Report a bug"),
                        add_css_class: "large-button",
                        connect_clicked => FailurePageMsg::ReportBug,
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
        let model = Self::default();

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| FailurePageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FailurePageMsg::Err(s) => self.buffer.write_str(&s).unwrap(),
            FailurePageMsg::ReportBug => gtk::UriLauncher::new(BUG_REPORT_LINK).launch(
                Option::<&libhelium::Window>::None,
                gtk::gio::Cancellable::NONE,
                |_| {},
            ),
            FailurePageMsg::Navigate(nav) => _ = sender.output(FailurePageOutput::Navigate(nav)),
            FailurePageMsg::Update => {}
        }
    }
}
