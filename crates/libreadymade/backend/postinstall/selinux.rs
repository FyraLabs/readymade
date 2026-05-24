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
                ["-e", "/proc", "-e", "/sys"],
                ["/etc/selinux/targeted/contexts/files/file_contexts", "/"],
            ] => |e| bail!("setfiles failed with exit code {:?}", e.code()));
        });

        Ok(())
    }
}
