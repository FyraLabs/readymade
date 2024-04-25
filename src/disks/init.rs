use crate::albius::DiskOperation;
use color_eyre::{
    eyre::{eyre, OptionExt},
    Report, Result, Section,
};
use std::path::{Path, PathBuf};
use tracing::{debug, info, trace};

const MIB: u64 = 1024 * 1024;

macro_rules! instr {
    ($vec:ident:$($disk:ident $op:ident $($arg:expr),+);+) => { {
        $(
            $vec.push($crate::albius::DiskOperation {
                disk: $disk.clone(),
                operation: $crate::albius::DiskOperationType::$op,
                params: vec![$(format!("{}", $arg)),+],
            });
        )+
    } }
}

/// Erase the current partition table, make 3 partitions:
/// - /boot/efi   512 MiB
/// - /boot         1 GiB
/// - /         remaining
#[tracing::instrument]
pub fn clean_install(diskpath: &Path) -> Result<Vec<DiskOperation>> {
    let disks = rs_drivelist::drive_list().map_err(|e| eyre!(e))?;
    let disk = (disks.into_iter())
        .find(|d| (d.devicePath.as_ref()).map_or(false, |p| PathBuf::from(p) == diskpath));
    let diskobj = disk.ok_or_eyre(Report::msg("Cannot find disk"))?;
    let disk = diskpath.to_path_buf();
    let sdiskpath = diskpath.display().to_string();
    let disksize = diskobj.size / MIB;
    let orignumparts = cmd_lib::run_fun!(lsblk -o path)?
        .split('\n')
        .filter(|l| l.starts_with(&sdiskpath) && l != &sdiskpath)
        .count();

    let mut ops = vec![];

    // erase all partitions
    (1..=orignumparts).for_each(|n| instr!(ops: disk Rm format!("{n}")));

    instr!(ops:
        disk Label "gpt";
        // params: Name, FsType, Start, End (MiB)
        disk Mkpart "EFI", "fat32", 1, 513;
        disk Mkpart "Boot", "ext4", 513, 513+1024;
        disk Mkpart "LinuxRoot", "btrfs", 513+1024, disksize - 1
    );

    Ok(ops)
}

/// Resize partition wigh highest minor ID (part no.), then make /boot and root for Linux.
/// - /boot     1 GiB
/// - /     remaining
///
/// # Parameters
/// `resize` should be in MiB.
#[tracing::instrument]
pub fn dual_boot(diskpath: &Path, resize: u64) -> Result<Vec<DiskOperation>> {
    let sdiskpath = diskpath.display().to_string();
    let disk = diskpath.to_path_buf();
    let lsblk = cmd_lib::run_fun!(lsblk -bo partn,path,size)?;
    // assume all dev paths start with /
    let (partn, path_size) = (lsblk.split('\n').skip(1))
        .filter_map(|l| l.trim_start().split_once(' '))
        .filter(|(left, _)| !left.starts_with('/')) // things that start with / are path, not partn
        .filter(|(_, right)| right.starts_with(&sdiskpath))
        .last()
        .ok_or_eyre(Report::msg("lsblk has no output"))?;
    let (partpath, size) = (path_size.split_once(' '))
        .ok_or_else(|| Report::msg("Cannot split path_size(=>note)").note(path_size.to_string()))?;
    info!(partn, partpath, "Found partition to resize");
    let mut origsize: u64 = (size.trim_start().parse()).map_err(chain_err("Cannot parse size"))?;
    origsize /= MIB; // was in bytes

    let mut ops = vec![];
    if origsize != resize {
        trace!(origsize, "Parsing partnum for resize");
        instr!(ops: disk Resizepart partn, resize);
    }

    let (_, diskid) = (sdiskpath.rsplit_once('/')).ok_or_eyre(Report::msg("Cannot get disk id"))?;
    let (_, partid) = (partpath.rsplit_once('/')).ok_or_eyre(Report::msg("Cannot get part id"))?;

    let startpath = format!("/sys/block/{diskid}/{partid}/start");
    debug!(startpath, "Reading start sector pos");
    let start = std::fs::read_to_string(startpath)?;
    let mut start: u64 = (start.trim().parse()).map_err(chain_err("Cannot parse part start"))?;
    start *= 512 / MIB; // 1 unit was 512 B

    let sizepath = format!("/sys/block/{diskid}/size");
    debug!(sizepath, "Reading disk size");
    let size = std::fs::read_to_string(sizepath)?;
    let mut size: u64 = (size.trim().parse()).map_err(chain_err("Cannot parse disk size"))?;
    size *= 512 / MIB;

    info!(pstart = start, dsize = size, "Making parts for Linux");

    // NOTE: We will put EFI stuff in the already existing one
    // Somewhere else we should check that it's like at least 384 MiB (as suggested by mo)
    // -- mado
    instr!(ops:
        disk Mkpart "Boot", start+resize+1, start+resize+1025;
        disk Mkpart "LinuxRoot", start+resize+1025, size-1
    );
    Ok(ops)
}

fn chain_err<E: std::error::Error + Send + Sync + 'static>(
    msg: &'static str,
) -> impl FnOnce(E) -> Report {
    move |e| Report::msg(msg).error(e)
}
