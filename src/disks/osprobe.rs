use std::path::PathBuf;

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
        let parts: Vec<&str> =
            tracing::debug_span!("OS Probe Entry", ?entry).in_scope(|| entry.split(':').collect());

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

    // #[tracing::instrument]
    pub fn scan() -> Option<Vec<Self>> {
        // check if root already

        const ERROR: &str = "os-prober failed to run! Are we root? Is it installed? Continuing without OS detection.";

        let scan = tracing::info_span!("Scanning for OS").in_scope(|| {
            tracing::info!("Scanning for OS with os-prober");
            (crate::util::sys::run_as_root("os-prober").ok())
                .map(|x| x.trim().to_owned())
                .filter(|x| !x.is_empty())
        });

        // let scan: Option<String> = Some("".to_string()); // test case for failure

        scan.map(|strout| {
            tracing::info!(?strout, "OS Probe Output");

            (strout.split('\n').map(str::trim))
                .filter(|l| !l.is_empty())
                .map(Self::from_entry)
                .collect()
        })
        .or_else(|| {
            tracing::error!("{ERROR}");
            None
        })
    }
}
