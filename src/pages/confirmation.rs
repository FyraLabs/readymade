use crate::prelude::*;

page!(Confirmation:
    init(root, sender, model, widgets) {}
    update(self, message, sender) {
        StartInstallation => {
            sender
                .output(Self::Output::StartInstallation)
                .unwrap();

            sender
                .output(Self::Output::Navigate(NavigationAction::GoTo(
                    Page::Installation,
                )))
                .unwrap();
        },
    } => { StartInstallation }

    gtk::CenterBox {
        set_orientation: gtk::Orientation::Horizontal,
        set_valign: gtk::Align::Center,
        set_vexpand: true,

        #[wrap(Some)]
        set_start_widget = &gtk::Box {
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
                set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.disk_name).unwrap_or_default(),
                inline_css: "font-size: 16px; font-weight: bold"
            },

            gtk::Label {
                #[watch]
                set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.os_name).unwrap_or_default(),
            }
        },

        #[wrap(Some)]
        set_center_widget = &gtk::Image {
            set_icon_name: Some("go-next-symbolic"),
            inline_css: "-gtk-icon-size: 64px",
            set_margin_horizontal: 16,
        },

        #[wrap(Some)]
        set_end_widget = &gtk::Box {
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
                set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.disk_name).unwrap_or_default(),
                inline_css: "font-size: 16px; font-weight: bold"
            },

            gtk::Label {
                set_label: &crate::CONFIG.read().distro.name,
            }
        }
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Horizontal,
        set_spacing: 4,

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &gettext("Previous"),
            add_css_class: "large-button",
            connect_clicked => ConfirmationPageMsg::Navigate(NavigationAction::GoTo(
                if crate::CONFIG.read().install.allowed_installtypes.len() == 1 {
                    crate::Page::Destination
                } else {
                    crate::Page::InstallationType
                }
            )),
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &gettext("Install"),
            add_css_class: "large-button",
            add_css_class: "destructive-action",
            connect_clicked => ConfirmationPageMsg::StartInstallation
        },
    }
);
