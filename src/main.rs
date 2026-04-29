mod logging;
mod parser;
mod paths;
mod scheduler;

use std::io::ErrorKind;

use anyhow::Result;
use single_instance::SingleInstance;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::scheduler::{ReminderEvent, Scheduler};

#[tokio::main]
async fn main() -> Result<()> {
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

    let reminders = load_reminders()?;
    info!(count = reminders.len(), "scheduling reminders");

    let (tx, mut rx) = mpsc::channel::<ReminderEvent>(64);
    let sched = Scheduler::spawn(reminders, tx);

    // Receiver loop: log every fire until shutdown. Step 7+ replaces this
    // with the audio + popup pipeline.
    let receiver = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            info!(
                schedule = %event.schedule,
                icon = %event.icon,
                body = %event.body,
                fired_at = %event.fired_at,
                "reminder fired"
            );
        }
    });

    tokio::signal::ctrl_c().await.ok();
    info!("ctrl-c received; shutting down");
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
