use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::util::fs::{exist_then, exist_then_read_dir};

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PrepareFedora;

impl PostInstallModule for PrepareFedora {
    fn run(&self, _context: &Context) -> Result<()> {
        exist_then(std::fs::remove_file("/var/lib/systemd/random-seed"))?;
        // We're gonna make an empty machine-id file so that systemd can generate a new one
        std::fs::File::create("/etc/machine-id")?;

        // wipe NetworkManager state
        exist_then(std::fs::remove_dir_all(
            "/etc/NetworkManager/system-connections",
        ))?;
        std::fs::create_dir_all("/etc/NetworkManager/system-connections")?;

        // todo: Copy over NetworkManager state from current livesys

        // wipe temporary RPM database
        exist_then_read_dir("/var/lib/rpm")?
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("__db"))
            .map(|entry| entry.path())
            .try_for_each(std::fs::remove_file)?;

        // wipe temporary dnf cache
        exist_then(std::fs::remove_dir_all("/var/cache/dnf"))?;

        // todo: set locale and timezone from config

        Ok(())
    }
}
