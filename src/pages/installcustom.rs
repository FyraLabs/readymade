use std::path::PathBuf;

use relm4::factory::FactoryVecDeque;

use crate::{
    backend::custom::MountTarget as ChooseMount, prelude::*, NavigationAction, INSTALLATION_STATE,
};

/// Wizard to set up custom partitioning.
///
/// Users should also be able to label their partition with a special magic value to suggest
/// Readymade to use that partition for a specific purpose, like /boot or /home.
///
/// i.e. If the user labels their partition `ROOT`, Readymade will automatically use that partition as the root partition.
/// Or `ESP` for the EFI System Partition, `XBOOTLDR` for the bootloader partition, etc.
///
/// We may also handle cases where a BTRFS subvolume is labeled as `ROOT` or `HOME` automatically select it as the home partition.
pub struct InstallCustomPage {
    pub choose_mount_factory: FactoryVecDeque<ChooseMount>,
    pub root: libhelium::ViewMono,
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
    OpenPartTool,
}

#[derive(Debug)]
pub enum InstallCustomPageOutput {
    Navigate(NavigationAction),
}

// todo: Open PartitionToolSelector window?
// And then a button to re-open that window?
// And a button to refresh the partitions?

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
                set_label: &t!("page-installcustom"),
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
                    set_title: &t!("page-installcustom-title"),

                    #[watch]
                    set_description: &t!("page-installcustom-desc", num = model.choose_mount_factory.len()),

                    prepend_button[libhelium::BottomBarPosition::Right] = &libhelium::Button {
                        set_is_iconic: true,
                        #[watch]
                        set_tooltip: &t!("page-installcustom-tool"),

                        set_icon: Some("preferences-system-symbolic"),
                        connect_clicked => InstallCustomPageMsg::OpenPartTool,
                    },

                    prepend_button[libhelium::BottomBarPosition::Left] = &libhelium::Button {
                        set_is_iconic: true,
                        #[watch]
                        set_tooltip: &t!("page-installcustom-add"),

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
            root: root.clone(),
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
                .make_window(
                    self.root.clone(),
                    sender.input_sender(),
                    InstallCustomPageMsg::UpdateRow,
                );
            }
            InstallCustomPageMsg::OpenPartTool => {
                _ = PartitionToolSelector::make_window(self.root.clone());
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
                    .make_window(
                        self.root.clone(),
                        sender.input_sender(),
                        InstallCustomPageMsg::UpdateRow,
                    );
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
                set_tooltip: &t!("installtype-edit-mp"),
                add_css_class: "suggested-action",
                connect_clicked => ChooseMountMsg::Edit,
            },

            libhelium::Button {
                set_icon_name: "edit-clear-symbolic",
                #[watch]
                set_tooltip: &t!("installtype-rm-mp"),
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
// PartitionTypeDropdown
#[derive(Debug, Default, Clone)]
pub enum PartitionType {
    /// Root partition
    #[default]
    Root,
    /// /boot (xbootldr)
    ExtendedBoot,
    /// /boot/efi (esp)
    Esp,
    /// /home
    Home,
    /// /var
    Var,
    Other(String),
}

macro_rules! impl_partition_type {
    ($($entry:ident $s:literal),*$(,)?) => {
        impl PartitionType {
            #[must_use] fn all() -> &'static [Self] {
                Box::leak(Box::new([$(Self::$entry),*, Self::Other("".to_owned())]))
            }

            #[allow(clippy::missing_const_for_fn)]
            fn as_str(&self) -> &str {
                match self {
                    $(Self::$entry => $s,)*
                    Self::Other(s) => s,
                }
            }

            #[cfg(any())]
            fn from_str(s: &str) -> Self {
                match s {
                    $($s => Self::$entry,)*
                    _ => Self::Other(s.to_string()),
                }
            }

            fn description_string(&self) -> String {
                match self {
                    $(Self::$entry =>
                            paste::paste! {with_builtin_macros::with_builtin!(let $msgid =
                                concat!("parttype-", stringify!([<$entry:lower>]))
                                in { t!($msgid, path = $s) }
                            )
                        },
                    )*
                    Self::Other(_) => t!("parttype-other"),
                }
            }
        }
    };
}

