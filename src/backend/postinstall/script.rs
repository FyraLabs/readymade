use std::os::unix::fs::PermissionsExt;

use color_eyre::{eyre::Context as _, Result, Section as _};
use serde::{Deserialize, Serialize};

use super::{Context, PostInstallModule};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Script;

impl PostInstallModule for Script {
    #[allow(clippy::unwrap_in_result)]
    fn run(&self, context: &Context) -> Result<()> {
        if std::fs::exists("/etc/readymade/postinstall.sh").is_ok_and(|x| x) {
            let cmd = std::process::Command::new("/etc/readymade/postinstall.sh")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            handle_process(context, "/etc/readymade/postinstall.sh", cmd)?;
        }

        if std::fs::exists("/usr/share/readymade/postinstall.sh").is_ok_and(|x| x) {
            let cmd = std::process::Command::new("/usr/share/readymade/postinstall.sh")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            handle_process(context, "/usr/share/readymade/postinstall.sh", cmd)?;
        }

        if std::fs::exists("/etc/readymade/postinstall.d/").is_ok_and(|x| x) {
            for f in std::fs::read_dir("/etc/readymade/postinstall.d/")? {
                let f = f?;
                if f.metadata()?.is_file() && f.metadata()?.permissions().mode() & 0o111 != 0 {
                    let cmd = std::process::Command::new(f.path())
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()?;
                    handle_process(context, f.path().to_string_lossy().as_ref(), cmd)?;
                }
            }
        }

        if std::fs::exists("/usr/share/readymade/postinstall.d/").is_ok_and(|x| x) {
            for f in std::fs::read_dir("/usr/share/readymade/postinstall.d/")? {
                let f = f?;
                if f.metadata()?.is_file() && f.metadata()?.permissions().mode() & 0o111 != 0 {
                    let cmd = std::process::Command::new(f.path())
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()?;
                    handle_process(context, f.path().to_string_lossy().as_ref(), cmd)?;
                }
            }
        }

        Ok(())
    }
}

fn handle_process(
    context: &Context,
    f: &str,
    mut cmd: std::process::Child,
) -> Result<(), color_eyre::eyre::Error> {
    serde_json::to_writer(cmd.stdin.as_mut().unwrap(), context)
        .wrap_err("fail to serialize ctx")?;
    let out = cmd.wait_with_output()?;
    if !out.status.success() {
        return Err(color_eyre::Report::msg("fail to run script(s)")
            .note(format!("Running: {f}"))
            .section(format!("Stdout:\n{}", String::from_utf8_lossy(&out.stdout)))
            .section(format!("Stderr:\n{}", String::from_utf8_lossy(&out.stderr))));
    }

    Ok(())
}
