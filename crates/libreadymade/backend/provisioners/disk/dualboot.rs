use repart::Output;
use serde_systemd_unit::Value;

use crate::{
    backend::provisioners::disk::{
        DiskProvisionerModule,
        repart::{RepartArgs, SystemdRepartData, dry_run, mounts_from_output, systemd_repart},
    },
    dualboot::{self, DualBootInfo, MIN_ROOT_SIZE},
    prelude::*,
    stage,
};

/// Extra room on the ESP on top of the EFI files we copy, for grub.cfg, grubenv and updates.
const ESP_HEADROOM: u64 = 8 << 20;

/// Install next to Windows: shrink its NTFS partition, then let systemd-repart fill the freed space
/// (`--empty=allow`), adopting Windows' ESP instead of creating one.
///
/// Every check from [`dualboot::probe`] runs again here, as root, right before the disk is touched.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DualBoot {
    /// Directory with the repart definitions, see `templates/dualboot`. They get layered into a
    /// temporary copy with the sizes filled in, see [`layer_templates`].
    pub directory: PathBuf,
    pub copy_source: Option<PathBuf>,
    /// The NTFS partition to shrink
    pub windows_partition: PathBuf,
    /// The size the user picked for the Windows partition, in bytes
    pub windows_size: u64,
}

/// What to change in the repart definitions for one install, see [`layer_templates`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateLayers {
    /// Pin the adopted ESP to its current size so repart never grows it
    pub esp_size: Option<u64>,
    /// Exact size of the XBOOTLDR partition
    pub boot_size: Option<u64>,
    /// Cap the root partition at this size
    pub root_size: Option<u64>,
    /// Encrypt the root partition with the key file, plus TPM if `true`
    pub encrypt: Option<bool>,
    /// Keep `CopyFiles=` (only meaningful with `--copy-source`, otherwise it would copy the host)
    pub copy_files: bool,
}

/// Copy the repart definitions in `src` to `dest`, applying `layers` to them, so the templates
/// themselves stay untouched. Definitions are recognized by their `Type=`.
///
/// # Errors
///
/// Returns an error if a definition can't be read, parsed or written.
pub fn layer_templates(src: &Path, dest: &Path, layers: &TemplateLayers) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let mut ini = serde_systemd_unit::parse(&std::fs::read_to_string(&path)?)
            .map_err(|e| eyre!("cannot parse {path:?}: {e:?}"))?;
        let part = ini.sections.get_mut("Partition")
            .ok_or_else(|| eyre!("{path:?} has no [Partition] section"))?;
        let ty = part.get("Type").map(|v| v.as_str().to_owned());
        let mut set = |k: &str, v: String| _ = part.insert(k.to_owned(), Value::String(v));
        match ty.as_deref() {
            Some("esp") => {
                if let Some(size) = layers.esp_size {
                    set("SizeMinBytes", size.to_string());
                    set("SizeMaxBytes", size.to_string());
                }
            }
            Some("xbootldr") => {
                if let Some(size) = layers.boot_size {
                    set("SizeMinBytes", size.to_string());
                    set("SizeMaxBytes", size.to_string());
                }
            }
            Some(ty) if ty == "root" || ty.starts_with("root-") => {
                if let Some(size) = layers.root_size {
                    set("SizeMinBytes", MIN_ROOT_SIZE.min(size).to_string());
                    set("SizeMaxBytes", size.to_string());
                }
                if let Some(tpm) = layers.encrypt {
                    set(
                        "Encrypt",
                        if tpm { "key-file+tpm2" } else { "key-file" }.to_owned(),
                    );
                }
            }
            _ => {}
        }
        if !layers.copy_files {
            part.remove("CopyFiles");
            part.remove("ExcludeFiles");
        }
        let name = path.file_name().ok_or_eyre("template without file name")?;
        std::fs::write(dest.join(name), ini.to_string())?;
    }
    Ok(())
}

