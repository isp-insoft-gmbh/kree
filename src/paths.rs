use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::BaseDirs;

/// Root of kree's per-user state: `%APPDATA%\kree` on Windows.
///
/// We use `BaseDirs::data_dir()` (not `ProjectDirs`) because `ProjectDirs`
/// appends a `/data` subdirectory on Windows, which the spec layout in
/// SPEC.md § 3 doesn't want.
pub fn data_dir() -> Result<PathBuf> {
    let base = BaseDirs::new().context("could not resolve user base directories")?;
    Ok(base.data_dir().join("kree"))
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("logs"))
}

pub fn reminders_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("reminders.txt"))
}

/// Path to the most recent log file, by lexicographic filename order
/// (`tracing-appender` rolls daily as `app.log.YYYY-MM-DD`, so a string
/// compare gives the newest). `None` if the logs directory is empty
/// or missing.
pub fn latest_log_file() -> Result<Option<PathBuf>> {
    let dir = logs_dir()?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut newest: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match (&newest, path.file_name()) {
            (None, _) => newest = Some(path),
            (Some(prev), Some(name)) if Some(name) > prev.file_name() => {
                newest = Some(path);
            }
            _ => {}
        }
    }
    Ok(newest)
}
