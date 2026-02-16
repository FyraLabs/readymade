use gpt::partition_types;
use repart::{Config, EncryptOption, Output, OutputPartition, Partition};

use file_guard::Lock;
use std::{collections::BTreeMap, process::Stdio, str::FromStr};
use uuid::Uuid;

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
            configs.insert(
                path.file_name().unwrap().to_string_lossy().to_string(),
                config,
            );
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
        let repart_out = systemd_repart(
            &playbook.destination_disk,
            &self.directory,
            playbook.encryption.is_some(),
            self.copy_source.as_deref(),
        )?;
        let repartcfg_export = SystemdRepartData::get_configs(&self.directory)?;
        let mut configs = repartcfg_export.configs.into_iter().collect_vec();
        configs.sort_by_key(|(k, _)| k.clone());
        Ok(Mounts(
            configs
                .into_iter()
                .map(|(_, v)| v)
                .enumerate()
                .flat_map(
                    |(
                        i,
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
                        let OutputPartition {
                            node, part_type, ..
                        } = repart_out.partitions.get(i).expect("part doesn't exist");

                        mount_point.into_iter().filter_map(move |mount_point| {
                            if mount_point.is_empty() {
                                return None;
                            }
                            // If there's a colon, split it into two fields
                            // only the first colon is considered though, so if there are more than one, the rest are ignored
                            let mut parts = mount_point.splitn(2, ':');
                            let fst = parts.next()?.to_owned();
                            let snd = parts.next().map(std::borrow::ToOwned::to_owned);
                            Some(std::io::Result::Ok(Mount {
                                label: label.clone(),
                                partition: PathBuf::from(node),
                                mountpoint: PathBuf::from(fst),
                                options: snd.unwrap_or_default(),
                                encryption_type: match encrypt {
                                    EncryptOption::Off => None,
                                    EncryptOption::KeyFile => Some(EncryptionOption::KeyFile),
                                    EncryptOption::Tpm2 => Some(EncryptionOption::Tpm2),
                                    EncryptOption::KeyFileTpm2 => {
                                        Some(EncryptionOption::KeyFileTpm2)
                                    }
                                },
                                gpt_type: None,
                                // gpt_type: Some(partition_types::Type::from(
                                //     Uuid::from_str(part_type).unwrap(),
                                // )),
                            }))
                        })
                    },
                )
                .try_collect()?,
        ))
    }
}

fn systemd_repart(
    blockdev: &Path,
    cfgdir: &Path,
    use_keyfile: bool,
    copy_source: Option<&Path>,
) -> Result<Output> {
    let dry_run = std::env::var("READYMADE_DRY_RUN").map_or(cfg!(debug_assertions), |v| v == "1");
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
        let mut _lock = file_guard::lock(&mut device, Lock::Exclusive, 0, 1)?;

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
            .args(["--empty", "force", "--offline", "false", "--json", "pretty"])
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
