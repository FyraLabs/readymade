use std::{path::PathBuf, process::Command};

use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct OSProbe {
    pub part: PathBuf,
    pub os_name_pretty: String,
    // pub os_name: String,
    // pub part_type: String,
    // pub part_fs: Option<String>,
    // pub part_uuid: Option<String>,
    // pub kernel_opts: Option<String>,
}

impl OSProbe {
    #[tracing::instrument]
    pub fn from_entry(entry: &str) -> Self {
        let parts = tracing::debug_span!("OS Probe Entry", ?entry)
            .in_scope(|| entry.split(':').collect_vec());

        // Minimum 4 parts, Part 5, 6 and 7 are optional

        let [part, os_name_pretty, /*os_name, part_type,*/ ..] = parts[..] else {
            panic!("Expected at least 4 OS Probe entries for `{entry}`, but found the following: {parts:?}");
        };

        tracing::info_span!("Serializing os-prober entry").in_scope(|| Self {
            part: part.into(),
            os_name_pretty: os_name_pretty.to_owned(),
            // os_name: os_name.to_owned(),
            // part_type: part_type.to_owned(),
            // part_fs: parts.get(4).map(ToString::to_string),
            // part_uuid: parts.get(5).map(ToString::to_string),
            // kernel_opts: parts.get(6).map(ToString::to_string),
        })
    }

    pub fn scan() -> Option<Vec<Self>> {
        const ERROR: &str = "os-prober failed to run! Are we root? Is it installed? Continuing without OS detection.";

        let ret = tracing::info_span!("Scanning for OS").in_scope(|| {
            tracing::info!("Scanning for OS with os-prober");
            let x = Command::new("pkexec").arg("os-prober").output().ok()?;
            let strout = String::from_utf8(x.stdout).inspect_err(|e| {
                tracing::error!(?e, "os-prober should return valid utf8");
            });
            let strout = strout.ok()?;
            tracing::info!(?strout, "OS Probe Output");
            let v = strout.lines().map(str::trim);
            let v = v.filter(|l| !l.is_empty()).map(Self::from_entry);
            let v = v.collect_vec();
            (!v.is_empty()).then_some(v)
        });
        ret.or_else(|| {
            tracing::error!("{ERROR}");
            None
        })
    }
}
