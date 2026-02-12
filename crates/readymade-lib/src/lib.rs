pub mod backend;
pub mod cfg;
pub mod consts;
pub mod disks;
pub mod prelude;
pub mod util;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use backend::install::{
    FinalInstallationState, InstallationMessage, InstallationState, InstallationType, IPC_CHANNEL,
};
pub use cfg::ReadymadeConfig;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiskInit {
    pub disk_name: String,
    pub os_name: String,
    pub devpath: PathBuf,
    pub size: bytesize::ByteSize,
}
