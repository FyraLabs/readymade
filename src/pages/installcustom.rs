use std::path::PathBuf;

use relm4::factory::FactoryVecDeque;

use crate::{prelude::*, NavigationAction};

pub struct InstallCustomPage {
    choose_mount_factory: FactoryVecDeque<ChooseMount>,
}

#[derive(Debug)]
pub enum InstallCustomPageMsg {
    AddRow,
    UpdateRow(AddDialog),
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
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let choose_mount_factory = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .detach();

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
                // FIXME: the dialog just doesn't appear at all…?
                let out = sender.input_sender();
                let dialog = AddDialog::builder();
                dialog
                    .launch(AddDialog {
                        index: self.choose_mount_factory.len(),
                        ..AddDialog::default()
                    })
                    .forward(out, InstallCustomPageMsg::UpdateRow)
                    .detach_runtime();
            }
            InstallCustomPageMsg::UpdateRow(msg) => {
                let mut guard = self.choose_mount_factory.guard();
                let obj = guard.get_mut(msg.index).unwrap();
                obj.partition = PathBuf::from(msg.partition);
                obj.mountpoint = PathBuf::from(msg.mountpoint);
                obj.options = msg.mountopts;
            }
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
                add_css_class: "monospace",
            },

            // TODO: edit btn

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
}

#[relm4::component]
impl SimpleComponent for AddDialog {
    type Init = Self;
    type Input = AddDialogMsg;
    type Output = Self;

    view! {
        libhelium::Window {
            #[wrap(Some)]
            set_child = &gtk::FlowBox {
                set_orientation: gtk::Orientation::Vertical,
                set_max_children_per_line: 2,
                append = &gtk::Label {
                    set_label: &gettext("Partition"),
                },
                #[local_ref]
                append = partlist -> gtk::DropDown {
                    set_enable_search: true,
                },
                append = &gtk::Label {
                    set_label: &gettext("Mount at"),
                },
                #[name = "tf_at"]
                append = &libhelium::TextField {
                    add_css_class: "monospace",

                },
                append = &gtk::Label {
                    set_label: &gettext("Mount options"),
                },
                #[name = "tf_opts"]
                append = &libhelium::TextField {
                    add_css_class: "monospace",
                },
            },
            connect_close_request[sender, model] => move |_| {
                sender.output(model.clone()).unwrap();
                tracing::trace!(?model, "connect_close_request()");
                gtk::glib::Propagation::Proceed
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
        let disk = crate::INSTALLATION_STATE.read().destination_disk.clone();
        let disk = disk.unwrap().devpath.to_string_lossy().to_string();
        let partlist = lsblk::BlockDevice::list().unwrap();
        let partlist = (partlist.iter())
            .filter(|b| b.is_part() && b.disk_name().is_ok_and(|d| d == disk))
            .map(|p| p.fullname.clone());
        let partvec = partlist.collect_vec();
        let partlist =
            &gtk::DropDown::from_strings(&partvec.iter().filter_map(|s| s.to_str()).collect_vec());

        let (sd0, sd1, sd2) = (sender.clone(), sender.clone(), sender.clone());
        // connect signal for the dropdown
        partlist.connect_selected_notify(move |dropdown| {
            sd0.input(AddDialogMsg::ChangedPart(
                #[allow(clippy::indexing_slicing)]
                partvec[dropdown.selected() as usize].display().to_string(),
            ));
        });

        let model = init;
        let widgets = view_output!();
        // connect signal for textfields
        widgets
            .tf_at
            .internal_entry()
            .connect_changed(move |en| sd1.input(AddDialogMsg::ChangedMnpt(en.text().to_string())));
        widgets
            .tf_opts
            .internal_entry()
            .connect_changed(move |en| sd2.input(AddDialogMsg::ChangedMnpt(en.text().to_string())));
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AddDialogMsg::ChangedPart(part) => self.partition = part,
            AddDialogMsg::ChangedMnpt(mnpt) => self.mountpoint = mnpt,
            AddDialogMsg::ChangedOpts(opts) => self.mountopts = opts,
        }
    }
}
