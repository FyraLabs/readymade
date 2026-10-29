use repart::{Config, EncryptOption, Output, OutputPartition, Partition};

use file_guard::Lock;
use std::{collections::BTreeMap, process::Stdio};

use crate::{
    backend::{provisioners::disk::DiskProvisionerModule, util::fs::CopySource},
    playbook::EncryptionConfig,
    prelude::*,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemdRepartData {
    pub configs: BTreeMap<String, Config>,
}

impl SystemdRepartData {
    #[must_use]
    pub const fn new(configs: BTreeMap<String, Config>) -> Self {
        Self { configs }
    }

    /// Read and parse every systemd-repart config file in `cfg_path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or a file in it cannot be read, or a file is not a valid
    /// repart config.
    pub fn get_configs(cfg_path: &Path) -> Result<Self> {
        let mut configs = BTreeMap::new();
        // Read path
        for entry in std::fs::read_dir(cfg_path)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_config = std::fs::read_to_string(&path)?;

            // Parse the config
            let config: Config = serde_systemd_unit::from_str(&file_config)?;

            // Add to the list
            configs.insert(entry.file_name().to_string_lossy().into_owned(), config);
        }
        Ok(Self::new(configs))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Repart {
    pub directory: PathBuf,
    pub copy_source: Option<PathBuf>,
}

/// Copy the repart definitions in `src` to `dest`, so they can be modified without touching the
/// originals.
///
/// If `encryption` is set, root partitions that don't specify `Encrypt=` themselves are set to
/// be encrypted with the key file (and the TPM, if enabled).
fn layer_configs(src: &Path, dest: &Path, encryption: Option<&EncryptionConfig>) -> Result<()> {
    for entry in
        std::fs::read_dir(src).wrap_err_with(|| format!("cannot read {}", src.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let dest = dest.join(entry.file_name());
        let Some(EncryptionConfig { tpm, .. }) = encryption else {
            std::fs::copy(&path, &dest)?;
            continue;
        };

        let contents = std::fs::read_to_string(&path)?;
        let mut unit = serde_systemd_unit::parse(&contents)
            .map_err(|e| eyre!("cannot parse {}: {e:?}", path.display()))?;
        let Some(partition) = unit.sections.get_mut("Partition") else {
            std::fs::copy(&path, &dest)?;
            continue;
        };
        let is_root = (partition.get("Type"))
            .is_some_and(|t| t.as_str() == "root" || t.as_str().starts_with("root-"));
        if !is_root || partition.contains_key("Encrypt") {
            std::fs::copy(&path, &dest)?;
            continue;
        }

        let encrypt = if *tpm { "key-file+tpm2" } else { "key-file" };
        tracing::debug!(?path, encrypt, "Enabling encryption for partition");
        partition.insert(
            "Encrypt".to_owned(),
            serde_systemd_unit::Value::String(encrypt.to_owned()),
        );
        std::fs::write(&dest, unit.to_string())?;
    }
    Ok(())
}

impl DiskProvisionerModule for Repart {
    fn run(&self, playbook: &crate::playbook::Playbook) -> Result<Mounts> {
        let cfgdir = tempfile::Builder::new()
            .prefix("readymade-repart-")
            .tempdir()?;
        layer_configs(&self.directory, cfgdir.path(), playbook.encryption.as_ref())?;

        // NamedTempFile is created with 0600 permissions and deleted on drop
        let keyfile = (playbook.encryption.as_ref())
            .map(|e| -> Result<_> {
                let mut f = tempfile::NamedTempFile::new()?;
                std::io::Write::write_all(&mut f, e.encryption_key.as_bytes())?;
                Ok(f)
            })
            .transpose()
            .wrap_err("cannot write LUKS key file")?;

        let copy_source = (self.copy_source.as_deref())
            .map(CopySource::open)
            .transpose()?;

        let repart_out = systemd_repart(
            &playbook.destination_disk,
            cfgdir.path(),
            keyfile.as_ref().map(tempfile::NamedTempFile::path),
            copy_source.as_ref().map(CopySource::path),
        )?;
        drop(keyfile);
        drop(copy_source);

        let configs = SystemdRepartData::get_configs(cfgdir.path())?.configs;

        let mut mounts = vec![];
        for (file, config) in configs {
            let Partition {
                label,
                mount_point,
                encrypt,
                ..
            } = config.partition;
            // repart lists partitions in partition table order, which doesn't have to match the
            // order of the definitions, so look them up by the definition they came from
            let OutputPartition { node, .. } = (repart_out.partitions.iter())
                .find(|p| p.file().file_name() == Some(std::ffi::OsStr::new(&file)))
                .ok_or_else(|| eyre!("systemd-repart did not create a partition for {file}"))?;
            let encryption_type = match encrypt {
                EncryptOption::Off => None,
                EncryptOption::KeyFile => Some(EncryptionOption::KeyFile),
                EncryptOption::Tpm2 => Some(EncryptionOption::Tpm2),
                EncryptOption::KeyFileTpm2 => Some(EncryptionOption::KeyFileTpm2),
            };

            // MountPoint=PATH[:OPTIONS]
            for mount_point in mount_point.iter().filter(|m| !m.is_empty()) {
                let (path, options) = mount_point.split_once(':').unwrap_or((mount_point, ""));
                mounts.push(Mount::new(
                    PathBuf::from(node),
                    PathBuf::from(path),
                    options.to_owned(),
                    encryption_type.clone(),
                    label.clone(),
                ));
            }
        }
        Ok(Mounts(mounts))
    }
}

fn systemd_repart(
    blockdev: &Path,
    cfgdir: &Path,
    keyfile: Option<&Path>,
    copy_source: Option<&Path>,
) -> Result<Output> {
    // HACK: #59, repart sometimes fails to reread the partition table it just wrote because
    // something (usually udev probing the new partitions) still has the disk open
    const DEVICE_BUSY: &str = "Device or resource busy";
    const ATTEMPTS: u32 = 3;

    let dry_run = std::env::var("READYMADE_DRY_RUN").map_or(cfg!(debug_assertions), |v| v == "1");
    tracing::debug!(?dry_run, "Running systemd-repart");

    let mut attempt = 1;
    let repart_cmd = loop {
        let output = run_systemd_repart(blockdev, cfgdir, keyfile, copy_source, dry_run)?;
        if output.status.success()
            || attempt >= ATTEMPTS
            || !String::from_utf8_lossy(&output.stderr).contains(DEVICE_BUSY)
        {
            break output;
        }
        tracing::warn!(
            attempt,
            "systemd-repart failed because the device was busy, retrying"
        );
        attempt += 1;
        std::thread::sleep(std::time::Duration::from_secs(2));
    };

    let stderr = String::from_utf8_lossy(&repart_cmd.stderr);
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }

    if !repart_cmd.status.success() {
        return Err(eyre!(
            "systemd-repart errored with status code {:?}",
            repart_cmd.status.code()
        ))
        .with_note(|| format!("stderr:\n{stderr}"));
    }

    // todo: wait for systemd 256 or genfstab magic
    tracing::debug!("systemd-repart finished");
    serde_json::from_slice(&repart_cmd.stdout).wrap_err("cannot parse systemd-repart output")
}

/// Run `systemd-repart` once, holding an exclusive lock on `blockdev` while it runs.
fn run_systemd_repart(
    blockdev: &Path,
    cfgdir: &Path,
    keyfile: Option<&Path>,
    copy_source: Option<&Path>,
    dry_run: bool,
) -> Result<std::process::Output> {
    // We are locking the device so that repart doesn't fail due to device busy
    let mut device = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(blockdev)
        .context("Failed to open block device")?;
    let _lock = file_guard::lock(&mut device, Lock::Exclusive, 0, 1)?;

    let mut cmd = Command::new("systemd-repart");

    if let Some(copy_source) = copy_source {
        cmd.arg("--copy-source").arg(copy_source);
    }

    if let Some(keyfile) = keyfile {
        tracing::debug!(?keyfile, "Using keyfile for systemd-repart");
        cmd.arg("--key-file").arg(keyfile);
    }

    // HACK: Disable whole-device TRIM to reduce wear on SSDs and formatting time
    // https://github.com/systemd/systemd/issues/32760
    // TODO: Turn off once systemd 259 lands
    // https://github.com/systemd/systemd/commit/29ee9c6fb7c75c421f887c8579c65eb04d4f634d
    cmd.env("SYSTEMD_REPART_MKFS_OPTIONS_BTRFS", "--nodiscard")
        .args(["--dry-run", if dry_run { "yes" } else { "no" }])
        .arg("--definitions")
        .arg(cfgdir)
        .args(["--empty", "force", "--offline", "false", "--json", "pretty"])
        .arg(blockdev)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    tracing::debug!(?cmd, "Executing systemd-repart command");

    cmd.output().context("can't run systemd-repart")
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_configs_encrypts_root() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/wholedisk");
        let dest = tempfile::tempdir().unwrap();
        let encryption = EncryptionConfig {
            tpm: true,
            encryption_key: "hunter2".to_owned(),
        };
        layer_configs(&src, dest.path(), Some(&encryption)).unwrap();

        let configs = SystemdRepartData::get_configs(dest.path()).unwrap().configs;
        assert_eq!(configs.len(), 3);
        let root = &configs["50-root.conf"].partition;
        assert!(matches!(root.encrypt, EncryptOption::KeyFileTpm2));
        assert_eq!(root.mount_point.len(), 2);
        assert!(matches!(
            configs["20-efi.conf"].partition.encrypt,
            EncryptOption::Off
        ));
        // untouched files are copied verbatim
        assert_eq!(
            std::fs::read_to_string(dest.path().join("45-boot.conf")).unwrap(),
            std::fs::read_to_string(src.join("45-boot.conf")).unwrap()
        );
    }

    #[test]
    fn test_layer_configs_no_encryption() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/wholedisk");
        let dest = tempfile::tempdir().unwrap();
        layer_configs(&src, dest.path(), None).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("50-root.conf")).unwrap(),
            std::fs::read_to_string(src.join("50-root.conf")).unwrap()
        );
    }
}
