use crate::prelude::*;
use crate::{NavigationAction, INSTALLATION_STATE};
use relm4::{
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInit {
    pub disk_name: String,
    pub os_name: String,
    pub devpath: PathBuf,
    pub size: bytesize::ByteSize,
}

#[relm4::factory(pub)]
impl FactoryComponent for DiskInit {
    type Init = Self;
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
            },

            gtk::Label {
                set_label: &self.size.to_string()
            }
        }
    }

    fn init_model(value: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        value
    }
}

pub struct DestinationPage {
    disks: FactoryVecDeque<DiskInit>,
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
            #[watch]
            set_title: &gettext("Destination"),
            set_vexpand: true,
            set_hexpand: false,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                #[local_ref]
                disk_list -> gtk::FlowBox {
                    set_selection_mode: gtk::SelectionMode::Single,
                    set_orientation: gtk::Orientation::Horizontal,
                    set_vexpand: true,
                    set_hexpand: false,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                    set_min_children_per_line: 5,
                    set_column_spacing: 4,
                    set_row_spacing: 4,
                    connect_selected_children_changed => DestinationPageMsg::SelectionChanged,
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &gettext("Previous"),
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Welcome))
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
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

        let disk_list: &gtk::FlowBox = model.disks.widget();
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
                let selected_disk = selected_children
                    .first()
                    .map(|d| self.disks.get(d.index().try_into().unwrap()).unwrap());

                let mut installation_state_guard = INSTALLATION_STATE.write();
                installation_state_guard.destination_disk = selected_disk.cloned();
            }
            DestinationPageMsg::Update => {}
        }
    }
}
