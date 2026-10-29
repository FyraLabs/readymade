use color_eyre::{
    Result,
    eyre::{OptionExt, WrapErr},
};
use libreadymade::playbook::Playbook;
use std::{fs, sync::mpsc, thread};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

fn main() -> Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(fmt::layer().compact())
        .with(EnvFilter::from_env("READYMADE_LOG"))
        .init();

    let playbook_file = std::env::args()
        .nth(1)
        .ok_or_eyre("usage: readymade-playbook <playbook-file>")?;

    let playbook: Playbook = serde_json::from_str(
        &fs::read_to_string(&playbook_file)
            .wrap_err_with(|| format!("cannot read {playbook_file}"))?,
    )
    .wrap_err_with(|| format!("cannot parse playbook {playbook_file}"))?;

    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        // `play()` drops the sender when it's done, which ends the progress loop below
        s.spawn(move || {
            while let Ok(progress) = rx.recv() {
                eprintln!("{progress:?}");
            }
        });

        playbook.play(tx)
    })
}
