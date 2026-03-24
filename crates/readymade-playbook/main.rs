use color_eyre::Result;
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
        .expect("usage: readymade-playbook <playbook-file>");

    let playbook: Playbook = serde_json::from_str(fs::read_to_string(playbook_file)?.as_str())?;

    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        let playbook_handle = s.spawn(move || playbook.play(tx));

        s.spawn(move || {
            while let Ok(progress) = rx.recv() {
                dbg!(progress);
            }
        });

        playbook_handle.join().unwrap()
    })?;

    Ok(())
}
