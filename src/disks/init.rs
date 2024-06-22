use crate::util::chain_err;
use crate::{albius::DiskOperation, pages::welcome::DISTRO};
use color_eyre::{eyre::OptionExt, Report, Result, Section};
use serde_json::Value;
use std::path::Path;
use tracing::{debug, info, trace};

const MIB: u64 = 1024 * 1024;

macro_rules! instr {
    ($vec:ident:$($disk:ident $op:ident $($arg:expr),+);+) => { {
        $(
            $vec.push($crate::albius::DiskOperation {
                disk: $disk.clone(),
                operation: $crate::albius::DiskOperationType::$op,
                params: vec![$(Value::from($arg)),+],
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
    let (partns, disksize) = _get_disk_partns_disksize(diskpath)?;
    let disk = diskpath.to_path_buf();
    let mut ops = vec![];

    // erase all partitions
    info!(?partns, "Will erase partitions");
    partns
        .into_iter()
        .for_each(|n| instr!(ops: disk Rm n.to_string()));

    instr!(ops:
        disk Label "gpt";
        // params: Name, FsType, Start, End (MiB)
        disk Mkpart "EFI", "fat32", 1, 513;
        // use XFS for /boot, it's Fedora's default
        disk Mkpart "Boot", "xfs", 513, 513+1024;
        disk Mkpart DISTRO, "btrfs", 513+1024, disksize - 1
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
        .filter(|(left, right)| !left.starts_with('/') && right.starts_with(&sdiskpath)) // things that start with / are path, not partn
        .last() // last one is the one with max partn
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

    let _start_bytesize = bytesize::ByteSize::b(start);

    debug!(start, ?_start_bytesize, "Reading disk start");

    let sizepath = format!("/sys/block/{diskid}/size");
    debug!(sizepath, "Reading disk size");
    let size = std::fs::read_to_string(sizepath)?;
    let mut size: u64 = (size.trim().parse()).map_err(chain_err("Cannot parse disk size"))?;
    size *= 512 / MIB;

    let _size_bytesize = bytesize::ByteSize::b(size);

    debug!(size, ?_size_bytesize, "Reading disk size");

    info!(pstart = start, dsize = size, "Making parts for Linux");

    // NOTE: We will put EFI stuff in the already existing one
    // Somewhere else we should check that it's like at least 384 MiB (as suggested by mo)
    // -- mado
    instr!(ops:
        disk Mkpart "Boot", start+resize+1, start+resize+1025;
        disk Mkpart DISTRO, start+resize+1025, size-1
    );
    Ok(ops)
}

/// Erase the current partition table, make 3 partitions:
/// - Submarine    16 MiB
/// - /boot         1 GiB
/// - /         remaining
#[tracing::instrument]
pub fn chromebook_clean_install(diskpath: &Path) -> Result<Vec<DiskOperation>> {
    let (partns, disksize) = _get_disk_partns_disksize(diskpath)?;
    let disk = diskpath.to_path_buf();
    let mut ops = vec![];

    // erase all partitions
    info!(?partns, "Will erase partitions");
    partns.into_iter().for_each(|n| instr!(ops: disk Rm n));

    instr!(ops:
        disk Label "gpt";
        // params: Name, FsType, Start, End (MiB)
        disk Mkpart "Submarine", "fat32", 1, 17;
        disk Mkpart "Boot", "ext4", 17, 17+1024;
        disk Mkpart DISTRO, "btrfs", 17+1024, disksize - 1
    );

    Ok(ops)
}

/// Returns `disk` (path of disk), `partns` (list of part nums), `disksize` in MiB.
#[tracing::instrument]
pub fn _get_disk_partns_disksize(diskpath: &Path) -> Result<(Vec<u8>, u64)> {
    let sdiskpath = diskpath.display().to_string();
    tracing::trace!(?sdiskpath, "Disk path");
    let lsblk = cmd_lib::run_fun!(lsblk --bytes -o partn,path,size)?;
    tracing::trace!(?lsblk, "lsblk output");
    eprintln!("{}", lsblk);
    // assume all dev paths start with /
    // let iter = (lsblk.split('\n').skip(1))
    //     .filter_map(|l| l.trim_start().split_once(|ch: char| ch.is_whitespace()))
    //     .filter(|(left, right)| !left.starts_with('/') && right.starts_with(&sdiskpath)); // things that start with / are path, not partn

    let iter = (lsblk.split('\n').skip(1))
        .map(|l| l.split_whitespace())
        .filter_map(|mut l| {
            let partn = l.next()?;
            let path = l.next()?;
            let size = l.next()?;
            Some((partn, (path, size)))
        });

    tracing::debug!(?iter);

    let (partn, (partpath, size)) = iter
        .clone()
        .next()
        .ok_or_eyre(Report::msg("lsblk has no output!? Output says: (=>note)").note(lsblk.clone()))
        .unwrap();

    tracing::debug!(?partn, ?partpath, ?size, "First partn");

    // let iter = iter.filter(|())
    // Filter by partpath if starts with diskpath
    let iter = iter.filter(|(_, (path, _))| path.contains(&sdiskpath));
    let (_, path_size) = iter
        .clone()
        .next()
        .ok_or_eyre(
            Report::msg("Cannot find disk in lsblk output!? We are literally querying: (=>note)")
                .note(sdiskpath.clone()),
        )
        .unwrap();

    // tracing::debug!(?path_size, "First partn");

    let (_path, disksize) = path_size;

    let mut disksize: u64 =
        (disksize.trim_start().parse()).map_err(chain_err("Cannot parse size"))?;

    //
    let disksize_bytesize = bytesize::ByteSize::b(disksize);
    info!(?disksize_bytesize, "Disk size");
    disksize /= MIB;

    info!(?disksize, "Disk size in MiB");

    let mut errs = vec![];
    let partns = iter
        .filter_map(|(n, _)| n.parse().map_err(|e| errs.push(e)).ok())
        .collect::<Vec<u8>>();
    if !errs.is_empty() {
        return Err(errs.into_iter().fold(
            Report::msg("Cannot parse some partn values from lsblk"),
            |report, err| report.error(err),
        ));
    }
    Ok((partns, disksize))
}
