mod logging;
mod paths;

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
    Ok(())
}
