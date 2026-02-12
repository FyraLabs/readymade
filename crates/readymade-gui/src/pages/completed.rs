#![allow(dead_code)] // variant Navigate never constructed in Input
use crate::prelude::*;

page!(Completed:
    init(root, sender, model, widgets) {}
    update(self, message, sender) {
        Reboot => {
            // supposedly it should run pkexec automatically?
            _ = std::process::Command::new("systemctl")
                .arg("reboot")
                .status();
        },
        Close => sender
            .output(CompletedPageOutput::Navigate(NavigationAction::Quit))
            .unwrap(),
        // Update => {},
    } => {}

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
            set_label: &t!("page-completed-desc"),
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
            set_label: &t!("page-completed-close"),
            add_css_class: "large-button",
            connect_clicked => CompletedPageMsg::Close,
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &t!("page-completed-reboot"),
            add_css_class: "large-button",
            connect_clicked => CompletedPageMsg::Reboot,
        }
    }
);