/// Make sure repart adopts the ESP Windows uses, and leaves every other existing partition alone.
fn check_adoption(out: &Output, info: &DualBootInfo) -> Result<()> {
    for part in &out.partitions {
        let foreign = part.file().as_os_str().is_empty();
        let is_esp = part.part_type == "esp" && !foreign;
        if (foreign || is_esp) && part.activity() != "unchanged" {
            bail!(
                "systemd-repart wants to {} existing partition {}, refusing",
                part.activity(),
                part.node
            );
        }
        if !foreign && Path::new(&part.node) == info.windows_partition {
            bail!(
                "systemd-repart matched the Windows partition {}, refusing",
                part.node
            );
        }
        if is_esp && Path::new(&part.node) != info.esp_partition {
            bail!(
                "systemd-repart picked {} as the ESP instead of {:?}, refusing",
                part.node,
                info.esp_partition
            );
        }
    }
    if !out.partitions.iter().any(|p| p.part_type == "esp" && !p.file().as_os_str().is_empty()) {
        bail!("systemd-repart did not adopt the existing ESP");
    }
    Ok(())
}

/// Copy our EFI binaries from `src` onto the ESP, next to Windows'.
fn copy_to_esp(esp: &Path, src: &Path) -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let mount = sys_mount::Mount::builder()
        .fstype("vfat")
        .mount_autodrop(esp, tmp.path(), sys_mount::UnmountFlags::empty())
        .wrap_err_with(|| format!("cannot mount the ESP {}", esp.display()))?;
    dualboot::copy_efi_files(src, &tmp.path().join("EFI"))?;
    nix::unistd::sync();
    drop(mount);
    Ok(())
}

/// `EFI` directory in the copy source, if there is anything in it.
fn efi_source(copy_source: &Path) -> Option<PathBuf> {
    let dir = copy_source.join("boot/efi/EFI");
    std::fs::read_dir(&dir)
        .is_ok_and(|mut d| d.next().is_some())
        .then_some(dir)
}

impl DiskProvisionerModule for DualBoot {
    fn run(&self, playbook: &crate::playbook::Playbook) -> Result<Mounts> {
        let disk = &playbook.destination_disk;
        let dry_run = dry_run();
        let efi_src = self.copy_source.as_deref().and_then(efi_source);
        let esp_required =
            efi_src.as_deref().map_or(0, |src| dualboot::efi_payload_size(src) + ESP_HEADROOM);

        let info = stage!(dualbootcheck "Checking the Windows partition" {
            dualboot::probe(disk, std::slice::from_ref(&self.windows_partition), esp_required)
                .map_err(|e| eyre!("Cannot install alongside Windows: {e}"))?
        });
        if info.windows_partition != self.windows_partition {
            bail!(
                "expected to shrink {:?}, but the Windows partition is {:?}",
                self.windows_partition,
                info.windows_partition
            );
        }
        let windows_size = info.plan(self.windows_size).map_err(|e| eyre!("{e}"))?;
        tracing::info!(?info, windows_size, "Dual boot plan");

        let cfgdir = tempfile::tempdir()?;
        layer_templates(
            &self.directory,
            cfgdir.path(),
            &TemplateLayers {
                esp_size: Some(info.esp_size),
                boot_size: Some(dualboot::BOOT_SIZE),
                root_size: Some(info.root_size(windows_size)),
                encrypt: playbook.encryption.as_ref().map(|e| e.tpm),
                copy_files: self.copy_source.is_some(),
            },
        )?;

        stage!(shrink "Resizing the Windows partition" {
            dualboot::shrink(disk, &info, windows_size, dry_run)?;
        });

        let keyfile = Path::new(crate::consts::LUKS_KEYFILE_PATH);
        if let Some(enc) = &playbook.encryption {
            std::fs::write(keyfile, &enc.encryption_key)?;
        }
        scopeguard::defer! {
            if playbook.encryption.is_some() {
                _ = std::fs::remove_file(keyfile)
                    .inspect_err(|e| tracing::warn!("Failed to remove keyfile: {e}"));
            }
        }

        let args = |dry_run| RepartArgs {
            blockdev: disk,
            cfgdir: cfgdir.path(),
            use_keyfile: playbook.encryption.is_some(),
            copy_source: self.copy_source.as_deref(),
            empty: "allow",
            dry_run: Some(dry_run),
        };
        // see what repart would do before letting it write anything
        let preview = systemd_repart(&args(true))?;
        check_adoption(&preview, &info)?;
        let out = if dry_run {
            preview
        } else {
            stage!(mkpart "Creating partitions and copying files" {
                systemd_repart(&args(false))?
            })
        };

        if let Some(src) = efi_src.filter(|_| !dry_run) {
            stage!(esp "Copying EFI files" {
                copy_to_esp(&info.esp_partition, &src)?;
            });
        }

        mounts_from_output(&out, SystemdRepartData::get_configs(cfgdir.path())?.configs)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use repart::{EncryptOption, FileSystem};

    fn templates() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/dualboot")
    }

