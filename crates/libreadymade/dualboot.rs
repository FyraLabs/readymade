//! Windows dual boot: finding the Windows partition, checking that it can be shrunk safely, and
//! shrinking it to make room for the new install.
//!
//! [`inspect`] only reads `lsblk` and can run unprivileged. [`probe`] needs root (ntfs-3g,
//! `ntfsresize`, mounting the ESP) and must succeed in the same process right before [`shrink`]
//! touches the disk.
//!
//! Only GPT disks with an existing ESP are supported. We never create a second ESP, reformat it or
//! touch any partition other than shrinking the Windows one.

use crate::prelude::*;
use std::{io::Write as _, process::Stdio};

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

/// Smallest root partition we are willing to create.
pub const MIN_ROOT_SIZE: u64 = 20 * GIB;
/// Size of the new XBOOTLDR `/boot` partition. Must match `templates/dualboot/45-boot.conf`.
pub const BOOT_SIZE: u64 = GIB;
/// Room left for systemd-repart to align the new partitions.
const REPART_SLACK: u64 = 16 * MIB;
/// Smallest amount of space the new install needs.
pub const MIN_INSTALL_SIZE: u64 = MIN_ROOT_SIZE + BOOT_SIZE + REPART_SLACK;
/// Partition boundaries we create are aligned to this.
pub const ALIGNMENT: u64 = MIB;
/// Free space needed on the existing ESP. shim + GRUB (+ the `EFI/BOOT` fallback) are about 8 MiB
/// on Fedora `x86_64`, so this leaves plenty of headroom for updates.
pub const ESP_MIN_FREE: u64 = 32 * MIB;
/// Space at the end of a GPT disk taken by the backup header (33 sectors), rounded up generously.
const GPT_TAIL: u64 = MIB;

const ESP_TYPE: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const BASIC_DATA_TYPE: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";
const WINDOWS_RE_TYPE: &str = "de94bba4-06d1-4d40-a16a-bfd50179d6ac";

/// Headroom we leave Windows on top of the smallest size `ntfsresize` can shrink it to, so it
/// still has room to boot and update.
#[must_use]
pub const fn windows_margin(ntfs_min: u64) -> u64 {
    let tenth = ntfs_min / 10;
    if tenth > 2 * GIB { tenth } else { 2 * GIB }
}

#[must_use]
pub const fn align_up(n: u64, align: u64) -> u64 {
    n.div_ceil(align) * align
}

#[must_use]
pub const fn align_down(n: u64, align: u64) -> u64 {
    n / align * align
}

/// Why dual booting isn't possible. The UI translates these, [`std::fmt::Display`] is for logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum DualBootError {
    #[error("the partition table is {0}, but dual boot needs GPT")]
    NotGpt(String),
    #[error("no Windows installation was found on this disk")]
    NoWindows,
    #[error("{0} is encrypted with BitLocker; suspend or decrypt BitLocker in Windows first")]
    BitLocker(PathBuf),
    #[error("{0} is mounted; unmount it first")]
    Mounted(PathBuf),
    #[error(
        "{0} is hibernated or Fast Startup is on; turn off Fast Startup and shut Windows down fully"
    )]
    Hibernated(PathBuf),
    #[error("{0} was not shut down cleanly; boot Windows and shut it down fully")]
    Unclean(PathBuf),
    #[error("{0} is marked dirty; run `chkdsk /f` in Windows first")]
    Dirty(PathBuf),
    #[error("cannot check the Windows partition {0}: {1}")]
    NtfsCheck(PathBuf, String),
    #[error("no EFI system partition was found on this disk")]
    NoEsp,
    #[error("Windows' EFI system partition {0} is not the first one on the disk")]
    MultipleEsp(PathBuf),
    #[error("the EFI system partition {0} is not FAT")]
    EspNotFat(PathBuf),
    #[error("cannot mount the EFI system partition {0}: {1}")]
    EspMount(PathBuf, String),
    #[error(
        "the EFI system partition {esp} has {free} bytes free, but {required} bytes are needed"
    )]
    EspFull {
        esp: PathBuf,
        free: u64,
        required: u64,
    },
    #[error("only {available} bytes can be freed, but at least {required} bytes are needed")]
    NotEnoughSpace { available: u64, required: u64 },
    #[error("Windows needs at least {min} bytes, but {requested} bytes were requested")]
    WindowsTooSmall { requested: u64, min: u64 },
    #[error("{0}")]
    Other(String),
}

impl DualBootError {
    /// Whether this error from [`inspect`] holds even if `lsblk` was missing information.
    #[must_use]
    pub const fn is_conclusive(&self) -> bool {
        matches!(self, Self::NotGpt(_) | Self::BitLocker(_))
    }

    fn other(e: impl std::fmt::Display) -> Self {
        Self::Other(e.to_string())
    }
}

/// A partition as seen by `lsblk`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partition {
    pub path: PathBuf,
    /// Partition number in the table
    pub number: u32,
    /// Offset from the start of the disk in bytes
    pub start: u64,
    pub size: u64,
    pub fstype: Option<String>,
    /// GPT partition type GUID, lowercase
    pub parttype: Option<String>,
}

