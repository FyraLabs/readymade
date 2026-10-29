use crate::prelude::*;
use crate::ui::{DestinationState, DiskEntry, SelectedDisk};
use libreadymade::disks::Disk;
use std::sync::LazyLock;

pub static DISKS_DATA: LazyLock<Vec<Disk>> = LazyLock::new(|| {
    let mut v = libreadymade::disks::detect_os();
    v.sort();
    // the old FlowBox pushed each disk to the front, keep showing them in the same order
    v.reverse();
    v
});

fn os_name(disk: &Disk) -> String {
    disk.os_name.clone().unwrap_or_else(|| t!("unknown-os"))
}

pub fn init(ui: &AppWindow) {
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let disks = DISKS_DATA
            .iter()
            .map(|disk| DiskEntry {
                name: disk.disk_name.to_shared_string(),
                os: os_name(disk).into(),
                size: disk.size.to_shared_string(),
            })
            .collect_vec();
        _ = weak.upgrade_in_event_loop(move |ui| {
            let state = ui.global::<DestinationState<'_>>();
            state.set_disks(model(disks));
            state.set_scanning(false);
        });
    });

    let weak = ui.as_weak();
    ui.global::<DestinationState<'_>>().on_select(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(disk) = usize::try_from(index).ok().and_then(|i| DISKS_DATA.get(i)) else {
            return;
        };
        ui.global::<DestinationState<'_>>().set_selected(index);
        let selected = ui.global::<SelectedDisk<'_>>();
        selected.set_name(disk.disk_name.to_shared_string());
        selected.set_os(os_name(disk).into());
        APPLICATION_STATE.write().destination_disk = Some(disk.clone());
    });
}