    #[test]
    fn layers_sizes_and_encryption() {
        let dest = tempfile::tempdir().unwrap();
        layer_templates(
            &templates(),
            dest.path(),
            &TemplateLayers {
                esp_size: Some(104_857_600),
                boot_size: Some(dualboot::BOOT_SIZE),
                root_size: Some(50 << 30),
                encrypt: Some(true),
                copy_files: true,
            },
        )
        .unwrap();
        let configs = SystemdRepartData::get_configs(dest.path()).unwrap().configs;
        assert_eq!(configs.len(), 3);

        let esp = &configs["20-efi.conf"].partition;
        assert_eq!(esp.size_min_bytes.as_u64(), 104_857_600);
        assert_eq!(esp.size_max_bytes.as_u64(), 104_857_600);
        // never format Windows' ESP
        assert!(esp.format.is_none());
        assert!(esp.copy_files.is_empty());
        assert!(matches!(esp.encrypt, EncryptOption::Off));

        let root = &configs["50-root.conf"].partition;
        assert_eq!(root.size_min_bytes.as_u64(), MIN_ROOT_SIZE);
        assert_eq!(root.size_max_bytes.as_u64(), 50 << 30);
        assert!(matches!(root.encrypt, EncryptOption::KeyFileTpm2));
        assert!(matches!(root.format, Some(FileSystem::Btrfs)));
        assert!(!root.copy_files.is_empty());
        assert_eq!(root.mount_point.len(), 2);

        let boot = &configs["45-boot.conf"].partition;
        assert_eq!(boot.size_max_bytes.as_u64(), dualboot::BOOT_SIZE);
        assert!(matches!(boot.encrypt, EncryptOption::Off));

        // the templates themselves are untouched
        let orig = SystemdRepartData::get_configs(&templates())
            .unwrap()
            .configs;
        assert_eq!(orig["20-efi.conf"].partition.size_max_bytes.as_u64(), 0);
    }

