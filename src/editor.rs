use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use tracing::info;

const TEMPLATE: &str = "\
# kree reminders — dev daily defaults.
# Format: <cron-expression> | <message>
# First emoji of message becomes the popup icon (Win+. for picker).

# Daily — work week
25 9 * * 1-5      | 🎙️ Standup in 5 — what shipped, what's blocked, what's next
0 12 * * 1-5      | 🍽️ Lunch — step away from the screen
0 14 * * 1-5      | 🎯 Afternoon deep-work block — silence Slack until 15
55 16 * * 1-5     | 🛑 Shutdown ritual — close tabs, write tomorrow's first task

# Hydration + posture
*/45 9-17 * * 1-5 | 💧 Drink water and stand for a moment
0 11 * * 1-5      | 🧘 Stand and stretch — 60s

# Weekly
30 9 * * 1        | 📋 Weekly review — last week's wins, this week's top three
0 16 * * 5        | 🧹 Friday tidy — close stale PRs, sweep notifications, drain inbox
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
