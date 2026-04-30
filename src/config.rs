//! User configuration in `%APPDATA%\kree\config.toml`.
//!
//! - Reads use `serde` + `toml` for typed parsing with defaults.
//! - Writes use `toml_edit` so comments / key order / unknown keys
//!   survive a GUI mutation (see `apply_patch_to_string`).
//! - Round-trip is verified by unit tests; no filesystem in test code.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use thiserror::Error;
use toml_edit::{DocumentMut, Item, Value};

/// Documented template seeded into a fresh `config.toml` on first GUI
/// write when no file exists yet.
pub const TEMPLATE: &str = "\
# kree configuration. Hot-reloaded — no restart required.
# Hand-edits and GUI changes round-trip without losing comments or
# unknown keys.

# Play the bundled chime when a reminder fires.
chime = true

# Speak the body via TTS 2 s after fire if the popup is still visible.
speak = true

# Where new popups appear. \"tray\" anchors above the tray icon
# (with a screen-corner fallback when unavailable).
# Other accepted values:
#   top_left, top_center, top_right,
#   left_center, center, right_center,
#   bottom_left, bottom_center, bottom_right
popup_position = \"tray\"

# Color theme. \"system\" follows the Windows app-mode setting at
# HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\\AppsUseLightTheme
# and updates within ~1 s when toggled. Other values: \"dark\", \"light\".
theme = \"system\"

# Optional fonts. Names are matched against installed system fonts
# (case-insensitive). Empty / missing keys fall through to the bundled
# JetBrains Mono Nerd Font + Segoe UI / Cascadia chain. Examples:
#   proportional = \"Segoe UI\"
#   monospace    = \"Cascadia Code\"
#   fallbacks    = [\"Segoe UI Emoji\", \"Symbols Nerd Font\"]
[font]
proportional = \"\"
monospace = \"\"
fallbacks = []
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupPosition {
    #[default]
    Tray,
    TopLeft,
    TopCenter,
    TopRight,
    LeftCenter,
    Center,
    RightCenter,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl PopupPosition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tray => "tray",
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::LeftCenter => "left_center",
            Self::Center => "center",
            Self::RightCenter => "right_center",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    pub const ALL: &'static [PopupPosition] = &[
        Self::Tray,
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::LeftCenter,
        Self::Center,
        Self::RightCenter,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    /// Follow the Windows app-mode setting (`AppsUseLightTheme` registry
    /// key) and update on change.
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub const ALL: &'static [ThemeChoice] = &[Self::System, Self::Dark, Self::Light];
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Preferred proportional family. Empty = use bundled defaults.
    pub proportional: String,
    /// Preferred monospace family. Empty = use bundled defaults.
    pub monospace: String,
    /// Additional families to append to both family chains as
    /// fallbacks (good for emoji / nerd glyphs / scripts).
    pub fallbacks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub chime: bool,
    pub speak: bool,
    pub popup_position: PopupPosition,
    pub theme: ThemeChoice,
    pub font: FontConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chime: true,
            speak: true,
            popup_position: PopupPosition::Tray,
            theme: ThemeChoice::System,
            font: FontConfig::default(),
        }
    }
}

/// In-process patches that the GUI sends to mutate `config.toml`. Each
/// variant maps to one TOML key.
#[derive(Debug, Clone, Copy)]
pub enum ConfigPatch {
    Chime(bool),
    Speak(bool),
    PopupPosition(PopupPosition),
    Theme(ThemeChoice),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
}

