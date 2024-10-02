use std::{path::PathBuf, rc::Rc};

use relm4::factory::FactoryVecDeque;

use crate::{prelude::*, NavigationAction};

pub struct InstallCustomPage {
    choose_mount_factory: Rc<FactoryVecDeque<ChooseMount>>,
}

#[derive(Debug)]
pub enum InstallCustomPageMsg {
    AddRow,
    #[doc(hidden)]
    Navigate(NavigationAction),
}

#[derive(Debug)]
pub enum InstallCustomPageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallCustomPage {
    type Init = ();
    type Input = InstallCustomPageMsg;
    type Output = InstallCustomPageOutput;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                set_label: &gettext("Custom"),
                set_css_classes: &["view-title"],
            },
            set_vexpand: true,
            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                set_vexpand: true,

                gtk::ScrolledWindow {
                    #[local_ref]
                    mounts -> gtk::Box {
                        add_css_class: "content-list",
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,
                    }
                },

                gtk::Box {
                    set_vexpand: false,
                    set_hexpand: true,
                    set_orientation: gtk::Orientation::Horizontal,

                    gtk::Box {
                        set_hexpand: true,
                    },

                    libhelium::OverlayButton {
                        set_typeb: libhelium::OverlayButtonTypeButton::Primary,
                        set_icon: "list-add",
                        connect_clicked => InstallCustomPageMsg::AddRow,
                    },
                },
            },
        }
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let choose_mount_factory = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .detach();

        let model = Self {
            choose_mount_factory: Rc::new(choose_mount_factory),
        };

        let mounts = model.choose_mount_factory.widget();
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallCustomPageMsg::Navigate(action) => sender
                .output(InstallCustomPageOutput::Navigate(action))
                .unwrap(),
            InstallCustomPageMsg::AddRow => todo!(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ChooseMount (row in main page)

#[derive(Debug, Clone, Default)]
struct ChooseMount {
    partition: PathBuf,
    mountpoint: PathBuf,
    options: String,
}

#[derive(Debug)]
enum ChooseMountMsg {
    Remove,
}

#[derive(Debug)]
enum ChooseMountOutput {
    Remove,
}

#[relm4::factory]
impl FactoryComponent for ChooseMount {
    type ParentWidget = gtk::Box;
    type Input = ChooseMountMsg;
    type Output = ChooseMountOutput;
    type CommandOutput = ();
    type Init = (PathBuf, PathBuf, String);

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            gtk::Label {
                set_label: &format!("{} ← {}{}", self.mountpoint.display(), self.partition.display(), if self.options.is_empty() { String::new() } else { format!(" [{}]", self.options) }),
            },

            libhelium::Button {
                set_icon_name: "delete",
                set_tooltip: &gettext("Remove mountpoint"),
                connect_clicked => ChooseMountMsg::Remove,
            },
        }
    }

    fn init_model(
        (partition, mountpoint, options): Self::Init,
        _index: &Self::Index,
        _sender: FactorySender<Self>,
    ) -> Self {
        Self {
            partition,
            mountpoint,
            options,
        }
    }

    fn update(&mut self, message: Self::Input, sender: FactorySender<Self>) {
        match message {
            ChooseMountMsg::Remove => sender.output(ChooseMountOutput::Remove).unwrap(),
        }
    }
}


// ────────────────────────────────────────────────────────────────────────────
// AddDialog (also for edit)

#[derive(Debug)]
struct AddDialog {
    partition: String,
    mountpoint: String,
    mountopts: String,
}

#[derive(Debug)]
enum AddDialogMsg {
    
}

#[relm4::component]
impl SimpleComponent for AddDialog {
    type Init = AddDialog;
    type Input = AddDialogMsg;
    type Output = ();
    
    view! {
        libhelium::Window {
            #[wrap(Some)]
            set_child = &gtk::FlowBox {
                set_orientation: gtk::Orientation::Vertical,
                set_max_children_per_line: 2,
                append = &gtk::Label {
                    set_label: &gettext("Partition"),
                },
                append = &gtk::DropDown {
                    
                },
                append = &gtk::Label {
                    set_label: &gettext("Mount at"),
                },
                append = &libhelium::TextField {
                    
                },
                append = &gtk::Label {
                    set_label: &gettext("Mount options"),
                },
                append = &libhelium::TextField {
                    
                },
            }
        }
    }
    
    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = init;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}