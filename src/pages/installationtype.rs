use crate::prelude::*;
use crate::{InstallationType, NavigationAction, Page, INSTALLATION_STATE};
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Default)]
pub struct InstallationTypePage {
    can_encrypt: bool,
}

#[derive(Debug)]
pub enum InstallationTypePageMsg {
    Update,
    #[doc(hidden)]
    Navigate(NavigationAction),
    InstallationTypeSelected(InstallationType),
    Next,
}

#[derive(Debug)]
pub enum InstallationTypePageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallationTypePage {
    type Init = ();
    type Input = InstallationTypePageMsg;
    type Output = InstallationTypePageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &gettext("Installation Type"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,

            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 6,

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
                            set_label: &gettext("Entire Disk"),
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
                            set_label: &gettext("Dual Boot"),
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
                            set_label: &gettext("Custom"),
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
                            set_label: &gettext("Chromebook"),
                            add_css_class: "large-button",
                            connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::ChromebookInstall)
                        },
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_halign: gtk::Align::Center,

                    gtk::CheckButton {
                        set_label: Some(&gettext("Enable disk encryption")),
                        #[watch]
                        set_sensitive: model.can_encrypt,
                        connect_toggled => |btn| INSTALLATION_STATE.write().encrypt = btn.is_active(),
                    },
                    gtk::CheckButton {
                        set_label: Some(&gettext("Enable TPM")),
                        #[watch]
                        set_sensitive: INSTALLATION_STATE.read().encrypt && model.can_encrypt,
                        connect_toggled => |btn| INSTALLATION_STATE.write().tpm = btn.is_active(),
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,

                    libhelium::Button {
                        set_is_textual: true,
                        #[watch]
                        set_label: &gettext("Previous"),
                        connect_clicked => InstallationTypePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Next"),
                        add_css_class: "large-button",
                        connect_clicked => InstallationTypePageMsg::Next,
                        #[watch]
                        set_sensitive: crate::INSTALLATION_STATE.read().installation_type.is_some(),
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

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationTypePageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::WholeDisk) => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::WholeDisk);
                self.can_encrypt = true;
            }
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::DualBoot(_)) => {
                self.can_encrypt = true;
            }
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::Custom) => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::Custom);
                self.can_encrypt = false;
            }
            InstallationTypePageMsg::InstallationTypeSelected(
                InstallationType::ChromebookInstall,
            ) => {
                INSTALLATION_STATE.write().installation_type =
                    Some(InstallationType::ChromebookInstall);
                self.can_encrypt = true;
            }
            InstallationTypePageMsg::Navigate(action) => sender
                .output(InstallationTypePageOutput::Navigate(action))
                .unwrap(),
            InstallationTypePageMsg::Next => {
                sender.input(InstallationTypePageMsg::Navigate(NavigationAction::GoTo({
                    let value = INSTALLATION_STATE.read().installation_type;
                    match value.unwrap() {
                        InstallationType::DualBoot(_) => Page::InstallDual,
                        InstallationType::ChromebookInstall | InstallationType::WholeDisk => {
                            Page::Confirmation
                        }
                        InstallationType::Custom => Page::InstallCustom,
                    }
                })));
            }
            InstallationTypePageMsg::Update => {}
        }
    }
}

/*
macro_rules! pagename {
    () => {
        "Installation Type"
    };
}

page!(InstallationType {
    can_encrypt: bool,
}:
    init(root, sender, model, widgets) { }

    update(self, message, sender) {
        InstallationTypeSelected(inner: InstallationType) => match inner {
            InstallationType::WholeDisk => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::WholeDisk);
                self.can_encrypt = true;
                INSTALLATION_STATE.write().encrypt = true;
            },
            InstallationType::DualBoot(_) => {
                self.can_encrypt = true;
                INSTALLATION_STATE.write().encrypt = true;
            },
            InstallationType::Custom => {
                INSTALLATION_STATE.write().installation_type = Some(InstallationType::Custom);
                self.can_encrypt = false;
                INSTALLATION_STATE.write().encrypt = false;
            },
            InstallationType::ChromebookInstall => {
                INSTALLATION_STATE.write().installation_type =
                    Some(InstallationType::ChromebookInstall);
                INSTALLATION_STATE.write().encrypt = true;
                self.can_encrypt = true;
            }
        },
        Next => {
            sender.input(InstallationTypePageMsg::Navigate(NavigationAction::GoTo({
                let value = INSTALLATION_STATE.read().installation_type;
                match value.unwrap() {
                    InstallationType::DualBoot(_) => Page::InstallDual,
                    InstallationType::ChromebookInstall | InstallationType::WholeDisk => {
                        Page::Confirmation
                    }
                    InstallationType::Custom => Page::InstallCustom,
                }
            })));
        },
    } => {}

    set_spacing: 6,

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
                set_label: &gettext("Entire Disk"),
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
                set_label: &gettext("Dual Boot"),
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
                set_label: &gettext("Custom"),
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
                set_label: &gettext("Chromebook"),
                add_css_class: "large-button",
                connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::ChromebookInstall)
            },
        },
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Vertical,
        set_halign: gtk::Align::Center,

        gtk::CheckButton {
            set_label: Some(&gettext("Enable disk encryption")),
            #[watch]
            set_sensitive: model.can_encrypt,
            connect_toggled => |btn| INSTALLATION_STATE.write().encrypt = btn.is_active(),
        },
        gtk::CheckButton {
            set_label: Some(&gettext("Enable TPM")),
            #[watch]
            set_sensitive: INSTALLATION_STATE.read().encrypt && model.can_encrypt,
            connect_toggled => |btn| INSTALLATION_STATE.write().tpm = btn.is_active(),
        },
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Horizontal,
        set_spacing: 6,

        libhelium::Button {
            set_is_textual: true,
            #[watch]
            set_label: &gettext("Previous"),
            connect_clicked => InstallationTypePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &gettext("Next"),
            add_css_class: "large-button",
            connect_clicked => InstallationTypePageMsg::Next,
            #[watch]
            set_sensitive: crate::INSTALLATION_STATE.read().installation_type.is_some(),
        }
    }
);
*/
