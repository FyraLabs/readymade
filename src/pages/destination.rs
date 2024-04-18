use crate::NavigationAction;
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{
    factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque},
    ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, WidgetTemplate,
};

pub struct DiskInit {
    pub disk_name: String,
    pub os_name: String,
}

struct Disk {
    disk_name: String,
    os_name: String,
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
        }
    }
}

pub struct DestinationPage {
    disks: FactoryVecDeque<Disk>,
}

#[derive(Debug)]
pub enum DestinationPageMsg {
    #[doc(hidden)]
    Navigate(NavigationAction),
    SelectionChanged,
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
            set_title: "Destination",
            set_vexpand: true,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                #[local_ref]
                disk_list -> gtk::FlowBox {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_vexpand: true,
                    set_hexpand: true,
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                    set_min_children_per_line: 7,
                    connect_selected_children_changed => DestinationPageMsg::SelectionChanged
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::TextButton {
                        set_label: "Previous",
                        connect_clicked => DestinationPageMsg::Navigate(NavigationAction::Back)
                    },

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::PillButton {
                        set_label: "Next",
                        inline_css: "padding-left: 48px; padding-right: 48px",
                    }
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut disks = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.input_sender(), |output| match output {
                _ => todo!(),
            });

        let disks_data = crate::disks::detect_os();

        for disk in disks_data {
            disks.guard().push_front(disk);
        }

        /* disks.guard().push_front(DiskInit {
            disk_name: "fuck".to_string(),
            os_name: "owo".to_string(),
        });

        disks.guard().push_front(DiskInit {
            disk_name: "fuck".to_string(),
            os_name: "owo".to_string(),
        });

        disks.guard().push_front(DiskInit {
            disk_name: "fuck".to_string(),
            os_name: "owo".to_string(),
        }); */

        let model = Self { disks };

        let disk_list = model.disks.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            DestinationPageMsg::Navigate(action) => sender
                .output(DestinationPageOutput::Navigate(action))
                .unwrap(),
            DestinationPageMsg::SelectionChanged => {}
        }
    }
}