/// Load `config.toml` from disk. Missing file → `Config::default()`.
/// Parse errors propagate so the caller can decide whether to fall
/// back or surface them.
pub fn load(path: &Path) -> Result<Config, LoadError> {
    let contents = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => {
            return Err(LoadError::Io {
                path: path.display().to_string(),
                source: e,
            });
        }
    };
    toml::from_str(&contents).map_err(|source| LoadError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Apply a single `ConfigPatch` to the TOML text, preserving comments,
/// key order, whitespace, and unknown keys. Pure function — no
/// filesystem. The disk-side wrapper ([`apply_patch`]) handles
/// atomic write.
pub fn apply_patch_to_string(input: &str, patch: ConfigPatch) -> Result<String> {
    let mut doc: DocumentMut = input.parse().context("parsing TOML for patch")?;
    match patch {
        ConfigPatch::Chime(v) => set_bool(&mut doc, "chime", v),
        ConfigPatch::Speak(v) => set_bool(&mut doc, "speak", v),
        ConfigPatch::PopupPosition(p) => set_str(&mut doc, "popup_position", p.as_str()),
        ConfigPatch::Theme(t) => set_str(&mut doc, "theme", t.as_str()),
    }
    Ok(doc.to_string())
}

/// Set a key whose existing value we want to mutate in place,
/// preserving its decor (prefix / trailing comment / surrounding
/// whitespace). Replacing via `doc[key] = value(v)` would discard
/// the trailing comment.
///
/// We don't rely on `Formatted::<T>::value_mut` (whose presence varies
/// across `toml_edit` versions); instead we capture the existing
/// `Value`'s decor, build a fresh `Value` from the new payload, and
/// re-attach the decor.
fn set_value(doc: &mut DocumentMut, key: &str, new: Value) {
    if let Some(Item::Value(existing)) = doc.get_mut(key) {
        let decor = existing.decor().clone();
        let mut new = new;
        *new.decor_mut() = decor;
        *existing = new;
        return;
    }
    doc[key] = Item::Value(new);
}

fn set_bool(doc: &mut DocumentMut, key: &str, v: bool) {
    set_value(doc, key, Value::from(v));
}

fn set_str(doc: &mut DocumentMut, key: &str, v: &str) {
    set_value(doc, key, Value::from(v));
}

/// Persist a single patch to `path`. Creates the file with [`TEMPLATE`]
/// if missing. Writes atomically (temp file + rename) so a crash
/// mid-write doesn't leave a half-flushed file.
pub fn apply_patch(path: &Path, patch: ConfigPatch) -> Result<()> {
    let initial = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => TEMPLATE.to_string(),
        Err(e) => {
            return Err(
                anyhow::Error::new(e).context(format!("reading {} for patch", path.display()))
            );
        }
    };

    let new = apply_patch_to_string(&initial, patch)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &new).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renaming over {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_chime_speak_tray() {
        let c = Config::default();
        assert!(c.chime);
        assert!(c.speak);
        assert_eq!(c.popup_position, PopupPosition::Tray);
    }

    #[test]
    fn parse_all_keys() {
        let toml = r#"
chime = false
speak = false
popup_position = "bottom_right"
"#;
        let c: Config = toml::from_str(toml).expect("parses");
        assert!(!c.chime);
        assert!(!c.speak);
        assert_eq!(c.popup_position, PopupPosition::BottomRight);
    }

    #[test]
    fn parse_missing_keys_falls_back_to_defaults() {
        let c: Config = toml::from_str("").expect("parses");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn parse_partial_keeps_defaults_for_missing() {
        let c: Config = toml::from_str("chime = false").expect("parses");
        assert!(!c.chime);
        assert!(c.speak); // default kept
        assert_eq!(c.popup_position, PopupPosition::Tray);
    }

    #[test]
    fn invalid_popup_position_is_rejected() {
        let result: Result<Config, _> = toml::from_str(r#"popup_position = "moon""#);
        assert!(result.is_err(), "expected parse error, got {result:?}");
    }

    #[test]
    fn apply_patch_preserves_comments_and_unknown_keys() {
        let input = "# leading comment
chime = true  # trailing comment
unknown_key = 42

speak = true
popup_position = \"tray\"
";
        let after = apply_patch_to_string(input, ConfigPatch::Chime(false)).expect("patch");
        assert!(
            after.contains("# leading comment"),
            "leading comment lost: {after}"
        );
        assert!(
            after.contains("# trailing comment"),
            "trailing comment lost: {after}"
        );
        assert!(
            after.contains("unknown_key = 42"),
            "unknown key lost: {after}"
        );
        assert!(
            after.contains("chime = false"),
            "chime not updated: {after}"
        );
        assert!(after.contains("speak = true"), "speak unchanged: {after}");
    }

    #[test]
    fn apply_patch_for_popup_position_writes_kebab_snake_value() {
        let after = apply_patch_to_string(
            "popup_position = \"tray\"\n",
            ConfigPatch::PopupPosition(PopupPosition::BottomRight),
        )
        .expect("patch");
        assert!(
            after.contains("popup_position = \"bottom_right\""),
            "{after}"
        );
    }

    #[test]
    fn apply_patch_byte_identical_when_value_already_matches() {
        // Critical for the watcher feedback-loop guard. If toml_edit
        // reformats the file when we set a key to its current value,
        // every GUI write triggers a fresh debounce cycle.
        let original = "# header\nchime = true\nspeak = true\npopup_position = \"tray\"\n";
        let after = apply_patch_to_string(original, ConfigPatch::Chime(true)).expect("patch");
        assert_eq!(
            original, after,
            "no-op patch must round-trip byte-identical"
        );
    }

    #[test]
    fn config_eq_ignores_formatting_in_source() {
        // Same logical config, different formatting.
        let a: Config =
            toml::from_str("chime=true\nspeak=true\npopup_position=\"tray\"\n").expect("parses");
        let b: Config = toml::from_str(
            "# preface\n  chime   =   true   \nspeak = true\npopup_position = \"tray\"\n",
        )
        .expect("parses");
        assert_eq!(a, b, "Config equality should ignore source formatting");
    }
}
