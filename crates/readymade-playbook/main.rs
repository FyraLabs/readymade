use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use color_eyre::Result;
use libreadymade::playbook::Playbook;
use std::fs;

fn main() -> Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(fmt::layer().compact())
        .with(EnvFilter::from_env("READYMADE_LOG"))
        .init();

    let playbook_file = std::env::args()
        .nth(1)
        .expect("usage: readymade-playbook <playbook-file>");

    let playbook: Playbook = serde_json::from_str(fs::read_to_string(playbook_file)?.as_str())?;

    playbook.play()?;

    Ok(())
}
