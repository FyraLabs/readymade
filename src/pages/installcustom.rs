use std::cell::RefCell;

use crate::prelude::*;
use crate::ui::{InstallCustomState, MountRow, PartTool};
use libreadymade::prelude::{Mount, Mounts};

/// A partition the user wants mounted somewhere in the new system.
#[derive(Debug, Clone, Default)]
struct MountTarget {
    partition: String,
    mountpoint: String,
    mountopts: String,
}

impl MountTarget {
    fn to_mount(&self) -> Mount {
        crate::state::mount_from_custom_target(&self.partition, &self.mountpoint, &self.mountopts)
    }

    fn to_ui(&self) -> MountRow {
        MountRow {
            partition: self.partition.to_shared_string(),
            mountpoint: self.mountpoint.to_shared_string(),
            options: self.mountopts.to_shared_string(),
        }
    }
}

#[derive(Default)]
struct State {
    mounts: Vec<MountTarget>,
    /// Row being edited by the mount dialog, `None` when adding a new one.
    editing: Option<usize>,
    /// Partitions listed in the mount dialog, in the same order as the dropdown.
    partitions: Vec<String>,
    tools: Vec<freedesktop_desktop_entry::DesktopEntry>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// The mount targets configured on this page.
pub fn mounts() -> Mounts {
    STATE.with_borrow(|state| Mounts(state.mounts.iter().map(MountTarget::to_mount).collect()))
}

fn refresh_rows(ui: &AppWindow) {
    let rows = STATE.with_borrow(|state| state.mounts.iter().map(MountTarget::to_ui).collect_vec());
    ui.global::<InstallCustomState<'_>>().set_mounts(model(rows));
}

fn list_partitions() -> Vec<String> {
    lsblk::BlockDevice::list()
        .inspect_err(|e| tracing::error!(?e, "cannot list block devices"))
        .unwrap_or_default()
        .iter()
        .filter(|b| b.is_part())
        .map(|p| p.fullname.display().to_string())
        .sorted()
        .collect()
}

/// Fill in and show the mount dialog for `target`, or a blank one.
fn open_mount_dialog(ui: &AppWindow, editing: Option<usize>, target: &MountTarget) {
    let partitions = list_partitions();
    let types = PartitionType::all();
    let state = ui.global::<InstallCustomState<'_>>();

    let partition = partitions
        .iter()
        .position(|p| *p == target.partition)
        .unwrap_or_default();
    let (mountpoint_type, custom) = types
        .iter()
        .position(|t| !matches!(t, PartitionType::Other(_)) && t.as_str() == target.mountpoint)
        .map_or_else(
            || (types.len() - 1, target.mountpoint.as_str()),
            |i| (i, ""),
        );

    state.set_partitions(model(partitions.iter().map(|p| p.to_shared_string())));
    state.set_mountpoint_types(model(
        types.iter().map(|t| t.description_string().into()),
    ));
    state.set_dialog_partition(i32::try_from(partition).unwrap_or_default());
    state.set_dialog_mountpoint_type(i32::try_from(mountpoint_type).unwrap_or_default());
    state.set_dialog_custom_mountpoint(custom.into());
    state.set_dialog_options(target.mountopts.to_shared_string());
    STATE.with_borrow_mut(|state| {
        state.editing = editing;
        state.partitions = partitions;
    });
    state.set_mount_dialog_open(true);
}

pub fn init(ui: &AppWindow) {
    let state = ui.global::<InstallCustomState<'_>>();

    let weak = ui.as_weak();
    state.on_add(move || {
        let Some(ui) = weak.upgrade() else { return };
        let target = MountTarget {
            mountpoint: PartitionType::default().as_str().to_owned(),
            ..MountTarget::default()
        };
        open_mount_dialog(&ui, None, &target);
    });

    let weak = ui.as_weak();
    state.on_edit(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Ok(index) = usize::try_from(index) else { return };
        let Some(target) = STATE.with_borrow(|state| state.mounts.get(index).cloned()) else {
            tracing::error!(index, "request to edit nonexistent row");
            return;
        };
        tracing::trace!(?target, "Edit MountTarget");
        open_mount_dialog(&ui, Some(index), &target);
    });