    #[test]
    fn serde_roundtrip() {
        use crate::backend::provisioners::DiskProvisioner;
        let prov = DiskProvisioner::DualBoot(DualBoot {
            directory: templates(),
            copy_source: Some("/run/rootfsbase".into()),
            windows_partition: "/dev/nvme0n1p3".into(),
            windows_size: 123 << 30,
        });
        let json = serde_json::to_string(&prov).unwrap();
        assert!(json.contains(r#""module":"DualBoot""#));
        assert_eq!(
            serde_json::from_str::<DiskProvisioner>(&json).unwrap(),
            prov
        );
    }

    #[test]
    fn layers_without_copy_source() {
        let dest = tempfile::tempdir().unwrap();
        layer_templates(
            &templates(),
            dest.path(),
            &TemplateLayers {
                encrypt: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        let configs = SystemdRepartData::get_configs(dest.path()).unwrap().configs;
        assert!(configs.values().all(|c| c.partition.copy_files.is_empty()));
        assert!(matches!(
            configs["50-root.conf"].partition.encrypt,
            EncryptOption::KeyFile
        ));
    }

    fn info() -> DualBootInfo {
        DualBootInfo {
            windows_partition: "/dev/vda3".into(),
            esp_partition: "/dev/vda1".into(),
            windows_start: 122_683_392,
            windows_size: 4 << 30,
            windows_min: 1 << 30,
            total: 6 << 30,
            esp_size: 104_857_600,
        }
    }

    #[test]
    fn adoption_ok() {
        let out = serde_json::from_str(include_str!(
            "../../../../repart/test/repart-out-dualboot.json"
        ))
        .unwrap();
        check_adoption(&out, &info()).unwrap();
    }

    #[test]
    fn adoption_wrong_esp() {
        let out = serde_json::from_str(include_str!(
            "../../../../repart/test/repart-out-dualboot.json"
        ))
        .unwrap();
        let info = DualBootInfo {
            esp_partition: "/dev/vda7".into(),
            ..info()
        };
        assert!(check_adoption(&out, &info).is_err());
    }

    #[test]
    fn adoption_refuses_touching_existing() {
        let json = include_str!("../../../../repart/test/repart-out-dualboot.json");
        // pretend repart wanted to grow Windows' recovery partition
        let json = json.replacen(r#""activity" : "unchanged""#, r#""activity" : "resize""#, 1);
        let out = serde_json::from_str(&json).unwrap();
        assert!(check_adoption(&out, &info()).is_err());
    }

    #[test]
    fn matches_output_by_file_name() {
        let out: Output = serde_json::from_str(include_str!(
            "../../../../repart/test/repart-out-dualboot.json"
        ))
        .unwrap();
        let configs = SystemdRepartData::get_configs(&templates())
            .unwrap()
            .configs;
        let mounts = mounts_from_output(&out, configs).unwrap();
        let find = |mp: &str| {
            mounts
                .0
                .iter()
                .find(|m| m.mountpoint == Path::new(mp))
                .unwrap()
                .partition
                .clone()
        };
        assert_eq!(find("/boot/efi"), Path::new("/dev/vda1"));
        assert_eq!(find("/boot"), Path::new("/dev/vda5"));
        assert_eq!(find("/"), Path::new("/dev/vda6"));
        assert_eq!(find("/home"), Path::new("/dev/vda6"));
    }

    fn sh(cmd: &str, stdin: Option<&str>) -> String {
        use std::io::Write as _;
        let mut child = Command::new("sh")
            .args(["-ec", cmd])
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.unwrap_or_default().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(out.status.success(), "`{cmd}` failed: {stdout}");
        stdout
    }

    /// The whole shrink + repart flow on image files, which works without root: a GPT disk image
    /// laid out like a stock Windows install (ESP, MSR, NTFS, recovery), with the NTFS volume in
    /// its own image since ntfsresize can't address a partition inside an image without a loop
    /// device. systemd-repart only does a dry run, since formatting needs loop devices.
    #[test]
    #[ignore = "needs ntfs-3g, ntfsprogs, dosfstools, util-linux and systemd-repart"]
    #[allow(clippy::too_many_lines)]
    fn image_dry_run() {
        const GIB: u64 = 1 << 30;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().display().to_string();
        let disk = tmp.path().join("disk.img");
        let ntfs = tmp.path().join("ntfs.img");
        let win_start = 239_616 * 512;
        sh(
            &format!(
                "cd {dir}
                truncate -s 64G disk.img
                truncate -s 100M esp.img && mkfs.vfat -F32 esp.img
                dd if=esp.img of=disk.img bs=1M seek=1 conv=notrunc,sparse status=none
                truncate -s 40G ntfs.img && mkntfs -F -Q -L Windows ntfs.img"
            ),
            None,
        );
        sh(
            &format!("sfdisk {dir}/disk.img"),
            Some(&format!(
                "label: gpt
start=2048, size=100MiB, type={ESP}
size=16MiB, type=E3C9E316-0B5C-4DB8-817D-F92DF00215AE
size=40GiB, type=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7
type=DE94BBA4-06D1-4D40-A16A-BFD50179D6AC
",
                ESP = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
            )),
        );

        dualboot::ntfs_probe_result(
            &ntfs,
            Command::new("ntfs-3g.probe")
                .arg("--readonly")
                .arg(&ntfs)
                .output()
                .unwrap()
                .status
                .code(),
        )
        .unwrap();
        assert!(!dualboot::is_hibernated(
            &dualboot::hiberfile_header(&ntfs).unwrap()
        ));
        let ntfs_info = dualboot::parse_ntfsresize_info(
            &ntfs,
            &sh(
                &format!("ntfsresize --info --no-action {dir}/ntfs.img"),
                None,
            ),
        )
        .unwrap();
        let info = DualBootInfo {
            windows_partition: format!("{dir}/disk.img3").into(),
            esp_partition: format!("{dir}/disk.img1").into(),
            windows_start: win_start,
            windows_size: 40 * GIB,
            windows_min: dualboot::align_up(
                ntfs_info.min_size + dualboot::windows_margin(ntfs_info.min_size),
                dualboot::ALIGNMENT,
            ),
            // the recovery partition follows right after Windows
            total: 40 * GIB,
            esp_size: 100 << 20,
        };
        info.check_space().unwrap();
        let new_size = info.plan(15 * GIB + 12_345).unwrap();
        assert_eq!((win_start + new_size) % dualboot::ALIGNMENT, 0);

        // the same steps as dualboot::shrink(), on the images
        sh(
            &format!("ntfsresize --no-action --no-progress-bar --size {new_size} {dir}/ntfs.img"),
            None,
        );
        sh(
            &format!("ntfsresize --no-progress-bar --size {new_size} {dir}/ntfs.img"),
            Some("y\n"),
        );
        let after = dualboot::parse_ntfsresize_info(
            &ntfs,
            &sh(
                &format!("ntfsresize --info --force --no-action {dir}/ntfs.img"),
                None,
            ),
        )
        .unwrap();
        // ntfsresize schedules a chkdsk for the next Windows boot, hence --force above
        assert!(after.volume_size.unwrap() <= new_size);
        let line = dualboot::sfdisk_resize_line(win_start, new_size, 512).unwrap();
        sh(
            &format!("sfdisk --no-reread --wipe never --wipe-partitions never -N 3 {dir}/disk.img"),
            Some(&line),
        );
        let table: serde_json::Value =
            serde_json::from_str(&sh(&format!("sfdisk --json {dir}/disk.img"), None)).unwrap();
        let parts = &table["partitiontable"]["partitions"];
        assert_eq!(parts[2]["start"], win_start / 512);
        assert_eq!(parts[2]["size"], new_size / 512);
        // the recovery partition hasn't moved
        assert_eq!(parts[3]["start"], (win_start + 40 * GIB) / 512);

        let cfgdir = tmp.path().join("defs");
        layer_templates(
            &templates(),
            &cfgdir,
            &TemplateLayers {
                esp_size: Some(info.esp_size),
                boot_size: Some(dualboot::BOOT_SIZE),
                root_size: Some(info.root_size(new_size)),
                encrypt: None,
                copy_files: false,
            },
        )
        .unwrap();
        let out = systemd_repart(&RepartArgs {
            blockdev: &disk,
            cfgdir: &cfgdir,
            use_keyfile: false,
            copy_source: None,
            empty: "allow",
            dry_run: Some(true),
        })
        .unwrap();
        check_adoption(&out, &info).unwrap();
        let root = out.partitions.iter()
            .find(|p| p.part_type.starts_with("root-"))
            .unwrap();
        assert_eq!(root.activity(), "create");

        let mounts = mounts_from_output(
            &out,
            SystemdRepartData::get_configs(&cfgdir).unwrap().configs,
        )
        .unwrap();
        let esp = mounts
            .0
            .iter()
            .find(|m| m.mountpoint == Path::new("/boot/efi"));
        assert_eq!(esp.unwrap().partition, info.esp_partition);
    }
}
