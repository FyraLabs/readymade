use crate::prelude::*;
use serde::{Deserialize, Serialize};

use crate::stage;

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SELinux;

impl PostInstallModule for SELinux {
    fn name(&self) -> &'static str {
        "SELinux"
    }

    fn run(&self, _context: &Context) -> Result<()> {
        stage!(selinux "Setting SELinux labels" {
            crate::cmd!("setfiles" [
                // /dev and /run/host are bind mounts from the live system
                ["-e", "/proc", "-e", "/sys", "-e", "/dev", "-e", "/run/host"],
                ["/etc/selinux/targeted/contexts/files/file_contexts", "/"],
            ] => |e| bail!("setfiles failed with exit code {:?}", e.code()));
        });

        Ok(())
    }
}