impl_partition_type!(Root "/", ExtendedBoot "/boot", Esp "/boot/efi", Home "/home", Var "/var");

/// Dropdown the set the mountpoint type, for ease of use.
///
/// There will be an Other option to allow the user to type in their own mountpoint,
/// which creates a text entry next to the dropdown.
///
/// This will be a `Gtk::Box` containing a `Gtk::ComboBoxText` and a `Gtk::Entry`.
#[derive(Debug, Default, Clone, Copy)]
struct PartitionTypeDropdown {
    is_other: bool,
}

#[relm4::component]
impl SimpleComponent for PartitionTypeDropdown {
    type Init = ();
    type Input = PartitionType;
    type Output = PartitionType;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 4,
            set_halign: gtk::Align::Fill,
            set_hexpand: true,

            #[local_ref] dropdown ->
            gtk::DropDown {
                connect_selected_notify[sender] => move |dropdown| {
                    let selected = PartitionType::all()[dropdown.selected() as usize].clone();
                    tracing::trace!(?selected, "PartitionTypeDropdown::selected");
                    sender.input(selected);
                },
            },

            #[name = "entry"]
            gtk::Entry {
                set_halign: gtk::Align::Fill,
                set_hexpand: false,
                #[watch]
                set_visible: model.is_other,

                connect_changed[sender] => move |entry| {
                    let text = entry.text();
                    if !text.is_empty() {
                        sender.output(PartitionType::Other(text.into())).unwrap();
                    }
                },
            },
        },
    }

    fn init(
        (): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self::default();
        let parttypes = PartitionType::all()
            .iter()
            .map(PartitionType::description_string)
            .collect_vec();

        let dropdown =
            gtk::DropDown::from_strings(&parttypes.iter().map(String::as_str).collect_vec());

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        if let PartitionType::Other(_) = message {
            self.is_other = true;
        } else {
            self.is_other = false;
        }
        sender.output(message).unwrap();
    }
}

// ────────────────────────────────────────────────────────────────────────────
// AddDialog (also for edit)

