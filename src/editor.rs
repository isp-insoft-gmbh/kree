use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tracing::info;

const TEMPLATE: &str = "\
# kree reminders — one per line.
# Format: <cron-expression> | <message>
#
# Examples:
#   */30 9-17 * * 1-5 | 💧 Drink water
#   0 17 * * 1-5      | 🛑 Shutdown ritual
#   0 9 * * 1         | 📋 Weekly review
#
# The first emoji of the message becomes the popup icon.

";

/// Open the user's `$VISUAL` / `$EDITOR` on the given file, creating the
/// file (and its parent directory) with a header template if missing.
/// Non-blocking: returns as soon as the editor is spawned.
pub fn open(path: &Path) -> Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(path, TEMPLATE)
            .with_context(|| format!("seeding template at {}", path.display()))?;
    }

    let editor = edit::get_editor().context("resolving editor (VISUAL / EDITOR)")?;
    info!(editor = %editor.display(), path = %path.display(), "spawning editor");
    Command::new(editor)
        .arg(path)
        .spawn()
        .context("spawning editor process")?;
    Ok(())
}
