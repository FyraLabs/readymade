use std::{os::unix::fs::FileTypeExt, path::PathBuf, process::Command, time::Duration};

use crate::prelude::*;

page!(Confirmation {
    problem: Option<Problem>,
    root: libhelium::ViewMono,
    warns: Vec<Warning>,
}:
    init(root, sender, model, widgets) {
        gtk::glib::timeout_add(Duration::from_secs(1), move || {
            sender.input(Self::Input::Check);
            gtk::glib::ControlFlow::Continue
        });
        model.root = root;
    }

    update(self, message, sender) {
        WarnCancel => {
            self.warns.clear();
        },
        WarnConfirm => {
            let Some(warn) = self.warns.pop() else {
                sender
                    .output(Self::Output::StartInstallation)
                    .unwrap();

                sender
                    .output(Self::Output::Navigate(NavigationAction::GoTo(
                        Page::Installation,
                    )))
                    .unwrap();
                return;
            };
            warn.pop(self.root.clone(), &sender);
        },
        StartInstallation => {
            self.warns = Warning::list().collect_vec();
            let Some(warn) = self.warns.pop() else {
                sender
                    .output(Self::Output::StartInstallation)
                    .unwrap();

                sender
                    .output(Self::Output::Navigate(NavigationAction::GoTo(
                        Page::Installation,
                    )))
                    .unwrap();
                return;
            };
            warn.pop(self.root.clone(), &sender);
        },
        Check => {
            self.problem = Problem::detect();
        }
    } => { StartInstallation }

    gtk::CenterBox {
        set_orientation: gtk::Orientation::Horizontal,
        set_valign: gtk::Align::Center,
        set_vexpand: true,

        #[wrap(Some)]
        set_start_widget = &gtk::Box {
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

        #[wrap(Some)]
        set_center_widget = &gtk::Image {
            set_icon_name: Some("go-next-symbolic"),
            inline_css: "-gtk-icon-size: 64px",
            set_margin_horizontal: 16,
        },

        #[wrap(Some)]
        set_end_widget = &gtk::Box {
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

    // relm4 doesn't support if lets
    gtk::Label {
        #[watch]
        set_label: &model.problem.as_ref().map(Problem::msg).unwrap_or_default(),
        set_use_markup: true,
        add_css_class: "error",
    },

    gtk::Box {
        set_orientation: gtk::Orientation::Horizontal,
        set_spacing: 4,

        libhelium::Button {
            set_is_pill: true,
            #[watch]
            set_label: &t!("prev"),
            add_css_class: "large-button",
            connect_clicked => ConfirmationPageMsg::Navigate(NavigationAction::GoTo(
                    crate::Page::InstallationType
            )),
        },

        gtk::Box {
            set_hexpand: true,
        },

        libhelium::Button {
            #[watch]
            set_sensitive: model.problem.is_none(),
            set_is_pill: true,
            #[watch]
            set_label: &t!("page-welcome-install"),
            add_css_class: "large-button",
            add_css_class: "destructive-action",
            connect_clicked => ConfirmationPageMsg::StartInstallation
        },
    }
);

#[derive(Debug, Clone)]
enum Problem {
    DeviceMounted(String, PathBuf),
    DevBlkOpen(String, Vec<usize>),
}

impl Problem {
    fn detect() -> Option<Self> {
        let Some(disk) = &INSTALLATION_STATE.read().destination_disk else {
            return None;
        };
        let disk = &disk.devpath.to_string_lossy();
        // assumes partition devpath must starts with disk devpath
        if let Some(dev) = lsblk::Mount::list()
            .inspect_err(|e| tracing::error!(?e, "cannot list mounts"))
            .into_iter()
            .flatten()
            .find(|dev| dev.device.starts_with(disk.as_ref()))
        {
            return Some(Self::DeviceMounted(dev.device, dev.mountpoint));
        }
        if let Some(procs) = find_open_block_devices(disk.as_ref())
            .ok()
            .filter(|procs| !procs.is_empty())
        {
            return Some(Self::DevBlkOpen(disk.to_string(), procs));
        }
        None
    }
    fn msg(&self) -> String {
        match self {
            Self::DeviceMounted(dev, mp) => t!(
                "page-confirmation-problem-device-mounted",
                dev = dev,
                mountpoint = mp.to_string_lossy().to_string(),
            ),
            Self::DevBlkOpen(dev, pids) => t!(
                "page-confirmation-problem-devblkopen",
                dev = dev,
                pids = pids.iter().join(", "),
            ),
        }
    }
}

fn find_open_block_devices(device: &str) -> std::io::Result<Vec<usize>> {
    use std::fs::read_dir;
    Ok(read_dir("/proc")?
        .filter_map(Result::ok)
        .filter_map(|f| f.file_name().to_string_lossy().parse().ok())
        .flat_map(|pid: usize| {
            read_dir(format!("/proc/{pid}/fd"))
                .into_iter()
                .flat_map(Iterator::flatten)
                .filter_map(|fd_entry| fd_entry.path().canonicalize().ok())
                .filter(|target| target.to_string_lossy().starts_with(device))
                .filter_map(|target| target.metadata().ok())
                .filter_map(move |metadata| metadata.file_type().is_block_device().then_some(pid))
        })
        .collect())
}

#[derive(Debug, Default)]
enum Warning {
    #[default]
    EfiPartFound, // #110
}

impl Warning {
    fn efi_part_found() -> Option<Self> {
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "lsblk {} -o parttype | grep 'c12a7328-f81f-11d2-ba4b-00a0c93ec93b'",
                INSTALLATION_STATE
                    .read()
                    .destination_disk
                    .as_ref()
                    .unwrap()
                    .devpath
                    .display()
            ))
            .status()
            .expect("cannot execute sh")
            .success()
            .then_some(Self::EfiPartFound)
            .filter(|_| {
                INSTALLATION_STATE.read().installation_type.unwrap()
                    == crate::backend::install::InstallationType::WholeDisk
            })
    }

    fn list() -> impl Iterator<Item = Self> {
        [Self::efi_part_found()].into_iter().flatten()
    }

    fn title(&self) -> String {
        match self {
            Self::EfiPartFound => t!("dialog-confirm-warn-efipartfound-title"),
        }
    }

    fn desc(&self) -> String {
        match self {
            Self::EfiPartFound => t!("dialog-confirm-warn-efipartfound-desc"),
        }
    }

    fn pop(
        self,
        parent: impl IsA<gtk::Widget>,
        sender: &ComponentSender<ConfirmationPage>,
    ) -> Controller<Self> {
        let root_window = parent.toplevel_window().unwrap();
        let mut ctrl = Self::builder().launch((self, root_window.clone())).forward(
            sender.input_sender(),
            |b| {
                if b {
                    ConfirmationPageMsg::WarnConfirm
                } else {
                    ConfirmationPageMsg::WarnCancel
                }
            },
        );
        // XXX: by design yes this is a memleak but we have no choice
        ctrl.detach_runtime();
        // ctrl.widget().set_transient_for(root_window.as_ref());

        ctrl.widget().set_parent(&root_window);
        ctrl.widget().present();
        ctrl
    }
}

#[relm4::component]
impl relm4::SimpleComponent for Warning {
    type Init = (Self, gtk::Window);
    type Input = ();
    type Output = bool;

    view! {
        libhelium::Dialog {
            set_title: &model.title(),
            set_icon: "dialog-error-symbolic",
            connect_destroy[sender] => move |_| sender.output(false).unwrap(),
        },
    }

    fn init(
        (model, root_window): Self::Init,
        #[expect(unused_assignments)] mut root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let btn = libhelium::Button::new(None, Some(&t!("dialog-installtype-confirm")));
        root = libhelium::Dialog::new(
            &root_window,
            Some(&model.title()),
            Some(&model.desc()),
            Some("dialog-error-symbolic"),
            Some(&btn),
            None::<&libhelium::Button>,
        );
        let widgets = view_output!();
        btn.connect_clicked(move |_| sender.output(true).unwrap());
        ComponentParts { model, widgets }
    }
}
