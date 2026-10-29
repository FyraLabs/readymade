use repart::{Config, EncryptOption, Output, OutputPartition, Partition};

use file_guard::Lock;
use std::{collections::BTreeMap, process::Stdio};

use crate::{backend::provisioners::disk::DiskProvisionerModule, prelude::*};

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

impl DiskProvisionerModule for Repart {
    fn run(&self, playbook: &crate::playbook::Playbook) -> Result<Mounts> {
        let repart_out = systemd_repart(&RepartArgs {
            blockdev: &playbook.destination_disk,
            cfgdir: &self.directory,
            use_keyfile: playbook.encryption.is_some(),
            copy_source: self.copy_source.as_deref(),
            empty: "force",
            dry_run: None,
        })?;
        let repartcfg_export = SystemdRepartData::get_configs(&self.directory)?;
        mounts_from_output(&repart_out, repartcfg_export.configs)
    }
}

/// Turn systemd-repart's output into mounts, using the `MountPoint=`s of the definitions in
/// `configs` (keyed by file name).
///
/// Output partitions are matched to definitions by file name rather than position: with
/// `--empty=allow`, partitions that already existed are listed too, in partition-table order.
///
/// # Errors
///
/// Returns an error if a definition has no partition in the output.
pub(crate) fn mounts_from_output(
    repart_out: &Output,
    configs: BTreeMap<String, Config>,
) -> Result<Mounts> {
    Ok(Mounts(
        configs
            .into_iter()
            .map(|(file, config)| {
                let node = repart_out.partitions.iter()
                    .find(|part| part.file().file_name() == Some(std::ffi::OsStr::new(&file)))
                    .map(|OutputPartition { node, .. }| node)
                    .ok_or_else(|| eyre!("systemd-repart output has no partition for {file}"))?;
                Ok((node, config))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flat_map(
                |(
                    node,
                    Config {
                        partition:
                            Partition {
                                label,
                                mount_point,
                                encrypt,
                                ..
                            },
                    },
                )| {
                    mount_point.into_iter().filter_map(move |mount_point| {
                        if mount_point.is_empty() {
                            return None;
                        }
                        // If there's a colon, split it into two fields
                        // only the first colon is considered though, so if there are more than one, the rest are ignored
                        let mut parts = mount_point.splitn(2, ':');
                        let fst = parts.next()?.to_owned();
                        let snd = parts.next().map(ToOwned::to_owned);
                        Some(Mount::new(
                            PathBuf::from(node),
                            PathBuf::from(fst),
                            snd.unwrap_or_default(),
                            match encrypt {
                                EncryptOption::Off => None,
                                EncryptOption::KeyFile => Some(EncryptionOption::KeyFile),
                                EncryptOption::Tpm2 => Some(EncryptionOption::Tpm2),
                                EncryptOption::KeyFileTpm2 => Some(EncryptionOption::KeyFileTpm2),
                            },
                            label.clone(),
                        ))
                    })
                },
            )
            .collect(),
    ))
}

pub(crate) struct RepartArgs<'a> {
    pub blockdev: &'a Path,
    pub cfgdir: &'a Path,
    pub use_keyfile: bool,
    pub copy_source: Option<&'a Path>,
    /// `--empty=`, see `systemd-repart(8)`
    pub empty: &'a str,
    /// Force (or forbid) a dry run, instead of following `READYMADE_DRY_RUN`
    pub dry_run: Option<bool>,
}

/// Whether to only pretend to touch disks, from `READYMADE_DRY_RUN` (default on in debug builds).
pub(crate) fn dry_run() -> bool {
    std::env::var("READYMADE_DRY_RUN").map_or(cfg!(debug_assertions), |v| v == "1")
}

pub(crate) fn systemd_repart(args: &RepartArgs<'_>) -> Result<Output> {
    let &RepartArgs {
        blockdev,
        cfgdir,
        use_keyfile,
        copy_source,
        empty,
        dry_run,
    } = args;
    let dry_run = dry_run.unwrap_or_else(self::dry_run);
    tracing::debug!(?dry_run, "Running systemd-repart");
    let arg_keyfile = use_keyfile.then(|| {
        let keyfile_path = crate::consts::LUKS_KEYFILE_PATH;
        tracing::debug!("Using keyfile for systemd-repart: {keyfile_path}");
        ["--key-file", keyfile_path]
    });

    // Scope to ensure device and lock live long enough for the command
    let repart_cmd = {
        // We are locking the device so that repart doesn't fail due to device busy
        let mut device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(blockdev)
            .context("Failed to open block device")?;
        let _lock = file_guard::lock(&mut device, Lock::Exclusive, 0, 1)?;

        // HACK: Disable whole-device TRIM to reduce wear on SSDs and formatting time
        // https://github.com/systemd/systemd/issues/32760
        // TODO: Turn off once systemd 259 lands
        // https://github.com/systemd/systemd/commit/29ee9c6fb7c75c421f887c8579c65eb04d4f634d
        let mut cmd = Command::new("systemd-repart");

        if let Some(copy_source) = copy_source {
            cmd.args(["--copy-source", copy_source.to_str().unwrap()]);
        }

        cmd.env("SYSTEMD_REPART_MKFS_OPTIONS_BTRFS", "--nodiscard")
            .args(["--dry-run", if dry_run { "yes" } else { "no" }])
            .args(["--definitions", cfgdir.to_str().unwrap()])
            .args(["--empty", empty, "--offline", "false", "--json", "pretty"])
            .args(arg_keyfile.iter().flatten())
            .arg(blockdev)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        tracing::debug!(?cmd, "Executing systemd-repart command");

        cmd.output().context("can't run systemd-repart")?
    };

    if !repart_cmd.status.success() {
        bail!(
            "systemd-repart errored with status code {:?}",
            repart_cmd.status.code()
        );
    }

    // todo: wait for systemd 256 or genfstab magic
    tracing::debug!("systemd-repart finished");
    Ok(serde_json::from_slice(&repart_cmd.stdout)?)
}
