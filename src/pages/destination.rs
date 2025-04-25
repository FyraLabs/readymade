use crate::prelude::*;
use relm4::factory::DynamicIndex;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::LazyLock};

pub static DISKS_DATA: LazyLock<Vec<DiskInit>> = LazyLock::new(|| {
    let mut v = crate::disks::detect_os();
    v.sort();
    v
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
            add_css_class: "mini-content-block",

            gtk::Image {
                set_icon_name: Some("drive-harddisk"),
                set_pixel_size: 128
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

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }
}

pub struct DestinationPage {
    disks: FactoryVecDeque<DiskInit>,
    scanning: bool,
    nodisk: libhelium::EmptyPage,
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
impl Component for DestinationPage {
    type Init = ();
    type Input = DestinationPageMsg;
    type Output = DestinationPageOutput;
    type CommandOutput = Vec<DiskInit>;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &t!("page-destination"),
                add_css_class: "view-title"
            },
            set_vexpand: true,
            set_hexpand: false,

            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 6,

                if model.scanning {
                    libhelium::EmptyPage {
                        set_icon: "content-loading-symbolic",
                        set_title: &t!("page-destination-scanning"),
                        set_description: &t!("page-destination-wait"),
                    }
                } else if model.disks.is_empty() {
                    #[local_ref] nodisk ->
                    libhelium::EmptyPage {
                        set_icon: "list-remove-all-symbolic",
                        set_title: &t!("page-destination-no-disk"),
                        set_description: &t!("page-destination-no-disk-desc"),
                    }
                } else {
                    gtk::ScrolledWindow {
                        #[local_ref]
                        disk_list -> gtk::FlowBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            set_orientation: gtk::Orientation::Horizontal,
                            set_vexpand: true,
                            set_homogeneous: true,
                            set_valign: gtk::Align::Center,
                            set_min_children_per_line: 1,
                            set_max_children_per_line: 7,
                            set_column_spacing: 6,
                            set_row_spacing: 6,
                            add_css_class: "content-flowbox",
                            connect_selected_children_changed => DestinationPageMsg::SelectionChanged,
                        },
                    }
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,

                    libhelium::Button {
                        set_is_textual: true,
                        #[watch]
                        set_label: &t!("prev"),
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::GoTo(Page::Welcome))
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::Button {
                        set_is_pill: true,
                        #[watch]
                        set_label: &t!("next"),
                        add_css_class: "large-button",
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::GoTo(Page::InstallationType)),
                        #[watch]
                        set_sensitive: INSTALLATION_STATE.read().destination_disk.is_some()
                    }
                },
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let disks = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .detach();

        sender.spawn_command(|sender| sender.send(DISKS_DATA.clone()).expect("cannot send disks"));

        let mut model = Self {
            disks,
            scanning: true,
            nodisk: libhelium::EmptyPage::default(),
        };

        let nodisk = model.nodisk;

        let disk_list: &gtk::FlowBox = model.disks.widget();
        let widgets = view_output!();

        model.nodisk = nodisk;

        // FIXME: lea please make the button more accessible, also why is it shown by default
        let btn = model.nodisk.last_child().unwrap();
        btn.last_child().unwrap().set_visible(false);
        // TODO: we probably should also disable the button for the loading page

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| DestinationPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
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

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        let mut guard = self.disks.guard();
        for disk in message {
            guard.push_front(disk);
        }
        self.scanning = false;
    }
}