kurage::generate_component!(AddDialog {
    partition: String,
    mountpoint: String,
    mountopts: String,
    index: usize,
}:
    preinit {
        let mut init = init;
        tracing::trace!(?init, "Spawned AddDialog");
        PartitionType::default()
            .as_str()
            .clone_into(&mut init.mountpoint);
        // populate partition dropdown list
        let mut mp_dropdown = PartitionTypeDropdown::builder()
            .launch(())
            .forward(sender.input_sender(), AddDialogMsg::SelectMnptType);
        mp_dropdown.detach_runtime();
        let dd_at = mp_dropdown.widget();
        let partlist = lsblk::BlockDevice::list().unwrap();
        let partlist = (partlist.iter())
            .filter(|b| b.is_part())
            .map(|p| p.fullname.clone());
        let partvec = partlist.sorted().collect_vec();
        let partlist =
            &gtk::DropDown::from_strings(&partvec.iter().filter_map(|s| s.to_str()).collect_vec());

        let (sd0, sd2) = (sender.clone(), sender.clone());
        let partvec0 = partvec.clone();
        // connect signal for the dropdown
        partlist.connect_selected_notify(move |dropdown| {
            sd0.input(AddDialogMsg::ChangedPart(
                #[allow(clippy::indexing_slicing)]
                partvec0[dropdown.selected() as usize].display().to_string(),
            ));
        });

        // todo: binding dropdown
        // when other selected:
        // - enable text field inside
        // - treat the text field like the old tf_at
        // - get the value from the now-visible text field and set the mountpoint to that value
        // when other is not selected:
        // - hide text field again
        // - blank out the text field if it was filled, so it won't fill in the mountpoint
        // - get the value from the dropdown using the `to_string` method, and set the mountpoint to that value

        let mut model = init;
    }
    init(root, sender, model, widgets) for init: Self {
        // connect signal for textfields
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
        // widgets.tf_at.internal_entry().set_text(&model.mountpoint);
        widgets.tf_opts.internal_entry().set_text(&model.mountopts);
    }

    update(self, message, sender) {
            ChangedPart(part: String) => self.partition = part,
            // ChangedMnpt(mnpt) => self.mountpoint = mnpt,
            ChangedOpts(opts: String) => self.mountopts = opts,
            Close(window: libhelium::Window) => {
                sender.output(std::mem::take(self)).unwrap();
                window.close();
            },
            SelectMnptType(t: PartitionType) => {
                // if let PartitionType::Other(_) = t {
                //     self.mnpt_type_is_other = true;
                // } else {
                //     self.mnpt_type_is_other = false;
                // }
                t.as_str().clone_into(&mut self.mountpoint);
            }
    } => Self

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
                    set_label: &t!("dialog-mp-part"),
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
                    set_label: &t!("dialog-mp-at"),
                },

                #[local_ref] dd_at ->
                gtk::Box {},
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_hexpand: true,
                set_spacing: 3,

                gtk::Label {
                    #[watch]
                    set_label: &t!("dialog-mp-opts"),
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
                // HACK: relm4 doesn't perform the #[watch] until the UI is updated by the user
                // e.g. they typed something into the entry, then relm4 actually finally realize
                // it needs to set this as sensitive
                //
                // therefore right here we just have relm4 default to a sensitivity before any UI trigs
                set_sensitive: !model.partition.is_empty(),
            },
        },
    }
);

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
    fn make_window<X, F>(
        self,
        widget: impl IsA<gtk::Widget>,
        sender: &relm4::Sender<X>,
        transform: F,
    ) -> Controller<Self>
    where
        X: 'static,
        F: Fn(Self) -> X + 'static,
    {
        let root_window = widget.toplevel_window();
        let mut ctrl = Self::builder().launch(self).forward(sender, transform);
        // WARN: by design yes this is a memleak but we have no choice
        ctrl.detach_runtime();
        ctrl.widget().set_transient_for(root_window.as_ref());
        libhelium::prelude::WindowExt::set_parent(ctrl.widget(), root_window.as_ref()); // FIXME: doubt if this actually works
                                                                                        // temporary hack is to use the gtk one instead
        gtk::prelude::WidgetExt::set_parent(ctrl.widget(), &root_window.expect("no root window"));
        ctrl.widget().present();
        ctrl
    }
}

const PARTITION_TOOLS_LIST: &[&str] = &[
    "gparted",
    "org.gnome.DiskUtility",
    "org.kde.partitionmanager",
    "blivet-gui",
];

// ────────────────────────────────────────────────────────────────────────────
// PartitionToolSelector

kurage::generate_component!(PartitionToolSelector {
    entry_factory: FactoryPartEntry,
}:
    init[entries { model.entry_factory.inner.widget() }](root, sender, model, widgets) {}
    update(self, message, _sender) {} => {}

    libhelium::ApplicationWindow {
        set_title: Some(&t!("installtype-parttool")),

        #[wrap(Some)]
        set_child = &gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_align: gtk::Align::Fill,
            set_vexpand: true,
            set_hexpand: true,

            libhelium::AppBar {},

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 16,
                set_align: gtk::Align::Fill,
                set_margin_all: 16,
                set_margin_top: 0,
                set_vexpand: true,
                set_hexpand: true,

                gtk::Label {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Start,
                    #[watch]
                    set_label: &t!("installtype-parttool"),
                    set_css_classes: &["view-title"],
                },

                #[local_ref] entries ->
                gtk::Box {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::End,
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 16,
                    set_homogeneous: true,
                },
            },
        },
    }
);

#[derive(Debug)]
struct FactoryPartEntry {
    inner: FactoryVecDeque<PartitionToolEntry>,
}

impl Default for FactoryPartEntry {
    fn default() -> Self {
        let mut inner = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .detach();
        let mut guard = inner.guard();
        PartitionToolSelector::list_partitioning_tools().for_each(|entry| {
            guard.push_back(entry);
        });
        drop(guard);
        Self { inner }
    }
}

