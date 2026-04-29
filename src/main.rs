mod logging;
mod parser;
mod paths;

use std::io::ErrorKind;

use anyhow::Result;
use single_instance::SingleInstance;
use tracing::{info, warn};

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

    let reminders_file = paths::reminders_path()?;
    match std::fs::read_to_string(&reminders_file) {
        Ok(contents) => {
            let report = parser::parse(&contents);
            for (line, err) in &report.errors {
                warn!(line, error = %err, "skipping invalid reminder");
            }
            info!(
                path = %reminders_file.display(),
                count = report.reminders.len(),
                "loaded reminders"
            );
            for r in &report.reminders {
                info!(schedule = %r.schedule, icon = %r.icon, body = %r.body, "reminder");
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!(
                path = %reminders_file.display(),
                "no reminders file yet — create it to start scheduling"
            );
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}
