use crate::prelude::*;
use std::fmt::Write;

const BUG_REPORT_LINK: &str = "https://github.com/FyraLabs/readymade/issues";

page!(Failure {
    buffer: gtk::TextBuffer,
}:
    init(root, sender, model, widgets) {
        root.set_hexpand(false);
    }
    update(self, message, sender) {
        ReportBug => gtk::UriLauncher::new(BUG_REPORT_LINK).launch(
            Option::<&libhelium::Window>::None,
            gtk::gio::Cancellable::NONE,
            |_| {},
        ),
        Err(s: String) => self.buffer.write_str(&strip_ansi_escapes::strip_str(&s)).unwrap(),
    } => {}

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
            set_label: &t!("page-failure-close"),
            add_css_class: "large-button",
            connect_clicked => FailurePageMsg::Navigate(NavigationAction::Quit)
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &t!("page-failure-bug"),
            add_css_class: "large-button",
            connect_clicked => FailurePageMsg::ReportBug,
        }
    }
);