impl Partition {
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.start + self.size
    }

    fn fstype_is(&self, fstype: &str) -> bool {
        self.fstype
            .as_deref()
            .is_some_and(|f| f.eq_ignore_ascii_case(fstype))
    }

    fn parttype_is(&self, guid: &str) -> bool {
        self.parttype.as_deref() == Some(guid)
    }

    #[must_use]
    pub fn is_bitlocker(&self) -> bool {
        self.fstype_is("BitLocker")
    }

    #[must_use]
    pub fn is_esp(&self) -> bool {
        self.parttype_is(ESP_TYPE)
    }

    /// NTFS (or `BitLocker`) partitions that may hold a Windows install
    fn may_be_windows(&self) -> bool {
        (self.fstype_is("ntfs") || self.is_bitlocker())
            && !self.parttype_is(WINDOWS_RE_TYPE)
            && !self.is_esp()
    }
}

/// The partition table of a disk, as seen by `lsblk`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskLayout {
    pub path: PathBuf,
    pub size: u64,
    /// Logical sector size, the unit `sfdisk` works in
    pub sector_size: u64,
    /// `gpt` or `dos`. `None` if `lsblk` couldn't tell (e.g. no udev database while unprivileged).
    pub pttype: Option<String>,
    /// Sorted by start offset
    pub partitions: Vec<Partition>,
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Deserialize)]
struct LsblkDevice {
    name: PathBuf,
    size: u64,
    /// In 512-byte sectors, regardless of the logical sector size
    start: Option<u64>,
    fstype: Option<String>,
    parttype: Option<String>,
    pttype: Option<String>,
    partn: Option<u32>,
    #[serde(rename = "log-sec")]
    log_sec: Option<u64>,
    #[serde(default)]
    children: Vec<Self>,
}

impl DiskLayout {
    /// Read the partition table of `disk` with `lsblk`.
    ///
    /// # Errors
    ///
    /// Returns an error if `lsblk` fails or its output can't be parsed.
    pub fn read(disk: &Path) -> Result<Self, DualBootError> {
        let out = Command::new("lsblk")
            .args(["--json", "--bytes", "--paths", "--output"])
            .arg("NAME,SIZE,START,FSTYPE,PARTTYPE,PTTYPE,PARTN,LOG-SEC")
            .arg(disk)
            .output()
            .map_err(DualBootError::other)?;
        if !out.status.success() {
            return Err(DualBootError::Other(format!(
                "lsblk failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Self::from_lsblk_json(&String::from_utf8_lossy(&out.stdout))
    }

    fn from_lsblk_json(json: &str) -> Result<Self, DualBootError> {
        let out: LsblkOutput = serde_json::from_str(json).map_err(DualBootError::other)?;
        let disk = out
            .blockdevices
            .into_iter()
            .next()
            .ok_or_else(|| DualBootError::other("lsblk returned no devices"))?;
        let mut partitions = disk
            .children
            .iter()
            .map(|part| {
                Ok(Partition {
                    path: part.name.clone(),
                    number: part.partn.ok_or_else(|| {
                        DualBootError::Other(format!(
                            "no partition number for {}",
                            part.name.display()
                        ))
                    })?,
                    start: part.start.ok_or_else(|| {
                        DualBootError::Other(format!("no start offset for {}", part.name.display()))
                    })? * 512,
                    size: part.size,
                    fstype: part.fstype.clone(),
                    parttype: part.parttype.as_deref().map(str::to_ascii_lowercase),
                })
            })
            .collect::<Result<Vec<_>, DualBootError>>()?;
        partitions.sort_by_key(|p| p.start);
        Ok(Self {
            // the disk's own PTTYPE is sometimes empty while its partitions have it
            pttype: disk
                .pttype
                .clone()
                .or_else(|| disk.children.iter().find_map(|c| c.pttype.clone())),
            path: disk.name,
            size: disk.size,
            sector_size: disk.log_sec.unwrap_or(512),
            partitions,
        })
    }

    fn partition(&self, path: &Path) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.path == path)
    }

    /// Free space right after `part`, up to the next partition or the end of the usable area.
    #[must_use]
    pub fn free_after(&self, part: &Partition) -> u64 {
        let limit = self
            .partitions
            .iter()
            .map(|p| p.start)
            .filter(|&start| start >= part.end())
            .min()
            .unwrap_or_else(|| self.size.saturating_sub(GPT_TAIL));
        limit.saturating_sub(part.end())
    }

    /// Pick the Windows partition to shrink.
    ///
    /// `hints` are the partitions os-prober attributed to Windows. On UEFI that's usually the ESP
    /// holding `bootmgfw.efi` rather than the NTFS partition itself, in which case every NTFS
    /// partition on the disk is a candidate. The largest one wins.
    ///
    /// # Errors
    ///
    /// Returns [`DualBootError::NoWindows`] if os-prober didn't find Windows on this disk.
    pub fn find_windows(&self, hints: &[PathBuf]) -> Result<&Partition, DualBootError> {
        let hinted = hints.iter().filter_map(|h| self.partition(h)).collect_vec();
        if hinted.is_empty() {
            return Err(DualBootError::NoWindows);
        }
        let direct = hinted.iter().copied().filter(|p| p.may_be_windows());
        let candidates = if direct.clone().next().is_some() {
            direct.collect_vec()
        } else {
            self.partitions
                .iter()
                .filter(|p| p.may_be_windows())
                .filter(|p| p.parttype.is_none() || p.parttype_is(BASIC_DATA_TYPE))
                .collect_vec()
        };
        candidates
            .into_iter()
            .max_by_key(|p| p.size)
            .ok_or(DualBootError::NoWindows)
    }

    /// The ESP we will share with Windows.
    ///
    /// systemd-repart adopts the first ESP in the partition table, so if os-prober found Windows'
    /// boot manager on a different one, refuse rather than install to the wrong ESP.
    ///
    /// # Errors
    ///
    /// Returns an error if there is no ESP, or Windows uses one that isn't the first.
    pub fn find_esp(&self, hints: &[PathBuf]) -> Result<&Partition, DualBootError> {
        let mut esps = self.partitions.iter().filter(|p| p.is_esp()).collect_vec();
        esps.sort_by_key(|p| p.number);
        let first = *esps.first().ok_or(DualBootError::NoEsp)?;
        if let Some(other) = esps
            .iter()
            .skip(1)
            .find(|esp| hints.contains(&esp.path) && !hints.contains(&first.path))
        {
            return Err(DualBootError::MultipleEsp(other.path.clone()));
        }
        Ok(first)
    }
}

/// What [`inspect`] can find out without root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inspection {
    pub layout: DiskLayout,
    pub windows: Partition,
    pub esp: Partition,
}

/// Find the Windows and EFI system partitions on `disk` and run the checks that don't need root.
///
/// `hints` are the partitions os-prober attributed to Windows, see [`DiskLayout::find_windows`].
///
/// # Errors
///
/// Returns why dual booting is impossible. Unprivileged `lsblk` may not know filesystem types if
/// the udev database isn't readable, so only [`DualBootError::NotGpt`] and
/// [`DualBootError::BitLocker`] are conclusive without root; see [`DualBootError::is_conclusive`].
pub fn inspect(disk: &Path, hints: &[PathBuf]) -> Result<Inspection, DualBootError> {
    let layout = DiskLayout::read(disk)?;
    tracing::debug!(?layout, ?hints, "Inspecting disk for dual boot");
    inspect_layout(layout, hints)
}

fn inspect_layout(layout: DiskLayout, hints: &[PathBuf]) -> Result<Inspection, DualBootError> {
    if let Some(pttype) = layout.pttype.as_deref().filter(|&t| t != "gpt") {
        return Err(DualBootError::NotGpt(pttype.to_owned()));
    }
    let windows = layout.find_windows(hints)?.clone();
    if windows.is_bitlocker() {
        return Err(DualBootError::BitLocker(windows.path));
    }
    let esp = layout.find_esp(hints)?.clone();
    if !(esp.fstype_is("vfat") || esp.fstype.is_none()) {
        return Err(DualBootError::EspNotFat(esp.path));
    }
    if layout.pttype.is_none() {
        return Err(DualBootError::Other(
            "cannot read the partition table type".into(),
        ));
    }
    Ok(Inspection {
        layout,
        windows,
        esp,
    })
}

/// Everything the UI needs to offer a split, and the playbook needs to carry it out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DualBootInfo {
    pub windows_partition: PathBuf,
    pub esp_partition: PathBuf,
    /// Start of the Windows partition in bytes
    pub windows_start: u64,
    /// Current size of the Windows partition
    pub windows_size: u64,
    /// Smallest size we allow Windows to be shrunk to, margin included
    pub windows_min: u64,
    /// Size of the Windows partition plus the free space right after it
    pub total: u64,
    /// Size of the ESP, so repart can be told not to grow it
    pub esp_size: u64,
}