/// A new pop-up window to select a partitioning tool, should take in a list of [`DesktopEntry`]'s
/// and ask the user which one to open, then call some function to open the selected tool using the
/// desktop entry data.
impl PartitionToolSelector {
    fn list_partitioning_tools() -> impl Iterator<Item = freedesktop_desktop_entry::DesktopEntry> {
        PARTITION_TOOLS_LIST
            .iter()
            .copied()
            .filter_map(Self::query_desktop_entry)
    }
    /// Get the XDG desktop entry for a given desktop entry name.
    ///
    /// # Arguments
    ///
    /// * `desktop_entry` - The name of the desktop entry to query. (foo.desktop)
    fn query_desktop_entry(desktop_entry: &str) -> Option<freedesktop_desktop_entry::DesktopEntry> {
        let locales = freedesktop_desktop_entry::get_languages_from_env();
        // we could do from_path() but we want to future proof this in case XDG or Fedora
        // changes some paths from now on, plus we don't have to hardcode the paths
        let x = freedesktop_desktop_entry::Iter::new(freedesktop_desktop_entry::default_paths())
            .entries(Some(&locales))
            .find(|entry| entry.appid == desktop_entry);
        x // FIXME: why can't we inline this variable
    }
    fn make_window(widget: impl IsA<gtk::Widget>) -> Controller<Self> {
        let root_window = widget.toplevel_window();
        let mut ctrl = Self::builder().launch(()).detach();
        // XXX: by design yes this is a memleak but we have no choice
        ctrl.detach_runtime();
        ctrl.widget().set_transient_for(root_window.as_ref());

        ctrl.widget()
            .set_parent(&root_window.expect("no root window"));
        ctrl.widget().present();
        ctrl
    }
}

/// Partition tool selector entry,
/// should be a factory to generate from a single [`freedesktop_desktop_entry::DesktopEntry`]
/// layout:
/// ```no_run
/// button {
///    box {
///         image,
///         label,
///     }
/// }
/// ```
#[derive(Debug)]
struct PartitionToolEntry {
    desktop_entry: freedesktop_desktop_entry::DesktopEntry,
}

impl Default for PartitionToolEntry {
    fn default() -> Self {
        Self {
            desktop_entry: freedesktop_desktop_entry::DesktopEntry::from_appid(String::default()),
        }
    }
}

#[relm4::factory(pub)]
impl FactoryComponent for PartitionToolEntry {
    type ParentWidget = gtk::Box;
    type Input = ();
    type Output = ();
    type CommandOutput = ();
    type Init = freedesktop_desktop_entry::DesktopEntry;

    view! {
        gtk::Button {
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_margin_all: 8,

                gtk::Image {
                    set_icon_name: self.desktop_entry.icon(),
                    set_pixel_size: 64,
                },
                gtk::Label {
                    set_label: &self.desktop_entry.name(&freedesktop_desktop_entry::get_languages_from_env()).map_or(String::new(), |name| name.to_string()),
                    add_css_class: "cb-subtitle",
                    set_wrap: true,
                    set_wrap_mode: gtk::pango::WrapMode::Word,
                },
            },
            connect_clicked[path = self.desktop_entry.path.clone()] => move |_| {
                // expect() should work here because we have already triple-checked and filtered
                // the broken entries in both
                // `PartitionToolSelector::list_partitioning_tools()` and `PartitionToolSelector::query_desktop_entry()`
                // so we can safely assume that the desktop entry is valid
                //
                // If for some arcane reason it's suddenly no longer valid, it's corrupted way beyond our control
                let appinfo =
                    gtk::gio::DesktopAppInfo::from_filename(&*path)
                        .expect("Invalid desktop file");

                let launch_ctx = gtk::gio::AppLaunchContext::new();
                appinfo.launch_uris(&[], Some(&launch_ctx)).expect("cannot launch?");
            },
        }
    }

    fn init_model(init: Self::Init, _: &Self::Index, _: FactorySender<Self>) -> Self {
        Self {
            desktop_entry: init,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_desktop_entry() {
        let iter = PartitionToolSelector::query_desktop_entry("gparted");
        if let Some(entry) = iter {
            println!("{entry:?}");
        }
    }
}