    let weak = ui.as_weak();
    state.on_remove(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        STATE.with_borrow_mut(|state| {
            if let Ok(index) = usize::try_from(index)
                && index < state.mounts.len()
            {
                state.mounts.remove(index);
            }
        });
        refresh_rows(&ui);
    });

    let weak = ui.as_weak();
    state.on_save(move |partition, mountpoint_type, custom, options| {
        let Some(ui) = weak.upgrade() else { return };
        STATE.with_borrow_mut(|state| {
            let Some(partition) = usize::try_from(partition)
                .ok()
                .and_then(|i| state.partitions.get(i))
            else {
                return;
            };
            let mountpoint = match usize::try_from(mountpoint_type)
                .ok()
                .and_then(|i| PartitionType::all().get(i))
            {
                Some(PartitionType::Other(_)) | None => custom.to_string(),
                Some(t) => t.as_str().to_owned(),
            };
            let target = MountTarget {
                partition: partition.clone(),
                mountpoint,
                mountopts: options.to_string(),
            };
            match state.editing.take() {
                Some(i) if i < state.mounts.len() => state.mounts[i] = target,
                _ => state.mounts.push(target),
            }
        });
        refresh_rows(&ui);
    });

    let weak = ui.as_weak();
    state.on_open_tools(move || {
        let Some(ui) = weak.upgrade() else { return };
        let locales = freedesktop_desktop_entry::get_languages_from_env();
        let tools = list_partitioning_tools().collect_vec();
        let rows = tools.iter().map(|entry| PartTool {
            name: entry
                .name(&locales)
                .map_or_else(Default::default, |name| name.to_shared_string()),
            icon: entry
                .icon()
                .map(|icon| crate::ui::icon(icon, 64))
                .unwrap_or_default(),
        });
        let state = ui.global::<InstallCustomState<'_>>();
        state.set_tools(model(rows));
        STATE.with_borrow_mut(|state| state.tools = tools);
        state.set_tools_open(true);
    });

    state.on_launch_tool(|index| {
        STATE.with_borrow(|state| {
            if let Some(entry) = usize::try_from(index).ok().and_then(|i| state.tools.get(i)) {
                launch(entry);
            }
        });
    });
}

fn launch(entry: &freedesktop_desktop_entry::DesktopEntry) {
    let args = match entry.parse_exec() {
        Ok(args) => args,
        Err(e) => return tracing::error!(?e, path = ?entry.path, "cannot parse Exec="),
    };
    let Some((cmd, args)) = args.split_first() else {
        return tracing::error!(path = ?entry.path, "empty Exec=");
    };
    if let Err(e) = std::process::Command::new(cmd).args(args).spawn() {
        tracing::error!(?e, cmd, "cannot launch partitioning tool");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PartitionType

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
    ($($entry:ident $msgid:literal $s:literal),*$(,)?) => {
        impl PartitionType {
            #[must_use] fn all() -> &'static [Self] {
                static ALL: &[PartitionType] = &[$(PartitionType::$entry),*, PartitionType::Other(String::new())];
                ALL
            }

            #[allow(clippy::missing_const_for_fn)]
            fn as_str(&self) -> &str {
                match self {
                    $(Self::$entry => $s,)*
                    Self::Other(s) => s,
                }
            }

            fn description_string(&self) -> String {
                match self {
                    $(Self::$entry => t!($msgid, path = $s),)*
                    Self::Other(_) => t!("parttype-other"),
                }
            }
        }
    };
}

impl_partition_type!(
    Root "parttype-root" "/",
    ExtendedBoot "parttype-extendedboot" "/boot",
    Esp "parttype-esp" "/boot/efi",
    Home "parttype-home" "/home",
    Var "parttype-var" "/var",
);

// ────────────────────────────────────────────────────────────────────────────
// Partitioning tools

const PARTITION_TOOLS_LIST: &[&str] = &[
    "gparted",
    "org.gnome.DiskUtility",
    "org.kde.partitionmanager",
    "blivet-gui",
];

fn list_partitioning_tools() -> impl Iterator<Item = freedesktop_desktop_entry::DesktopEntry> {
    PARTITION_TOOLS_LIST
        .iter()
        .copied()
        .filter_map(query_desktop_entry)
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
    freedesktop_desktop_entry::Iter::new(freedesktop_desktop_entry::default_paths())
        .entries(Some(&locales))
        .find(|entry| entry.appid == desktop_entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_desktop_entry() {
        let iter = query_desktop_entry("gparted");
        if let Some(entry) = iter {
            println!("{entry:?}");
        }
    }
}
