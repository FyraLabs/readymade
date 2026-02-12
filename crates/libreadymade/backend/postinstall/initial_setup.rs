use color_eyre::Result;
use serde::{Deserialize, Serialize};

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InitialSetup;

impl PostInstallModule for InitialSetup {
    fn run(&self, _context: &Context) -> Result<()> {
        // This triggers whatever the heck (e.g. Taidan) during next boot
        std::fs::File::create("/.unconfigured")?;
        Ok(())
    }
}
