use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

const DEBOUNCE: Duration = Duration::from_millis(500);

/// Watch the reminders file. Every debounced event boils down to a unit
/// reload signal on `tx`; the consumer re-parses the file. Drop the
/// returned [`Debouncer`] to stop watching.
pub fn watch(path: &Path, tx: UnboundedSender<()>) -> Result<Debouncer<RecommendedWatcher>> {
    let mut debouncer = new_debouncer(DEBOUNCE, move |res: DebounceEventResult| match res {
        Ok(events) if !events.is_empty() => {
            debug!(count = events.len(), "reminders.txt change detected");
            let _ = tx.send(());
        }
        Ok(_) => {}
        Err(error) => {
            warn!(error = %error, "file watcher error");
        }
    })
    .context("constructing notify debouncer")?;

    debouncer
        .watcher()
        .watch(path, RecursiveMode::NonRecursive)
        .with_context(|| format!("watching {}", path.display()))?;

    Ok(debouncer)
}
