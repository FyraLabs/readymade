use color_eyre::eyre::Result;

// TODO: Do a GUI and CLI for this, maybe.


fn main() -> Result<()> {
    color_eyre::install()?;
    // set up tracing for logfmt output
    tracing_subscriber::fmt()
        .pretty()
        .with_level(true)
        .with_max_level(tracing::Level::DEBUG)
        .init();
    tracing::debug!("Hello, world!");

    Ok(())
}
