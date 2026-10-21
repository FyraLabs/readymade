mod bootc;
mod copy;

use crate::prelude::*;
pub use bootc::Bootc;
pub use copy::Copy;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[enum_dispatch(FileSystemProvisioner)]
pub trait FileSystemProvisionerModule {
    /// Copy the OS files onto the provisioned partitions.
    ///
    /// # Errors
    ///
    /// Returns an error if the files cannot be copied.
    fn run(&self, playbook: &crate::playbook::Playbook, mounts: &Mounts) -> Result<()>;
    /// Clean up after post-installation has run. Does nothing by default.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup fails.
    fn cleanup(&self, _playbook: &crate::playbook::Playbook, _mounts: &Mounts) -> Result<()> {
        Ok(())
    }
}

/// The filesystem provisioner is responsible for copying the OS files to the partitions after the disk provisioner has set up the partitions.
///
/// Provisioners may also use context from the playbook to determine how to provision the installation, such as the destination disk and encryption settings.
#[enum_dispatch]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "module")]
pub enum FileSystemProvisioner {
    /// Provisions a bootc install.
    /// Refer to: <https://bootc-dev.github.io/bootc/man/bootc-install-to-disk.8.html>
    Bootc,
    /// Mounts the partitions specified by the end-user, then copy the files.
    Copy,
}
