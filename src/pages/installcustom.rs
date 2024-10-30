use std::path::PathBuf;

use relm4::factory::FactoryVecDeque;

use crate::{backend::custom::MountTarget as ChooseMount, prelude::*, NavigationAction};

pub struct InstallCustomPage {
    choose_mount_factory: FactoryVecDeque<ChooseMount>,
}

#[derive(Debug)]
pub enum InstallCustomPageMsg {
    AddRow,
    #[allow(private_interfaces)]
    UpdateRow(AddDialog),
    RowOutput(ChooseMountOutput),
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
                        set_orientation: gtk::Orientation::Vertical,
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
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let choose_mount_factory = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), InstallCustomPageMsg::RowOutput);

        let model = Self {
            choose_mount_factory,
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
            InstallCustomPageMsg::AddRow => {
                let out = sender.input_sender();
                let dialog = AddDialog::builder();
                let mut ctrl = dialog
                    .launch(AddDialog {
                        index: self.choose_mount_factory.len(),
                        ..AddDialog::default()
                    })
                    .forward(out, InstallCustomPageMsg::UpdateRow);
                ctrl.detach_runtime();
                ctrl.widget().present();
            }
            InstallCustomPageMsg::UpdateRow(msg) => {
                let mut guard = self.choose_mount_factory.guard();
                let Some(obj) = guard.get_mut(msg.index) else {
                    // new entry
                    guard.push_back((msg.partition.into(), msg.mountpoint.into(), msg.mountopts));
                    return;
                };
                obj.partition = PathBuf::from(msg.partition);
                obj.mountpoint = PathBuf::from(msg.mountpoint);
                obj.options = msg.mountopts;
            }
            InstallCustomPageMsg::RowOutput(action) => match action {
                ChooseMountOutput::Edit(index) => {
                    let Some(crate::backend::custom::MountTarget {
                        partition,
                        mountpoint,
                        options,
                        ..
                    }) = self.choose_mount_factory.get(index)
                    else {
                        unreachable!()
                    };

                    let dialog = AddDialog::builder();
                    let mut ctrl = dialog
                        .launch(AddDialog {
                            index,
                            partition: partition.display().to_string(),
                            mountpoint: mountpoint.display().to_string(),
                            mountopts: options.to_string(),
                        })
                        .forward(sender.input_sender(), InstallCustomPageMsg::UpdateRow);
                    ctrl.detach_runtime();
                    ctrl.widget().present();
                }
                ChooseMountOutput::Remove(index) => {
                    self.choose_mount_factory
                        .guard()
                        .remove(index)
                        .expect("can't remove requested row");
                }
            },
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ChooseMount (row in main page)

#[derive(Debug)]
pub enum ChooseMountMsg {
    Edit,
    Remove,
}

#[derive(Debug)]
pub enum ChooseMountOutput {
    Edit(usize),
    Remove(usize),
}

#[relm4::factory(pub)]
impl FactoryComponent for ChooseMount {
    type ParentWidget = gtk::Box;
    type Input = ChooseMountMsg;
    type Output = ChooseMountOutput;
    type CommandOutput = ();
    type Init = (PathBuf, PathBuf, String);

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 16,

            gtk::Label {
                set_label: &format!("{} ← {}{}", self.mountpoint.display(), self.partition.display(), if self.options.is_empty() { String::new() } else { format!(" [{}]", self.options) }),
                add_css_class: "monospace",
            },

            libhelium::Button {
                set_icon_name: "document-edit-symbolic",
                set_tooltip: &gettext("Remove mountpoint"),
                add_css_class: "suggested-action",
                connect_clicked => ChooseMountMsg::Edit,
            },

            libhelium::Button {
                set_icon_name: "edit-clear-symbolic",
                set_tooltip: &gettext("Remove mountpoint"),
                add_css_class: "destructive-action",
                connect_clicked => ChooseMountMsg::Remove,
            },
        }
    }

    fn init_model(
        (partition, mountpoint, options): Self::Init,
        index: &Self::Index,
        _sender: FactorySender<Self>,
    ) -> Self {
        Self {
            index: index.current_index(),
            partition,
            mountpoint,
            options,
        }
    }

    fn update(&mut self, message: Self::Input, sender: FactorySender<Self>) {
        match message {
            ChooseMountMsg::Remove => sender
                .output(ChooseMountOutput::Remove(self.index))
                .unwrap(),
            ChooseMountMsg::Edit => sender.output(ChooseMountOutput::Edit(self.index)).unwrap(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// AddDialog (also for edit)

#[derive(Clone, Debug, Default)]
struct AddDialog {
    partition: String,
    mountpoint: String,
    mountopts: String,
    index: usize,
}

#[derive(Debug)]
enum AddDialogMsg {
    ChangedPart(String),
    ChangedMnpt(String),
    ChangedOpts(String),
    Close(libhelium::Window),
}

#[relm4::component]
impl SimpleComponent for AddDialog {
    type Init = Self;
    type Input = AddDialogMsg;
    type Output = Self;

    view! {
        #[name(window)]
        libhelium::Window {
            set_title: Some("Mount Target"),
            set_default_width: 300,
            set_default_height: 250,
            set_vexpand: true,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                set_spacing: 4,
                set_margin_all: 8,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 6,

                    gtk::Label {
                        set_label: &gettext("Partition"),
                    },
                    #[local_ref]
                    partlist -> gtk::DropDown {
                        set_enable_search: true,
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 3,

                    gtk::Label {
                        set_label: &gettext("Mount at"),
                    },
                    #[name = "tf_at"]
                    libhelium::TextField {
                        add_css_class: "monospace",

                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 3,

                    gtk::Label {
                        set_label: &gettext("Mount options"),
                    },
                    #[name = "tf_opts"]
                    libhelium::TextField {
                        add_css_class: "monospace",
                    },
                },

                #[name(btn)]
                libhelium::OverlayButton {
                    set_label: Some("OK"),
                    connect_clicked[sender, window] => move |_| sender.input(AddDialogMsg::Close(window.clone())),
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        tracing::trace!(?init, "Spawned AddDialog");
        // populate partition dropdown list
        let disk = (crate::INSTALLATION_STATE.read().destination_disk.clone()).unwrap();
        let disk = disk.devpath.file_name().unwrap().to_str().unwrap();
        let partlist = lsblk::BlockDevice::list().unwrap();
        let partlist = (partlist.iter())
            .filter(|b| b.is_part() && b.disk_name().is_ok_and(|d| d == disk))
            .map(|p| p.fullname.clone());
        let partvec = partlist.collect_vec();
        let partlist =
            &gtk::DropDown::from_strings(&partvec.iter().filter_map(|s| s.to_str()).collect_vec());

        let (sd0, sd1, sd2) = (sender.clone(), sender.clone(), sender.clone());
        let partvec0 = partvec.clone();
        // connect signal for the dropdown
        partlist.connect_selected_notify(move |dropdown| {
            sd0.input(AddDialogMsg::ChangedPart(
                #[allow(clippy::indexing_slicing)]
                partvec0[dropdown.selected() as usize].display().to_string(),
            ));
        });

        let mut model = init;
        let widgets = view_output!();
        let window = widgets.window.clone();
        widgets.btn.connect_clicked(move |_| window.close());
        // connect signal for textfields
        widgets
            .tf_at
            .internal_entry()
            .connect_changed(move |en| sd1.input(AddDialogMsg::ChangedMnpt(en.text().to_string())));
        widgets
            .tf_opts
            .internal_entry()
            .connect_changed(move |en| sd2.input(AddDialogMsg::ChangedOpts(en.text().to_string())));
        model.partition = partvec[partlist.selected() as usize].display().to_string();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AddDialogMsg::ChangedPart(part) => self.partition = part,
            AddDialogMsg::ChangedMnpt(mnpt) => self.mountpoint = mnpt,
            AddDialogMsg::ChangedOpts(opts) => self.mountopts = opts,
            AddDialogMsg::Close(window) => {
                sender.output(std::mem::take(self)).unwrap();
                window.close();
            }
        }
    }
}