impl DualBootInfo {
    /// Largest size Windows can keep while leaving room for us. If there already is enough free
    /// space after Windows, that's its current size, i.e. no shrinking.
    #[must_use]
    pub const fn windows_max(&self) -> u64 {
        let max = align_down(
            self.windows_start + self.total.saturating_sub(MIN_INSTALL_SIZE),
            ALIGNMENT,
        )
        .saturating_sub(self.windows_start);
        if max < self.windows_size {
            max
        } else {
            self.windows_size
        }
    }

    /// Whether there's room for both Windows and us at all.
    ///
    /// # Errors
    ///
    /// Returns [`DualBootError::NotEnoughSpace`] if not.
    pub const fn check_space(&self) -> Result<(), DualBootError> {
        if self.windows_max() < self.windows_min {
            return Err(DualBootError::NotEnoughSpace {
                available: self.total.saturating_sub(self.windows_min),
                required: MIN_INSTALL_SIZE,
            });
        }
        Ok(())
    }

    /// Turn the size the user picked for Windows into the new partition size: the end is aligned
    /// up to [`ALIGNMENT`] (never giving Windows less than requested), then checked against the
    /// limits. Returns the current size if Windows doesn't need to shrink.
    ///
    /// # Errors
    ///
    /// Returns an error if Windows would get too small, or we wouldn't have enough space.
    pub const fn plan(&self, requested: u64) -> Result<u64, DualBootError> {
        let size = if requested >= self.windows_size {
            self.windows_size
        } else {
            align_up(self.windows_start + requested, ALIGNMENT) - self.windows_start
        };
        if size < self.windows_min {
            return Err(DualBootError::WindowsTooSmall {
                requested,
                min: self.windows_min,
            });
        }
        let ours = self.total.saturating_sub(size);
        if ours < MIN_INSTALL_SIZE {
            return Err(DualBootError::NotEnoughSpace {
                available: ours,
                required: MIN_INSTALL_SIZE,
            });
        }
        Ok(size)
    }

    /// Size cap for the new root partition once Windows is `windows_size` bytes.
    #[must_use]
    pub const fn root_size(&self, windows_size: u64) -> u64 {
        self.total
            .saturating_sub(windows_size)
            .saturating_sub(BOOT_SIZE + REPART_SLACK)
    }
}

