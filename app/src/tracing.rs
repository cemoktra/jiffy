use tracing::subscriber;

/// Configures the global tracing subscriber to not care about any spans or events, so the
/// `tracing` crate never writes out log lines for spans and trace events.
pub fn init() -> anyhow::Result<()> {
    subscriber::set_global_default(subscriber::NoSubscriber::new())?;
    Ok(())
}
