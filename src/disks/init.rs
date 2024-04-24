use crate::albius::DiskOperation;
use color_eyre::{
    eyre::{eyre, OptionExt},
    Result,
};
use std::path::{Path, PathBuf};

const MIB: u64 = 1024 * 1024;

macro_rules! instr {
    ($vec:ident:$($disk:ident $op:ident $($arg:expr),+);+) => {
        $(
            $vec.push($crate::albius::DiskOperation {
                disk: $disk.clone(),
                operation: $crate::albius::DiskOperationType::$op,
                params: vec![$(format!("{}", $arg)),+],
            });
        )+
    }
}

/// Erase the current partition table, make 3 partitions:
/// - /boot/efi   250 MiB
/// - /boot         1 GiB
/// - /         remaining
pub fn clean_install(diskpath: &Path) -> Result<Vec<DiskOperation>> {
    let disks = rs_drivelist::drive_list().map_err(|e| eyre!(e))?;
    let disk = (disks.into_iter())
        .find(|d| (d.devicePath.as_ref()).map_or(false, |p| PathBuf::from(p) == diskpath));
    let diskobj = disk.ok_or_eyre(eyre!("Cannot find disk: {diskpath:?}"))?;
    let disk = diskpath.to_path_buf();
    let sdiskpath = diskpath.display().to_string();
    let disksize = diskobj.size / MIB;
    let orignumparts = cmd_lib::run_fun!(lsblk -o path)?
        .split('\n')
        .filter(|l| l.starts_with(&sdiskpath) && l != &sdiskpath)
        .count();

    let mut ops = vec![];

    // erase all partitions
    for n in 1..=orignumparts {
        instr!(ops: disk Rm format!("{n}"));
    }

    instr!(ops:
        disk Label "gpt";
        // params: Name, FsType, Start, End (MiB)
        disk Mkpart "Boot", "ext4", 1, 1025;
        disk Mkpart "EFI", "fat32", 1025, 1275;
        disk Mkpart "LinuxRoot", "btrfs", 1275, disksize - 1
    );

    Ok(ops)
}
