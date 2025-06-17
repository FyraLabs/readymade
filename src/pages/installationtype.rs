use std::sync::LazyLock;

use crate::prelude::*;

use crate::{InstallationType, NavigationAction, Page, INSTALLATION_STATE};
use relm4::RelmWidgetExt;

static TPM_SUPPORT: LazyLock<bool> = LazyLock::new(|| {
    std::fs::read_to_string("/sys/class/tpm/tpm0/tpm_version_major")
        .ok()
        .and_then(|s: String| s.trim().parse::<usize>().ok())
        .is_some_and(|ver| ver >= 2)
});

page!(InstallationType {
    can_encrypt: bool,
    root: Option<libhelium::ViewMono>,
    act: Option<NavigationAction>,
    encrypt_btn: gtk::CheckButton,
}:
    init[encrypt_btn](root, sender, model, widgets) {
        model.root = Some(root);
    }
    update(self, message, sender) {
        InstallationTypeSelected(i: InstallationType) => match i {
            InstallationType::WholeDisk => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::WholeDisk);
                self.can_encrypt = true;
            },
            InstallationType::DualBoot(_) => {
                self.can_encrypt = true;
            },
            InstallationType::Custom => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::Custom);
                self.can_encrypt = false;
            },
            InstallationType::ChromebookInstall => {
                INSTALLATION_STATE.write().installation_type =
                    Some(InstallationType::ChromebookInstall);
                self.can_encrypt = true;
            },
        },
        EncryptDialogue(b: bool) => {
            INSTALLATION_STATE.write().encrypt = b;
            self.encrypt_btn.set_active(b);
            sender
                .output(InstallationTypePageOutput::Navigate(
                    self.act.take().unwrap(),
                ))
                .unwrap();
        },
        Next => {
            self.act = Some(NavigationAction::GoTo({
                let value = INSTALLATION_STATE.read().installation_type;
                match value.unwrap() {
                    InstallationType::DualBoot(_) => Page::InstallDual,
                    InstallationType::ChromebookInstall | InstallationType::WholeDisk => {
                        Page::Confirmation
                    }
                    InstallationType::Custom => Page::InstallCustom,
                }
            }));
            if INSTALLATION_STATE.read().encrypt {
                let mut dialogue = EncryptPassDialogue::builder()
                        .launch(self.root.as_ref().unwrap().toplevel_window().unwrap())
                        .forward(
                            sender.input_sender(),
                            InstallationTypePageMsg::EncryptDialogue,
                        );
                    dialogue.widget().present();
                    dialogue.detach_runtime();
                    return;
            }
            sender.input(InstallationTypePageMsg::Navigate(self.act.clone().unwrap()));
        },
    } => {}

    gtk::Box {
        set_orientation: gtk::Orientation::Vertical,
        set_valign: gtk::Align::Center,
        set_spacing: 18,

        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 6,
            set_vexpand: true,
            set_hexpand: true,
            set_valign: gtk::Align::Center,
            set_halign: gtk::Align::Center,

            gtk::Image {
                set_icon_name: Some("drive-harddisk-symbolic"),
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

        gtk::Box {
            set_spacing: 6,
            set_halign: gtk::Align::Center,
            set_valign: gtk::Align::End,
            set_homogeneous: true,
            libhelium::Button {
                set_visible: crate::CONFIG.read().install.allowed_installtypes.contains(&InstallationType::WholeDisk),
                #[watch]
                set_is_fill: crate::INSTALLATION_STATE.read().installation_type == Some(InstallationType::WholeDisk),
                #[watch]
                set_is_outline: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::WholeDisk),
                #[watch]
                set_is_tint: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::WholeDisk),
                #[watch]
                set_label: &t!("page-installationtype-entire"),
                add_css_class: "large-button",
                connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::WholeDisk)
            },
            libhelium::Button {
                set_visible: crate::CONFIG.read().install.allowed_installtypes.iter().any(|x| matches!(x, InstallationType::DualBoot(_))),
                #[watch]
                set_is_fill: matches!(crate::INSTALLATION_STATE.read().installation_type, Some(InstallationType::DualBoot(_))),
                #[watch]
                set_is_outline: !matches!(crate::INSTALLATION_STATE.read().installation_type, Some(InstallationType::DualBoot(_))),
                #[watch]
                set_is_tint: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::WholeDisk),
                #[watch]
                set_label: &t!("page-installationtype-dual"),
                add_css_class: "large-button",
                connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::DualBoot(0)),
            },
            libhelium::Button {
                set_visible: crate::CONFIG.read().install.allowed_installtypes.contains(&InstallationType::Custom),
                #[watch]
                set_is_fill: crate::INSTALLATION_STATE.read().installation_type == Some(InstallationType::Custom),
                #[watch]
                set_is_outline: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::Custom),
                #[watch]
                set_is_tint: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::WholeDisk),
                #[watch]
                set_label: &t!("page-installationtype-custom"),
                add_css_class: "large-button",
                connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::Custom)
            },
            libhelium::Button {
                set_visible: crate::CONFIG.read().install.allowed_installtypes.contains(&InstallationType::ChromebookInstall),
                #[watch]
                set_is_fill: crate::INSTALLATION_STATE.read().installation_type == Some(InstallationType::ChromebookInstall),
                #[watch]
                set_is_outline: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::ChromebookInstall),
                #[watch]
                set_is_tint: crate::INSTALLATION_STATE.read().installation_type != Some(InstallationType::WholeDisk),
                #[watch]
                set_label: &t!("page-installationtype-chromebook"),
                add_css_class: "large-button",
                connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::ChromebookInstall)
            },
        },
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Vertical,
        set_halign: gtk::Align::Center,

        #[local_ref] encrypt_btn ->
        gtk::CheckButton {
            set_label: Some(&t!("page-installationtype-encrypt")),
            #[watch]
            set_sensitive: model.can_encrypt,
            connect_toggled => |btn| INSTALLATION_STATE.write().encrypt = btn.is_active(),
        },
        gtk::CheckButton {
            set_label: Some(&t!("page-installationtype-tpm")),
            #[watch]
            set_sensitive: INSTALLATION_STATE.read().encrypt && model.can_encrypt && *TPM_SUPPORT,
            connect_toggled => |btn| INSTALLATION_STATE.write().tpm = btn.is_active(),
        },
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Horizontal,
        set_spacing: 6,

        libhelium::Button {
            set_is_textual: true,
            #[watch]
            set_label: &t!("prev"),
            connect_clicked => InstallationTypePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &t!("next"),
            add_css_class: "large-button",
            connect_clicked => InstallationTypePageMsg::Next,
            #[watch]
            set_sensitive: crate::INSTALLATION_STATE.read().installation_type.is_some(),
        }
    }
);