/// Parsed `ntfsresize --info` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtfsInfo {
    /// Smallest size the volume can be shrunk to, in bytes
    pub min_size: u64,
    pub volume_size: Option<u64>,
}

/// Parse `ntfsresize --info --no-action` output.
///
/// # Errors
///
/// Returns an error if the output says the volume is dirty, or has no minimum size in it.
pub fn parse_ntfsresize_info(partition: &Path, output: &str) -> Result<NtfsInfo, DualBootError> {
    if output.contains("Volume is scheduled for check") {
        return Err(DualBootError::Dirty(partition.to_owned()));
    }
    if output.contains("hibernated") || output.contains("Windows cache") {
        return Err(DualBootError::Hibernated(partition.to_owned()));
    }
    let bytes_after = |prefix: &str| {
        output.lines().find_map(|line| {
            let rest = line.trim().strip_prefix(prefix)?;
            rest.trim_start_matches([' ', ':'])
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
    };
    let min_size = bytes_after("You might resize at").ok_or_else(|| {
        DualBootError::NtfsCheck(
            partition.to_owned(),
            output
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("no output from ntfsresize")
                .trim()
                .to_owned(),
        )
    })?;
    Ok(NtfsInfo {
        min_size,
        volume_size: bytes_after("Current volume size"),
    })
}

/// Map `ntfs-3g.probe` exit codes, see `ntfs-3g.probe(8)`.
pub(crate) fn ntfs_probe_result(partition: &Path, code: Option<i32>) -> Result<(), DualBootError> {
    let part = partition.to_owned();
    match code {
        Some(0) => Ok(()),
        Some(14) => Err(DualBootError::Hibernated(part)),
        Some(15) => Err(DualBootError::Unclean(part)),
        Some(16) => Err(DualBootError::Mounted(part)),
        Some(12) => Err(DualBootError::NtfsCheck(part, "not an NTFS volume".into())),
        Some(13) => Err(DualBootError::NtfsCheck(
            part,
            "the volume is corrupt".into(),
        )),
        code => Err(DualBootError::NtfsCheck(
            part,
            format!("ntfs-3g.probe exited with {code:?}"),
        )),
    }
}

fn mountpoint_of(dev: &Path) -> Option<PathBuf> {
    let dev = std::fs::canonicalize(dev).unwrap_or_else(|_| dev.to_owned());
    lsblk::Mount::list()
        .ok()?
        .find(|m| std::fs::canonicalize(&m.device).is_ok_and(|d| d == dev))
        .map(|m| m.mountpoint)
}

/// The first 4 KiB of `hiberfil.sys` on `partition`, empty if there is none.
pub(crate) fn hiberfile_header(partition: &Path) -> Result<Vec<u8>, DualBootError> {
    use std::io::Read as _;
    let err =
        |e: &dyn std::fmt::Display| DualBootError::NtfsCheck(partition.to_owned(), e.to_string());
    let mut child = Command::new("ntfscat")
        .arg(partition)
        .arg("hiberfil.sys")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| err(&e))?;
    let mut header = Vec::with_capacity(4096);
    child
        .stdout
        .take()
        .ok_or_else(|| err(&"cannot read ntfscat output"))?
        .take(4096)
        .read_to_end(&mut header)
        .map_err(|e| err(&e))?;
    // hiberfil.sys can be many GB, we're done with it
    _ = child.kill();
    let out = child.wait_with_output().map_err(|e| err(&e))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    tracing::info!(?out.status, %stderr, len = header.len(), "ntfscat hiberfil.sys");
    if header.is_empty() && !out.status.success() && !stderr.contains("No such file") {
        return Err(err(&stderr.trim()));
    }
    Ok(header)
}

/// Whether a `hiberfil.sys` header says Windows is hibernated. Fast Startup hibernates the kernel
/// session, so this covers it too. Same check as ntfs-3g's `ntfs_volume_check_hiberfile`.
#[must_use]
pub fn is_hibernated(header: &[u8]) -> bool {
    header
        .get(..4)
        .is_some_and(|magic| magic.eq_ignore_ascii_case(b"hibr"))
}

/// Check the Windows partition can be resized safely and find its minimum size. Needs root.
///
/// # Errors
///
/// Returns why it can't be resized.
pub fn check_ntfs(partition: &Path) -> Result<NtfsInfo, DualBootError> {
    if mountpoint_of(partition).is_some() {
        return Err(DualBootError::Mounted(partition.to_owned()));
    }
    // Everything here is read-only: nothing may be written to Windows' partition before the user
    // confirmed and every check passed.
    let probe = Command::new("ntfs-3g.probe")
        .arg("--readonly")
        .arg(partition)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| DualBootError::NtfsCheck(partition.to_owned(), e.to_string()))?;
    tracing::info!(
        code = ?probe.status.code(),
        stderr = %String::from_utf8_lossy(&probe.stderr),
        "ntfs-3g.probe"
    );
    ntfs_probe_result(partition, probe.status.code())?;

    // ntfsresize opens the volume read-only for --info and therefore never looks at hiberfil.sys,
    // so check it the same way ntfs-3g does for read-write mounts. (ntfsresize does check it for
    // the actual resize, so it would still refuse then.)
    if is_hibernated(&hiberfile_header(partition)?) {
        return Err(DualBootError::Hibernated(partition.to_owned()));
    }

    // No --force: we want ntfsresize to refuse dirty volumes.
    let info = Command::new("ntfsresize")
        .args(["--info", "--no-action", "--no-progress-bar"])
        .arg(partition)
        .output()
        .map_err(|e| DualBootError::NtfsCheck(partition.to_owned(), e.to_string()))?;
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&info.stdout),
        String::from_utf8_lossy(&info.stderr)
    );
    tracing::info!(code = ?info.status.code(), %output, "ntfsresize --info");
    let parsed = parse_ntfsresize_info(partition, &output)?;
    if !info.status.success() {
        return Err(DualBootError::NtfsCheck(partition.to_owned(), output));
    }
    Ok(parsed)
}

