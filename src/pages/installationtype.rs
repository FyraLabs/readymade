use crate::prelude::*;
use crate::{InstallationType, NavigationAction, Page, INSTALLATION_STATE};
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

#[derive(Default)]
pub struct InstallationTypePage {
    can_encrypt: bool,
    root: Option<libhelium::ViewMono>,
    act: Option<NavigationAction>,
}

#[derive(Debug)]
pub enum InstallationTypePageMsg {
    Update,
    #[doc(hidden)]
    Navigate(NavigationAction),
    InstallationTypeSelected(InstallationType),
    EncryptDialogue(bool),
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
        let mut model = Self::default();

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationTypePageMsg::Update);

        model.root = Some(root.clone());

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
            InstallationTypePageMsg::EncryptDialogue(b) => {
                INSTALLATION_STATE.write().encrypt = b;
                sender
                    .output(InstallationTypePageOutput::Navigate(
                        self.act.take().unwrap(),
                    ))
                    .unwrap();
            }
            InstallationTypePageMsg::Navigate(action) => {
                if INSTALLATION_STATE.read().encrypt {
                    let dialogue = EncryptPassDialogue::builder()
                        .launch(self.root.as_ref().unwrap().toplevel_window().unwrap())
                        .forward(
                            sender.input_sender(),
                            InstallationTypePageMsg::EncryptDialogue,
                        );
                    dialogue.widget().present();
                    self.act = Some(action);
                    return;
                }
                sender
                    .output(InstallationTypePageOutput::Navigate(action))
                    .unwrap();
            }
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

kurage::generate_component!(EncryptPassDialogue {
    btn_confirm: libhelium::Button,
    tf_repeat: gtk::PasswordEntry,
}:
    init[tf_repeat](root, sender, model, widgets) for root_window: gtk::Window {
        libhelium::prelude::WindowExt::set_parent(&root, Some(&root_window));
        model.btn_confirm = widgets.btn_confirm.clone();
    }

    update(self, message, sender) {
        SetBtnSensitive(sensitive: bool) => {
            self.btn_confirm.set_sensitive(sensitive);
        },
        Enter => {
            if self.btn_confirm.is_sensitive() {
                sender.output(true).unwrap();
            }
        },
    } => bool

    libhelium::Dialog {
        set_modal: true,
        set_title: Some(&gettext("Disk Encryption")),

        #[wrap(Some)]
        set_child = &gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            
            
            #[name = "tf_passwd"]
            gtk::PasswordEntry {
                set_hexpand: true,
                set_halign: gtk::Align::Fill,
                set_show_peek_icon: true,
                set_placeholder_text: Some(&gettext("Password")),
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
                set_placeholder_text: Some(&gettext("Repeat Password")),
                connect_changed[sender] => move |en| {
                    let pass = en.text().to_string();
                    sender.input(Self::Input::SetBtnSensitive(INSTALLATION_STATE.read().encryption_key.as_ref().is_some_and(|p| p == &pass && !pass.is_empty())));
                },
                connect_activate => Self::Input::Enter,
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                #[name(btn_confirm)]
                libhelium::Button {
                    set_label: &gettext("Confirm"),
                    set_sensitive: false,
                    connect_activate => Self::Input::Enter,
                },

                libhelium::Button {
                    set_label: &gettext("Cancel"),
                    connect_activate[sender] => move |_| sender.output(false).unwrap(),
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
