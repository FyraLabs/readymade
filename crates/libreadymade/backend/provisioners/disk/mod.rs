use crate::prelude::*;
use enum_dispatch::enum_dispatch;
use manual::Manual;
use repart::Repart;
use serde::{Deserialize, Serialize};

pub mod manual;
pub mod repart;

#[enum_dispatch(DiskProvisioner)]
pub trait DiskProvisionerModule {
    fn run(&self, playbook: &crate::playbook::Playbook) -> Result<Mounts>;
}

/// The disk provisioner is responsible for partitioning the disk, before the filesystem provisioner sets up the install files on the partitions.
/// Provisioners may also use context from the playbook to determine how to provision the installation, such as the destination disk and encryption settings.
/// Some disk provisioners support copying files to the installation disk, making a filesystem provisioner optional.
#[enum_dispatch]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "module")]
pub enum DiskProvisioner {
    /// Uses systemd-repart to partition and provision the disk, this is recommended for most users as it is fast and flexible.
    /// Refer to: https://www.freedesktop.org/software/systemd/man/latest/repart.d.html
    Repart,
    /// Readymade will not partition the disk. Instead, the user provides a list of mountpoints.
    Manual,
}
