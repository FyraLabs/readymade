use crate::prelude::*;
use crate::{NavigationAction, Page, INSTALLATION_STATE};
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

pub struct ConfirmationPage;

#[derive(Debug)]
pub enum ConfirmationPageMsg {
    StartInstallation,
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
}

#[derive(Debug)]
pub enum ConfirmationPageOutput {
    StartInstallation,
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for ConfirmationPage {
    type Init = ();
    type Input = ConfirmationPageMsg;
    type Output = ConfirmationPageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &gettext("Confirmation"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,
            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_valign: gtk::Align::Center,
                    set_spacing: 16,

                    gtk::Box {
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

                    gtk::Image {
                        set_icon_name: Some("go-next-symbolic"),
                        inline_css: "-gtk-icon-size: 64px"
                    },


                    gtk::Box {
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
                        inline_css: "min-width: 96px",
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
                        inline_css: "min-width: 96px",
                        add_css_class: "destructive-action",
                        connect_clicked => ConfirmationPageMsg::StartInstallation
                    },
                }
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| ConfirmationPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ConfirmationPageMsg::StartInstallation => {
                sender
                    .output(ConfirmationPageOutput::StartInstallation)
                    .unwrap();

                sender
                    .output(ConfirmationPageOutput::Navigate(NavigationAction::GoTo(
                        Page::Installation,
                    )))
                    .unwrap();
            }
            ConfirmationPageMsg::Navigate(action) => sender
                .output(ConfirmationPageOutput::Navigate(action))
                .unwrap(),
            ConfirmationPageMsg::Update => {}
        }
    }
}
