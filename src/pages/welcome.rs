use crate::prelude::*;

page!(Welcome:
    init(root, sender, model, widgets) {
        root.set_title(None::<&gtk::Window>); // unset the title from page!()
    }

    update(self, message, sender) {} => {}


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
            set_label: &gettext("Welcome to %s").replace("%s", &crate::CONFIG.read().distro.name),
            inline_css: "font-weight: bold; font-size: 1.75rem",
        },

        gtk::Label {
            #[watch]
            set_label: &gettext(r#"Either test %s from this installer or start the installation now. You can always return to this screen by selecting "Installer" in the menu."#).replace("%s", &crate::CONFIG.read().distro.name),
            set_justify: gtk::Justification::Center,
            set_max_width_chars: 60,
            set_wrap: true
        },
    },

    gtk::Box {
        set_spacing: 4,
        // set_halign: gtk::Align::Center,

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &gettext("Try"),
            add_css_class: "large-button",
            connect_clicked => WelcomePageMsg::Navigate(NavigationAction::Quit)
        },

            gtk::Box {
                set_hexpand: true,
            },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &gettext("Install"),
            add_css_class: "suggested-action",
            add_css_class: "large-button",
            connect_clicked => WelcomePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
        }
    }
);