kurage::generate_component!(EncryptPassDialogue {
    btn_confirm: libhelium::Button,
    tf_repeat: gtk::PasswordEntry,
    root: libhelium::Dialog,
}:
    init[tf_repeat](root, sender, model, widgets) for root_window: gtk::Window {
        libhelium::prelude::WindowExt::set_parent(&root, Some(&root_window));
        model.btn_confirm = widgets.btn_confirm.clone();
        model.root = root;
    }

    update(self, message, sender) {
        SetBtnSensitive(sensitive: bool) => {
            self.btn_confirm.set_sensitive(sensitive);
        },
        Enter => {
            if self.btn_confirm.is_sensitive() {
                sender.output(true).unwrap();
                self.root.set_visible(false);
                self.root.destroy();
            }
        },
    } => bool

    libhelium::Dialog {
        set_modal: true,
        set_title: Some(&t!("dialog-installtype-encrypt")),

        #[wrap(Some)]
        set_child = &gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
            set_hexpand: true,
            set_margin_horizontal: 16,
            set_margin_vertical: 16,
            set_spacing: 16,

            gtk::Label {
                set_label: &t!("dialog-installtype-encrypt-desc"),
            },

            #[name = "tf_passwd"]
            gtk::PasswordEntry {
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
                set_show_peek_icon: true,
                set_placeholder_text: Some(&t!("dialog-installtype-password")),
                connect_changed[sender, tf_repeat] => move |en| {
                    sender.input(Self::Input::SetBtnSensitive(en.text() == tf_repeat.text() && !en.text().is_empty()));
                    INSTALLATION_STATE.write().encryption_key = Some(en.text().to_string());
                },
            },

            #[local_ref] tf_repeat ->
            gtk::PasswordEntry {
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
                set_show_peek_icon: true,
                set_placeholder_text: Some(&t!("dialog-installtype-repeat")),
                connect_changed[sender] => move |en| {
                    let pass = en.text().to_string();
                    sender.input(Self::Input::SetBtnSensitive(INSTALLATION_STATE.read().encryption_key.as_ref().is_some_and(|p| p == &pass && !pass.is_empty())));
                },
                connect_activate => Self::Input::Enter,
            },

            gtk::Box {
                set_vexpand: true,
            },

            gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Horizontal,
                set_valign: gtk::Align::End,

                libhelium::Button {
                    set_label: &t!("dialog-installtype-cancel"),
                    connect_clicked[sender, root] => move |_| {
                        root.set_visible(false);
                        root.destroy();
                        sender.output(false).unwrap();
                    }
                },

                gtk::Box {
                    set_vexpand: true,
                },

                #[name(btn_confirm)]
                libhelium::Button {
                    set_label: &t!("dialog-installtype-confirm"),
                    set_sensitive: false,
                    connect_clicked => Self::Input::Enter,
                },
            },
        },

        // FIXME: for some reason the libhelium crate does not contain these methods
        // (actually DialogExt is just totally missing)

        // #[name(btn_confirm)]
        // #[wrap(Some)]
        // set_primary_button = &libhelium::Button {
        //     set_label: &gettext("Confirm"),
        //     set_sensitive: false,
        //     connect_activate => Self::Input::Enter,
        // },

        // #[wrap(Some)]
        // set_secondary_button = &libhelium::Button {
        //     set_label: &gettext("Cancel"),
        //     connect_activate[sender] => move |_| sender.output(false).unwrap(),
        // },
    },
);
