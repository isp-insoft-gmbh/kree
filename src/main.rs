mod audio;
mod logging;
mod parser;
mod paths;
mod scheduler;
mod tray;

use std::io::ErrorKind;
use std::time::Duration;

use anyhow::Result;
use single_instance::SingleInstance;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::scheduler::{ReminderEvent, Scheduler};

fn main() -> Result<()> {
    // Hold the guard for the entire process lifetime so the appender flushes on exit.
    let _log_guard = logging::init()?;

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let mutex_name = format!("kree-singleton-{user}");

    // The mutex must outlive the program; bind it to `_instance` so it isn't dropped early.
    let _instance = match SingleInstance::new(&mutex_name)? {
        i if i.is_single() => i,
        _ => {
            warn!("another kree instance is already running; exiting");
            return Ok(());
        }
    };

    info!("kree starting (user={user})");

    // Build the tray *before* the runtime; tray-icon registers a hidden
    // window on this thread, which must be the same thread that pumps
    // Win32 messages below.
    let tray = tray::build()?;

    // Tokio runtime lives in a worker thread so the main thread is free
    // to run the Win32 message loop.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let _runtime_task = runtime.spawn(run_async(shutdown_rx));

    // Block until the user clicks Quit (or the loop fails).
    tray::run_event_loop(&tray)?;

    info!("shutting down");
    let _ = shutdown_tx.send(());
    runtime.shutdown_timeout(Duration::from_secs(2));
    Ok(())
}

/// Async lifecycle: load reminders, run scheduler, drain events, wait for
/// shutdown.
async fn run_async(shutdown: oneshot::Receiver<()>) -> Result<()> {
    let reminders = load_reminders()?;
    info!(count = reminders.len(), "scheduling reminders");

    let (tx, mut rx) = mpsc::channel::<ReminderEvent>(64);
    let sched = Scheduler::spawn(reminders, tx);

    let receiver = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            info!(
                schedule = %event.schedule,
                icon = %event.icon,
                body = %event.body,
                fired_at = %event.fired_at,
                "reminder fired"
            );
            audio::play_chime();
        }
    });

    let _ = shutdown.await;
    sched.shutdown();
    receiver.abort();
    Ok(())
}

fn load_reminders() -> Result<Vec<parser::Reminder>> {
    let path = paths::reminders_path()?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let report = parser::parse(&contents);
            for (line, err) in &report.errors {
                warn!(line, error = %err, "skipping invalid reminder");
            }
            info!(path = %path.display(), count = report.reminders.len(), "loaded reminders");
            Ok(report.reminders)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!(path = %path.display(), "no reminders file yet — create it to start scheduling");
            Ok(Vec::new())
        }
        Err(e) => Err(e.into()),
    }
}
