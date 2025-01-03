use std::path::PathBuf;

use relm4::factory::FactoryVecDeque;

use crate::{
    backend::custom::MountTarget as ChooseMount, prelude::*, NavigationAction, INSTALLATION_STATE,
};

pub struct InstallCustomPage {
    pub choose_mount_factory: FactoryVecDeque<ChooseMount>,
}

#[derive(Debug)]
pub enum InstallCustomPageMsg {
    AddRow,
    #[allow(private_interfaces)]
    UpdateRow(AddDialog),
    RowOutput(ChooseMountOutput),
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
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
                #[watch]
                set_label: &gettext("Custom Installation"),
                set_css_classes: &["view-title"],
            },
            set_vexpand: true,
            append = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                set_vexpand: true,
                set_hexpand: true,

                gtk::ScrolledWindow {
                    #[local_ref]
                    mounts -> gtk::Box {
                        set_margin_horizontal: 16,
                        set_spacing: 8,
                        set_orientation: gtk::Orientation::Vertical,
                        add_css_class: "content-list",
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Fill,
                    }
                },

                // FIXME: help me position this button!!!!

                libhelium::OverlayButton {
                    set_valign: gtk::Align::End,
                    set_halign: gtk::Align::End,

                    set_typeb: libhelium::OverlayButtonTypeButton::Primary,
                    set_icon: "go-next",
                    connect_clicked => InstallCustomPageMsg::Navigate(NavigationAction::GoTo(crate::Page::Confirmation)),
                },

                libhelium::BottomBar {
                    #[watch]
                    set_title: &gettext("Partitions and Mountpoints"),

                    #[watch]
                    set_description: &gettext("%s definition(s)").replace("%s", &model.choose_mount_factory.len().to_string()),

                    prepend_button[libhelium::BottomBarPosition::Left] = &libhelium::Button {
                        set_is_iconic: true,
                        #[watch]
                        set_tooltip: &gettext("Add a new definition/row"),

                        set_icon: Some("list-add"),
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

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallCustomPageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        tracing::trace!(?message, "InstallCustomPage::update");
        match message {
            InstallCustomPageMsg::Navigate(action) => sender
                .output(InstallCustomPageOutput::Navigate(action))
                .unwrap(),
            InstallCustomPageMsg::AddRow => {
                AddDialog {
                    index: self.choose_mount_factory.len(),
                    ..AddDialog::default()
                }
                .make_window(sender.input_sender(), InstallCustomPageMsg::UpdateRow);
            }
            InstallCustomPageMsg::UpdateRow(msg) => {
                let mut guard = self.choose_mount_factory.guard();
                let (i, obj) = (msg.index, msg.into());
                if guard.len() > i {
                    guard.insert(i, obj);
                    guard.remove(i.wrapping_add(1));
                } else {
                    // new entry
                    guard.push_back(obj);
                }
            }
            InstallCustomPageMsg::RowOutput(action) => match action {
                ChooseMountOutput::Edit(index) => {
                    let mnt_target = (self.choose_mount_factory.get(index))
                        .expect("request to edit nonexistent row");

                    tracing::trace!(?mnt_target, "Edit MountTarget");

                    AddDialog {
                        index,
                        ..mnt_target.into()
                    }
                    .make_window(sender.input_sender(), InstallCustomPageMsg::UpdateRow);
                }
                ChooseMountOutput::Remove(index) => {
                    (self.choose_mount_factory.guard().remove(index))
                        .expect("can't remove requested row");
                    self.choose_mount_factory
                        .broadcast(ChooseMountMsg::Removed(index));
                }
            },
            InstallCustomPageMsg::Update => {}
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// ChooseMount (row in main page)

#[derive(Debug, Clone)]
pub enum ChooseMountMsg {
    Edit,
    Remove,
    Removed(usize),
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
    type Init = Self;

    view! {
        gtk::Box {
            set_halign: gtk::Align::Fill,
            set_hexpand: true,
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 16,

            gtk::Label {
                set_label: &format!("{} ← {}{}", self.mountpoint.display(), self.partition.display(), if self.options.is_empty() { String::new() } else { format!(" [{}]", self.options) }),
                add_css_class: "monospace",
            },

            gtk::Box {
                set_halign: gtk::Align::Fill,
                set_hexpand: true,
            },

            libhelium::Button {
                set_icon_name: "document-edit-symbolic",
                #[watch]
                set_tooltip: &gettext("Remove mountpoint"),
                add_css_class: "suggested-action",
                connect_clicked => ChooseMountMsg::Edit,
            },

            libhelium::Button {
                set_icon_name: "edit-clear-symbolic",
                #[watch]
                set_tooltip: &gettext("Remove mountpoint"),
                add_css_class: "destructive-action",
                connect_clicked => ChooseMountMsg::Remove,
            },
        }
    }

    fn init_model(mut init: Self::Init, index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        init.index = index.current_index();
        init
    }

    fn update(&mut self, message: Self::Input, sender: FactorySender<Self>) {
        match message {
            ChooseMountMsg::Remove => sender
                .output(ChooseMountOutput::Remove(self.index))
                .unwrap(),
            ChooseMountMsg::Edit => sender.output(ChooseMountOutput::Edit(self.index)).unwrap(),
            ChooseMountMsg::Removed(i) => {
                if self.index > i {
                    self.index -= 1;
                }
            }
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
            set_default_width: 400,
            set_default_height: 325,
            set_vexpand: true,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_vexpand: true,
                set_spacing: 4,
                set_margin_all: 8,
                set_margin_top: 0,

                // NOTE: so that the window can actually be closed
                libhelium::AppBar {},

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 6,

                    gtk::Label {
                        #[watch]
                        set_label: &gettext("Partition"),
                    },
                    #[local_ref]
                    partlist -> gtk::DropDown {
                        set_halign: gtk::Align::End,
                        set_enable_search: true,
                        add_css_class: "monospace",
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 3,

                    gtk::Label {
                        #[watch]
                        set_label: &gettext("Mount at"),
                    },
                    #[name = "tf_at"]
                    libhelium::TextField {
                        set_halign: gtk::Align::Fill,
                        set_hexpand: true,
                        set_is_outline: true,
                        add_css_class: "monospace",

                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_hexpand: true,
                    set_spacing: 3,

                    gtk::Label {
                        #[watch]
                        set_label: &gettext("Mount options"),
                    },
                    #[name = "tf_opts"]
                    libhelium::TextField {
                        set_halign: gtk::Align::Fill,
                        set_hexpand: true,
                        set_is_outline: true,
                        add_css_class: "monospace",
                    },
                },

                libhelium::OverlayButton {
                    // set_halign: gtk::Align::End,
                    // set_valign: gtk::Align::End,
                    set_icon: "list-add",
                    // set_is_iconic: true,
                    connect_clicked[sender, window] => move |_| sender.input(AddDialogMsg::Close(window.clone())),

                    #[watch]
                    set_sensitive: !model.partition.is_empty() && !model.mountpoint.is_empty(),
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
        // connect signal for textfields
        widgets
            .tf_at
            .internal_entry()
            .connect_changed(move |en| sd1.input(AddDialogMsg::ChangedMnpt(en.text().to_string())));
        widgets
            .tf_opts
            .internal_entry()
            .connect_changed(move |en| sd2.input(AddDialogMsg::ChangedOpts(en.text().to_string())));
        if !partvec.is_empty() {
            model.partition = partvec[partlist.selected() as usize].display().to_string();
        }

        // override settings from init
        if let Some(index) = partvec
            .iter()
            .position(|part| part.display().to_string() == model.partition)
        {
            partlist.set_selected(index as u32);
        }
        widgets.tf_at.internal_entry().set_text(&model.mountpoint);
        widgets.tf_opts.internal_entry().set_text(&model.mountopts);

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

impl From<&AddDialog> for ChooseMount {
    fn from(
        AddDialog {
            partition,
            mountpoint,
            mountopts,
            index,
        }: &AddDialog,
    ) -> Self {
        Self {
            index: *index,
            partition: PathBuf::from(partition),
            mountpoint: PathBuf::from(mountpoint),
            options: mountopts.clone(),
        }
    }
}

impl From<&ChooseMount> for AddDialog {
    fn from(
        ChooseMount {
            index,
            partition,
            mountpoint,
            options,
        }: &ChooseMount,
    ) -> Self {
        Self {
            index: *index,
            partition: partition.display().to_string(),
            mountpoint: mountpoint.display().to_string(),
            mountopts: options.clone(),
        }
    }
}

impl From<AddDialog> for ChooseMount {
    fn from(
        AddDialog {
            partition,
            mountpoint,
            mountopts,
            index,
        }: AddDialog,
    ) -> Self {
        Self {
            index,
            partition: PathBuf::from(partition),
            mountpoint: PathBuf::from(mountpoint),
            options: mountopts,
        }
    }
}

impl From<ChooseMount> for AddDialog {
    fn from(
        ChooseMount {
            index,
            partition,
            mountpoint,
            options,
        }: ChooseMount,
    ) -> Self {
        Self {
            index,
            partition: partition.display().to_string(),
            mountpoint: mountpoint.display().to_string(),
            mountopts: options,
        }
    }
}

impl AddDialog {
    fn make_window<X, F>(self, sender: &relm4::Sender<X>, transform: F) -> Controller<Self>
    where
        X: 'static,
        F: Fn(Self) -> X + 'static,
    {
        let root_window = relm4::main_application().window_by_id(*crate::ROOT_WINDOW_ID.read());
        let root_window = root_window.as_ref();
        let mut ctrl = Self::builder().launch(self).forward(sender, transform);
        // WARN: by design yes this is a memleak but we have no choice
        ctrl.detach_runtime();
        ctrl.widget().set_transient_for(root_window);
        libhelium::prelude::WindowExt::set_parent(ctrl.widget(), root_window);
        ctrl.widget().present();
        ctrl
    }
}
