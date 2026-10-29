use std::{
    cell::RefCell, os::unix::fs::FileTypeExt, path::PathBuf, process::Command, rc::Rc,
    time::Duration,
};

use crate::ui::{ConfirmationState, Nav};
use crate::{cfg::InstallationType, prelude::*};

thread_local! {
    static PROBLEM_TIMER: slint::Timer = slint::Timer::default();
}

pub fn init(ui: &AppWindow) {
    let weak = ui.as_weak();
    PROBLEM_TIMER.with(|timer| {
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.global::<Nav<'_>>().get_page() != Page::Confirmation {
                return;
            }
            let problem = Problem::detect().as_ref().map(Problem::msg).unwrap_or_default();
            ui.global::<ConfirmationState<'_>>().set_problem(problem.into());
        });
    });

    let warns = Rc::new(RefCell::new(Vec::<Warning>::new()));
    let state = ui.global::<ConfirmationState<'_>>();

    let (weak, warns2) = (ui.as_weak(), Rc::clone(&warns));
    state.on_start(move || {
        let Some(ui) = weak.upgrade() else { return };
        *warns2.borrow_mut() = Warning::list().collect_vec();
        show_next_warning(&ui, &warns2);
    });

    let (weak, warns2) = (ui.as_weak(), Rc::clone(&warns));
    state.on_warning_confirm(move || {
        let Some(ui) = weak.upgrade() else { return };
        show_next_warning(&ui, &warns2);
    });

    let weak = ui.as_weak();
    state.on_warning_cancel(move || {
        let Some(ui) = weak.upgrade() else { return };
        warns.borrow_mut().clear();
        ui.global::<ConfirmationState<'_>>().set_warning_open(false);
    });
}

/// Pop the next warning into the dialog, or start installing once the user went through all of them.
fn show_next_warning(ui: &AppWindow, warns: &RefCell<Vec<Warning>>) {
    let state = ui.global::<ConfirmationState<'_>>();
    if let Some(warn) = warns.borrow_mut().pop() {
        state.set_warning_title(warn.title().into());
        state.set_warning_desc(warn.desc().into());
        state.set_warning_open(true);
    } else {
        state.set_warning_open(false);
        crate::pages::installation::start(ui);
    }
}

#[derive(Debug, Clone)]
enum Problem {
    DeviceMounted(String, PathBuf),
    DevBlkOpen(String, Vec<usize>),
}

impl Problem {
    fn detect() -> Option<Self> {
        let Some(disk) = &APPLICATION_STATE.read().destination_disk else {
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
        let msg = match self {
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
        };
        // the translations still carry Pango markup from the GTK days
        msg.replace("<tt>", "").replace("</tt>", "")
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
                APPLICATION_STATE
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
                APPLICATION_STATE.read().installation_type.unwrap() == InstallationType::WholeDisk
            })
    }

    fn list() -> impl Iterator<Item = Self> {
        std::iter::once(Self::efi_part_found()).flatten()
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
}
