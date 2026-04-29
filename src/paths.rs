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