/// How much space is free on the ESP. Mounts it read-only if it isn't mounted yet. Needs root.
///
/// # Errors
///
/// Returns an error if it can't be mounted as FAT.
pub fn esp_free_space(esp: &Path) -> Result<u64, DualBootError> {
    let statvfs = |path: &Path| {
        nix::sys::statvfs::statvfs(path)
            .map(|s| s.blocks_available() * s.fragment_size())
            .map_err(|e| DualBootError::EspMount(esp.to_owned(), e.to_string()))
    };
    if let Some(mountpoint) = mountpoint_of(esp) {
        return statvfs(&mountpoint);
    }
    let tmp = tempfile::tempdir().map_err(DualBootError::other)?;
    let mount = sys_mount::Mount::builder()
        .fstype("vfat")
        .flags(sys_mount::MountFlags::RDONLY)
        .mount_autodrop(esp, tmp.path(), sys_mount::UnmountFlags::DETACH)
        .map_err(|e| DualBootError::EspMount(esp.to_owned(), e.to_string()))?;
    let free = statvfs(tmp.path());
    drop(mount);
    free
}

/// Run every check, including the ones needing root, and work out the limits for the split.
///
/// `esp_required` is how much free space we need on the ESP, at least [`ESP_MIN_FREE`].
///
/// # Errors
///
/// Returns why dual booting is impossible.
pub fn probe(
    disk: &Path,
    hints: &[PathBuf],
    esp_required: u64,
) -> Result<DualBootInfo, DualBootError> {
    let Inspection {
        layout,
        windows,
        esp,
    } = inspect(disk, hints)?;
    tracing::info!(?windows, ?esp, "Found Windows and ESP");

    let free = esp_free_space(&esp.path)?;
    let required = esp_required.max(ESP_MIN_FREE);
    tracing::info!(free, required, "ESP free space");
    if free < required {
        return Err(DualBootError::EspFull {
            esp: esp.path,
            free,
            required,
        });
    }

    let ntfs = check_ntfs(&windows.path)?;
    let info = DualBootInfo {
        windows_min: align_up(ntfs.min_size + windows_margin(ntfs.min_size), ALIGNMENT)
            .min(windows.size),
        total: windows.size + layout.free_after(&windows),
        windows_partition: windows.path,
        esp_partition: esp.path,
        windows_start: windows.start,
        windows_size: windows.size,
        esp_size: esp.size,
    };
    tracing::info!(?info, ?ntfs, "Dual boot probe done");
    info.check_space()?;
    Ok(info)
}

