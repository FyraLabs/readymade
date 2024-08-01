use std::fmt::Write;

use crate::prelude::*;
use crate::NavigationAction;
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

const BUG_REPORT_LINK: &str = "https://github.com/FyraLabs/readymade/issues";
const BUG_REPORT_MSG: &str = "If you believe the failure is caused by a bug in this installer, we would appreciate a bug report. You may click the button below to open up the issue tracking webpage.";

#[derive(Debug, Default)]
pub struct FailurePage {
    buffer: gtk::TextBuffer,
}

#[derive(Debug)]
pub enum FailurePageMsg {
    Navigate(NavigationAction),
    ReportBug,
    Err(String),
}

#[derive(Debug)]
pub enum FailurePageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for FailurePage {
    type Init = ();
    type Input = FailurePageMsg;
    type Output = FailurePageOutput;

    view! {
        libhelium::ViewMono {
            #[watch]
            set_title: &gettext("Installation Failure"),
            set_vexpand: true,
            set_hexpand: true,
            // set_halign: gtk::Align::Center,
            // set_valign: gtk::Align::Center,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                set_hexpand: true,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,

                gtk::Label {
                    #[watch]
                    set_label: &gettext("The installation process failed."),
                    set_justify: gtk::Justification::Center,
                    set_max_width_chars: 60,
                    set_wrap: true
                },

                gtk::Label {
                    #[watch]
                    set_label: &gettext(BUG_REPORT_MSG),
                    set_justify: gtk::Justification::Center,
                    set_max_width_chars: 60,
                    set_wrap: true
                },

                gtk::TextView {
                    inline_css: "monospace: true",
                    add_css_class: "text-view",

                    set_buffer: Some(&model.buffer),
                },

                // TODO: box for displaying logs

                gtk::Box {
                    set_spacing: 8,
                    set_halign: gtk::Align::Center,

                    libhelium::PillButton {
                        #[watch]
                        set_label: &gettext("Close"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => FailurePageMsg::Navigate(NavigationAction::Quit)
                    },

                    libhelium::PillButton {
                        #[watch]
                        set_label: &gettext("Report a bug"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
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

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            FailurePageMsg::Err(s) => self.buffer.write_str(&s).unwrap(),
            FailurePageMsg::ReportBug => {
                open::that(BUG_REPORT_LINK).unwrap();
            }
            FailurePageMsg::Navigate(nav) => _ = sender.output(FailurePageOutput::Navigate(nav)),
        }
    }
}
