use color_eyre::Result;
use serde::{Deserialize, Serialize};

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Language;

impl PostInstallModule for Language {
    fn run(&self, context: &Context) -> Result<()> {
        // use UTF-8 locale, since the non-UTF-8 ones break stuff
        let lang = format!("{}.UTF-8", context.lang).as_bytes();
        // `LOCALE.CONF(5)`: /etc/locale.conf
        std::fs::write(
            "/etc/locale.conf",
            format_bytes::format_bytes!(
                b"LANG={}
LANGUAGE={}
LC_MESSAGES={}",
                lang,
                lang,
                lang
            ),
        )?; // welcome to rust and rustfmt
        Ok(())
    }
}
