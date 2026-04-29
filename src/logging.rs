use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use crate::paths;

/// Initialize tracing with a rolling daily file appender at
/// `<data_dir>/logs/app.log`. Default level `info`; respects `RUST_LOG`.
///
/// The returned [`WorkerGuard`] must be held by the caller for the lifetime
/// of the process — dropping it flushes the non-blocking writer.
pub fn init() -> Result<WorkerGuard> {
    let logs = paths::logs_dir()?;
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("creating log directory {}", logs.display()))?;

    let appender = tracing_appender::rolling::daily(&logs, "app.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(false)
        .init();

    Ok(guard)
}
