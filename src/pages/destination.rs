use std::path::PathBuf;

use crate::{NavigationAction, INSTALLATION_STATE};
use gettextrs::gettext;
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent,
};

#[derive(Debug, Clone)]
pub struct DiskInit {
    pub disk_name: String,
    pub os_name: String,
    pub devpath: PathBuf,
}

struct Disk {
    disk_name: String,
    os_name: String,
    devpath: PathBuf,
}

#[relm4::factory]
impl FactoryComponent for Disk {
    type Init = DiskInit;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 2,

            gtk::Image {
                set_icon_name: Some("drive-harddisk"),
                inline_css: "-gtk-icon-size: 128px"
            },

            gtk::Label {
                set_label: &self.disk_name,
                inline_css: "font-size: 16px; font-weight: bold"
            },

            gtk::Label {
                set_label: &self.os_name
            }
        }
    }

    fn init_model(value: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            disk_name: value.disk_name,
            os_name: value.os_name,
            devpath: value.devpath,
        }
    }
}

pub struct DestinationPage {
    disks: FactoryVecDeque<Disk>,
}

#[derive(Debug)]
pub enum DestinationPageMsg {
    Update,
    SelectionChanged,
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum DestinationPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for DestinationPage {
    type Init = ();
    type Input = DestinationPageMsg;
    type Output = DestinationPageOutput;

    view! {
        libhelium::ViewMono {
            set_title: &gettext("Destination"),
            set_vexpand: true,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                #[local_ref]
                disk_list -> gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::Single,
                    set_orientation: gtk::Orientation::Horizontal,
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                    set_min_children_per_line: 7,
                    set_column_spacing: 4,
                    set_row_spacing: 4,
                    connect_selected_children_changed => DestinationPageMsg::SelectionChanged,
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::TextButton {
                        set_label: &gettext("Previous"),
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome))
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: &gettext("Next"),
                        inline_css: "padding-left: 48px; padding-right: 48px",
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::GoTo(crate::Page::InstallationType)),
                        #[watch]
                        set_sensitive: INSTALLATION_STATE.read().destination_disk.is_some()
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut disks = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |_output| todo!());

        let disks_data = crate::disks::detect_os();

        for disk in disks_data {
            disks.guard().push_front(disk);
        }

        let model = Self { disks };

        let disk_list = model.disks.widget();
        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| DestinationPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            DestinationPageMsg::Navigate(action) => sender
                .output(DestinationPageOutput::Navigate(action))
                .unwrap(),
            DestinationPageMsg::SelectionChanged => {
                let disk_list = self.disks.widget();
                let selected_children = disk_list.selected_children();
                let selected_disk = selected_children.first().map(|d| {
                    let disk = self.disks.get(d.index().try_into().unwrap()).unwrap();
                    DiskInit {
                        disk_name: disk.disk_name.clone(),
                        os_name: disk.os_name.clone(),
                        devpath: disk.devpath.clone(),
                    }
                });

                let mut installation_state_guard = INSTALLATION_STATE.write();
                installation_state_guard.destination_disk = selected_disk;
            }
            DestinationPageMsg::Update => {}
        }
    }
}