fn run_logged(cmd: &mut Command, stdin: Option<&str>) -> Result<String> {
    tracing::info!(?cmd, "Running");
    let mut child = cmd
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err_with(|| format!("cannot run {cmd:?}"))?;
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .ok_or_eyre("cannot open stdin")?
            .write_all(input.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    tracing::info!(status = ?out.status.code(), %stdout, %stderr, "Finished {cmd:?}");
    if !out.status.success() {
        bail!("{cmd:?} failed with {:?}: {stderr}", out.status.code());
    }
    Ok(stdout)
}

/// `sfdisk` script line that keeps the partition's start and sets its new size, in sectors.
pub(crate) fn sfdisk_resize_line(start: u64, new_size: u64, sector_size: u64) -> Result<String> {
    if !start.is_multiple_of(sector_size) || !new_size.is_multiple_of(sector_size) {
        bail!("partition boundaries are not sector-aligned ({start}, {new_size}, {sector_size})");
    }
    Ok(format!(
        "{},{}\n",
        start / sector_size,
        new_size / sector_size
    ))
}

/// Shrink the Windows partition to `new_size` bytes: the NTFS filesystem, then the partition.
///
/// The partition keeps its start. Must only be called after [`probe`] succeeded and `new_size`
/// came from [`DualBootInfo::plan`].
///
/// With `dry_run`, only `ntfsresize --no-action` runs.
///
/// # Errors
///
/// Returns an error if any step fails. If the filesystem was shrunk but the partition wasn't,
/// Windows still works; it just has unused space at the end of its partition.
pub fn shrink(disk: &Path, info: &DualBootInfo, new_size: u64, dry_run: bool) -> Result<()> {
    let disk = DiskLayout::read(disk).map_err(|e| eyre!("{e}"))?;
    let part = disk
        .partitions
        .iter()
        .find(|p| p.path == info.windows_partition)
        .ok_or_eyre("Windows partition disappeared")?;
    if part.start != info.windows_start || part.size != info.windows_size {
        bail!("the Windows partition changed since it was checked");
    }
    if new_size == part.size {
        tracing::info!("Enough free space after Windows already, not resizing it");
        return Ok(());
    }
    if new_size > part.size {
        bail!(
            "refusing to grow the Windows partition ({} -> {new_size})",
            part.size
        );
    }
    let line = sfdisk_resize_line(part.start, new_size, disk.sector_size)?;
    let size = new_size.to_string();

    // keep a copy of the partition table in the logs, it can be fed back to sfdisk
    let dump = run_logged(Command::new("sfdisk").arg("--dump").arg(&disk.path), None)?;
    tracing::info!(%dump, "Partition table before shrinking Windows");

    run_logged(
        Command::new("ntfsresize")
            .args(["--no-action", "--no-progress-bar", "--size", &size])
            .arg(&part.path),
        None,
    )?;
    if dry_run {
        tracing::warn!("Dry run, not resizing {:?} to {new_size} bytes", part.path);
        return Ok(());
    }

    // Filesystem first, then the partition. ntfsresize asks for confirmation on stdin.
    run_logged(
        Command::new("ntfsresize")
            .args(["--no-progress-bar", "--size", &size])
            .arg(&part.path),
        Some("y\n"),
    )?;
    run_logged(
        Command::new("sfdisk")
            .args([
                "--no-reread",
                "--wipe",
                "never",
                "--wipe-partitions",
                "never",
            ])
            .args(["-N", &part.number.to_string()])
            .arg(&disk.path),
        Some(&line),
    )?;
    run_logged(Command::new("partprobe").arg(&disk.path), None)?;
    run_logged(Command::new("udevadm").arg("settle"), None)?;

    let after = DiskLayout::read(&disk.path).map_err(|e| eyre!("{e}"))?;
    let part_after = after
        .partitions
        .iter()
        .find(|p| p.number == part.number)
        .ok_or_eyre("Windows partition disappeared after resizing")?;
    if part_after.start != part.start || part_after.size != new_size {
        bail!("Windows partition is {part_after:?} after resizing, expected size {new_size}");
    }
    tracing::info!(?part_after, "Windows partition shrunk");
    Ok(())
}

/// Copy our EFI binaries (`EFI/fedora` etc.) from `src` into `dest`, the ESP's `EFI` directory.
///
/// `EFI/Microsoft` is never touched, and the `EFI/BOOT` fallback loader is only added if Windows
/// didn't already put one there.
///
/// # Errors
///
/// Returns an error if copying fails.
pub fn copy_efi_files(src: &Path, dest: &Path) -> Result<()> {
    fn copy_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
        if src.is_dir() {
            std::fs::create_dir_all(dest)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                copy_recursive(&entry.path(), &dest.join(entry.file_name()))?;
            }
        } else {
            std::fs::copy(src, dest)?;
        }
        Ok(())
    }
    // FAT is case-insensitive, `EFI/Boot` and `EFI/BOOT` are the same thing
    let exists = |name: &std::ffi::OsStr| {
        std::fs::read_dir(dest).is_ok_and(|mut entries| {
            entries.any(|e| e.is_ok_and(|e| e.file_name().eq_ignore_ascii_case(name)))
        })
    };
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.eq_ignore_ascii_case("Microsoft") {
            continue;
        }
        if name.eq_ignore_ascii_case("BOOT") && exists(&name) {
            tracing::info!("Keeping the existing EFI/BOOT fallback loader");
            continue;
        }
        tracing::info!(?name, "Copying EFI files to the ESP");
        copy_recursive(&entry.path(), &dest.join(&name))?;
    }
    Ok(())
}

