use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use color_eyre::{
    Result, Section as _,
    eyre::{Context as _, OptionExt as _},
};
use serde::{Deserialize, Serialize};

use super::{Context, PostInstallModule};

/// Run the distribution's postinstall scripts inside the installed system.
///
/// The [`Context`] is passed to each script as JSON on stdin.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Script;

const SCRIPTS: [&str; 2] = [
    "/etc/readymade/postinstall.sh",
    "/usr/share/readymade/postinstall.sh",
];
const SCRIPT_DIRS: [&str; 2] = [
    "/etc/readymade/postinstall.d/",
    "/usr/share/readymade/postinstall.d/",
];

impl PostInstallModule for Script {
    fn name(&self) -> &'static str {
        "Script"
    }

    fn run(&self, context: &Context) -> Result<()> {
        for script in SCRIPTS.map(Path::new).into_iter().filter(|p| p.exists()) {
            run_script(context, script)?;
        }

        for dir in SCRIPT_DIRS
            .map(Path::new)
            .into_iter()
            .filter(|p| p.exists())
        {
            let mut scripts = vec![];
            for f in std::fs::read_dir(dir)? {
                let f = f?;
                let metadata = std::fs::metadata(f.path())?;
                if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                    scripts.push(f.path());
                }
            }
            // run in a predictable order, so scripts can be ordered by prefixing them with numbers
            scripts.sort();
            scripts
                .iter()
                .try_for_each(|script: &PathBuf| run_script(context, script))?;
        }

        Ok(())
    }
}

fn run_script(context: &Context, script: &Path) -> Result<()> {
    tracing::info!(?script, "Running postinstall script");
    let mut cmd = std::process::Command::new(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .wrap_err_with(|| format!("cannot run {}", script.display()))?;

    let stdin = (cmd.stdin.take()).ok_or_eyre("cannot open stdin of script")?;
    match serde_json::to_writer(stdin, context) {
        // the script doesn't have to read the context
        Err(e) if e.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) => {}
        res => res.wrap_err("fail to serialize ctx")?,
    }
    let out = cmd.wait_with_output()?;
    if !out.status.success() {
        return Err(color_eyre::Report::msg("fail to run script(s)")
            .note(format!("Running: {}", script.display()))
            .note(out.status.to_string())
            .section(format!("Stdout:\n{}", String::from_utf8_lossy(&out.stdout)))
            .section(format!("Stderr:\n{}", String::from_utf8_lossy(&out.stderr))));
    }

    Ok(())
}
