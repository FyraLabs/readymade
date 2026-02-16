mod bootc;
mod copy;

use crate::prelude::*;
use bootc::Bootc;
use copy::Copy;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[enum_dispatch(FileSystemProvisioner)]
pub trait FileSystemProvisionerModule {
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()>;
    fn cleanup(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()> {
        Ok(())
    }
}

/// The filesystem provisioner is responsible for copying the OS files to the partitions after the disk provisioner has set up the partitions.
/// Provisioners may also use context from the playbook to determine how to provision the installation, such as the destination disk and encryption settings.
#[enum_dispatch]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "module")]
pub enum FileSystemProvisioner {
    /// Provisions a bootc install.
    /// Refer to: https://bootc-dev.github.io/bootc/man/bootc-install-to-disk.8.html
    Bootc,
    /// Mounts the partitions specified by the end-user, then copy the files.
    Copy,
}