/// Total size of the regular files under `dir`, skipping `EFI/Microsoft`.
#[must_use]
pub fn efi_payload_size(dir: &Path) -> u64 {
    fn walk(dir: &Path) -> u64 {
        std::fs::read_dir(dir).map_or(0, |entries| {
            entries
                .flatten()
                .filter(|e| !e.file_name().eq_ignore_ascii_case("Microsoft"))
                .map(|e| match e.file_type() {
                    Ok(t) if t.is_dir() => walk(&e.path()),
                    Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
                    _ => 0,
                })
                .sum()
        })
    }
    walk(dir)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    const NTFSRESIZE_INFO: &str = "\
ntfsresize v2026.2.25 (libntfs-3g)
Device name        : /dev/vda3
NTFS volume version: 3.1
Cluster size       : 4096 bytes
Current volume size: 6442447360 bytes (6443 MB)
Current device size: 6442450944 bytes (6443 MB)
Checking filesystem consistency ...
100.00 percent completed
Accounting clusters ...
Space in use       : 33 MB (0.5%)
Collecting resizing constraints ...
You might resize at 32870400 bytes or 33 MB (freeing 6410 MB).
Please make a test run using both the -n and -s options before real resizing!
";

    const NTFSRESIZE_DIRTY: &str = "\
ntfsresize v2026.2.25 (libntfs-3g)
Device name        : /dev/vda3
NTFS volume version: 3.1
Cluster size       : 4096 bytes
Current volume size: 6442447360 bytes (6443 MB)
Current device size: 6442450944 bytes (6443 MB)
Volume is scheduled for check.
Run chkdsk /f and please try again, or see option -f.
";

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn parses_ntfsresize_info() {
        let info = parse_ntfsresize_info(&p("/dev/vda3"), NTFSRESIZE_INFO).unwrap();
        assert_eq!(info.min_size, 32_870_400);
        assert_eq!(info.volume_size, Some(6_442_447_360));
    }

    #[test]
    fn ntfsresize_dirty() {
        assert_eq!(
            parse_ntfsresize_info(&p("/dev/vda3"), NTFSRESIZE_DIRTY),
            Err(DualBootError::Dirty(p("/dev/vda3")))
        );
    }

    #[test]
    fn ntfsresize_garbage() {
        let err = parse_ntfsresize_info(&p("/dev/vda3"), "ERROR: NTFS signature is missing.\n");
        assert_eq!(
            err,
            Err(DualBootError::NtfsCheck(
                p("/dev/vda3"),
                "ERROR: NTFS signature is missing.".into()
            ))
        );
    }

    #[test]
    fn hibernation() {
        assert!(is_hibernated(b"HIBR\0\0\0\0"));
        assert!(is_hibernated(b"hibr"));
        assert!(!is_hibernated(b"wake\0\0\0\0"));
        assert!(!is_hibernated(&[0; 4096]));
        // no hiberfil.sys at all
        assert!(!is_hibernated(&[]));
    }

    #[test]
    fn ntfs_probe_codes() {
        let part = p("/dev/vda3");
        assert_eq!(ntfs_probe_result(&part, Some(0)), Ok(()));
        assert_eq!(
            ntfs_probe_result(&part, Some(14)),
            Err(DualBootError::Hibernated(part.clone()))
        );
        assert_eq!(
            ntfs_probe_result(&part, Some(15)),
            Err(DualBootError::Unclean(part.clone()))
        );
        assert!(ntfs_probe_result(&part, None).is_err());
    }

    /// ESP, MSR, Windows, recovery, like a stock Windows 11 install, plus some free space after
    /// Windows, in the shape of `lsblk --json --bytes --paths` output.
    const LSBLK: &str = r#"{
   "blockdevices": [
      {"name":"/dev/loop0", "size":68719476736, "start":null, "fstype":null, "parttype":null, "pttype":"gpt", "partn":null, "log-sec":512,
         "children": [
            {"name":"/dev/loop0p1", "size":104857600, "start":2048, "fstype":"vfat", "parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "pttype":"gpt", "partn":1, "log-sec":512},
            {"name":"/dev/loop0p2", "size":16777216, "start":206848, "fstype":null, "parttype":"e3c9e316-0b5c-4db8-817d-f92df00215ae", "pttype":"gpt", "partn":2, "log-sec":512},
            {"name":"/dev/loop0p4", "size":629145600, "start":125829120, "fstype":"ntfs", "parttype":"de94bba4-06d1-4d40-a16a-bfd50179d6ac", "pttype":"gpt", "partn":4, "log-sec":512},
            {"name":"/dev/loop0p3", "size":53687091200, "start":239616, "fstype":"ntfs", "parttype":"EBD0A0A2-B9E5-4433-87C0-68B6B72699C7", "pttype":"gpt", "partn":3, "log-sec":512}
         ]
      }
   ]
}"#;

    fn layout() -> DiskLayout {
        DiskLayout::from_lsblk_json(LSBLK).unwrap()
    }

    #[test]
    fn parses_lsblk() {
        let layout = layout();
        assert_eq!(layout.pttype.as_deref(), Some("gpt"));
        assert_eq!(layout.sector_size, 512);
        // sorted by start, not by lsblk order
        assert_eq!(
            layout.partitions.iter().map(|p| p.number).collect_vec(),
            [1, 2, 3, 4]
        );
        let win = &layout.partitions[2];
        assert_eq!(win.start, 239_616 * 512);
        assert_eq!(win.parttype.as_deref(), Some(BASIC_DATA_TYPE));
    }

    #[test]
    fn finds_windows_from_efi_hint() {
        let layout = layout();
        // os-prober on UEFI points at the ESP (with the `@/EFI/...` suffix stripped)
        let win = layout.find_windows(&[p("/dev/loop0p1")]).unwrap();
        assert_eq!(win.path, p("/dev/loop0p3"));
        assert_eq!(
            layout.find_esp(&[p("/dev/loop0p1")]).unwrap().path,
            p("/dev/loop0p1")
        );
    }

    #[test]
    fn finds_windows_from_direct_hint() {
        let layout = layout();
        let win = layout.find_windows(&[p("/dev/loop0p3")]).unwrap();
        assert_eq!(win.path, p("/dev/loop0p3"));
        // hints on other disks don't count
        assert_eq!(
            layout.find_windows(&[p("/dev/sda1")]),
            Err(DualBootError::NoWindows)
        );
    }

    #[test]
    fn free_space_after_windows() {
        let layout = layout();
        let win = &layout.partitions[2];
        // up to the recovery partition
        assert_eq!(layout.free_after(win), 125_829_120 * 512 - win.end());
        // the recovery partition runs up to the end of the disk minus the backup GPT
        let re = &layout.partitions[3];
        assert_eq!(layout.free_after(re), 68_719_476_736 - GPT_TAIL - re.end());
    }

    #[test]
    fn refuses_bitlocker_and_mbr() {
        let mut layout = layout();
        layout.partitions[2].fstype = Some("BitLocker".into());
        assert_eq!(
            inspect_layout(layout.clone(), &[p("/dev/loop0p1")]),
            Err(DualBootError::BitLocker(p("/dev/loop0p3")))
        );
        layout.pttype = Some("dos".into());
        let err = inspect_layout(layout, &[p("/dev/loop0p3")]).unwrap_err();
        assert_eq!(err, DualBootError::NotGpt("dos".into()));
        assert!(err.is_conclusive());
    }

    #[test]
    fn refuses_missing_or_non_fat_esp() {
        let mut layout = layout();
        layout.partitions[0].fstype = Some("ext4".into());
        assert_eq!(
            inspect_layout(layout.clone(), &[p("/dev/loop0p3")]),
            Err(DualBootError::EspNotFat(p("/dev/loop0p1")))
        );
        layout.partitions.remove(0);
        assert_eq!(
            inspect_layout(layout, &[p("/dev/loop0p3")]),
            Err(DualBootError::NoEsp)
        );
    }

    #[test]
    fn refuses_windows_on_second_esp() {
        let mut layout = layout();
        let mut esp2 = layout.partitions[0].clone();
        esp2.path = p("/dev/loop0p5");
        esp2.number = 5;
        esp2.start = 60 * GIB;
        layout.partitions.push(esp2);
        assert_eq!(
            layout.find_esp(&[p("/dev/loop0p5")]),
            Err(DualBootError::MultipleEsp(p("/dev/loop0p5")))
        );
    }

    fn info() -> DualBootInfo {
        DualBootInfo {
            windows_partition: p("/dev/loop0p3"),
            esp_partition: p("/dev/loop0p1"),
            windows_start: 239_616 * 512,
            windows_size: 100 * GIB,
            windows_min: 30 * GIB,
            total: 100 * GIB + 5 * MIB,
            esp_size: 100 * MIB,
        }
    }

    #[test]
    fn margin() {
        assert_eq!(windows_margin(GIB), 2 * GIB);
        assert_eq!(windows_margin(100 * GIB), 10 * GIB);
    }

    #[test]
    fn alignment() {
        assert_eq!(align_up(1, MIB), MIB);
        assert_eq!(align_up(MIB, MIB), MIB);
        assert_eq!(align_down(MIB + 1, MIB), MIB);
    }

    #[test]
    fn plan_aligns_end_up() {
        let info = info();
        let size = info.plan(50 * GIB + 123).unwrap();
        assert!(size >= 50 * GIB + 123);
        assert!(size < 50 * GIB + 123 + ALIGNMENT);
        assert_eq!((info.windows_start + size) % ALIGNMENT, 0);
        assert!(info.root_size(size) >= MIN_ROOT_SIZE);
        assert_eq!(
            info.root_size(size) + size + BOOT_SIZE + REPART_SLACK,
            info.total
        );
    }

    #[test]
    fn plan_limits() {
        let info = info();
        assert!(matches!(
            info.plan(29 * GIB),
            Err(DualBootError::WindowsTooSmall { .. })
        ));
        assert!(matches!(
            info.plan(90 * GIB),
            Err(DualBootError::NotEnoughSpace { .. })
        ));
        info.plan(info.windows_min).unwrap();
        info.plan(info.windows_max()).unwrap();
        info.plan(info.windows_max() + ALIGNMENT).unwrap_err();
        info.check_space().unwrap();
        // asking for more than Windows has means leaving it alone, if there's room for us
        let roomy = DualBootInfo {
            total: info.windows_size + 50 * GIB,
            ..info
        };
        assert_eq!(roomy.windows_max(), roomy.windows_size);
        assert_eq!(roomy.plan(200 * GIB), Ok(roomy.windows_size));
    }

    #[test]
    fn not_enough_space() {
        let info = DualBootInfo {
            windows_min: 90 * GIB,
            ..info()
        };
        assert!(matches!(
            info.check_space(),
            Err(DualBootError::NotEnoughSpace { .. })
        ));
    }

    #[test]
    fn sfdisk_line() {
        assert_eq!(
            sfdisk_resize_line(239_616 * 512, 4 * GIB, 512).unwrap(),
            "239616,8388608\n"
        );
        assert_eq!(
            sfdisk_resize_line(MIB, 4 * GIB, 4096).unwrap(),
            "256,1048576\n"
        );
        let err = sfdisk_resize_line(MIB, 4 * GIB + 512, 4096).unwrap_err();
        assert!(err.to_string().contains("not sector-aligned"));
    }

    #[test]
    fn efi_copy_keeps_windows_files() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        for f in [
            "fedora/shimx64.efi",
            "BOOT/BOOTX64.EFI",
            "Microsoft/evil.efi",
        ] {
            let f = src.path().join(f);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, "ours").unwrap();
        }
        let bootmgfw = dest.path().join("Microsoft/Boot/bootmgfw.efi");
        std::fs::create_dir_all(bootmgfw.parent().unwrap()).unwrap();
        std::fs::write(&bootmgfw, "windows").unwrap();
        std::fs::create_dir_all(dest.path().join("Boot")).unwrap();
        std::fs::write(dest.path().join("Boot/bootx64.efi"), "windows").unwrap();

        assert_eq!(efi_payload_size(src.path()), 8);
        copy_efi_files(src.path(), dest.path()).unwrap();

        assert_eq!(std::fs::read_to_string(&bootmgfw).unwrap(), "windows");
        assert!(!dest.path().join("Microsoft/evil.efi").exists());
        assert!(!dest.path().join("BOOT").exists());
        assert_eq!(
            std::fs::read_to_string(dest.path().join("Boot/bootx64.efi")).unwrap(),
            "windows"
        );
        assert_eq!(
            std::fs::read_to_string(dest.path().join("fedora/shimx64.efi")).unwrap(),
            "ours"
        );
    }
}
