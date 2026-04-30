//! Resolve user-configured family names against the installed font set.
//!
//! Two well-known directories on Windows:
//!
//! - `C:\Windows\Fonts\` — system-wide installs.
//! - `%LOCALAPPDATA%\Microsoft\Windows\Fonts\` — per-user installs.
//!
//! The index is built lazily on first use via `OnceLock` and cached for
//! the lifetime of the process. **Known limitation**: fonts installed
//! after kree starts are *not* picked up — restart kree to re-scan.
//! That's acceptable for the rare-edit case and avoids re-walking the
//! disk on every config reload.
//!
//! Match is case-insensitive on the family name. We pick face index 0
//! for TTC collections (which contain multiple faces per file) — a
//! future TODO can expose a sub-key to choose a different face.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{debug, warn};

static FONT_INDEX: OnceLock<HashMap<String, PathBuf>> = OnceLock::new();

/// Returns a process-lifetime path to a font file matching `family`,
/// or `None` if no installed font advertises that family name.
pub fn resolve_family(family: &str) -> Option<&'static Path> {
    if family.is_empty() {
        return None;
    }
    let key = family.to_lowercase();
    font_index().get(&key).map(|p| p.as_path())
}

fn font_index() -> &'static HashMap<String, PathBuf> {
    FONT_INDEX.get_or_init(scan_installed_fonts)
}

fn scan_installed_fonts() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let mut total = 0usize;
    let mut indexed = 0usize;
    for dir in font_dirs() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                warn!(path = %dir.display(), error = %e, "could not read font dir; skipping");
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_supported_font(&path) {
                continue;
            }
            total += 1;
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if let Some(name) = read_family_name(&bytes) {
                let key = name.to_lowercase();
                map.entry(key).or_insert(path);
                indexed += 1;
            }
        }
    }
    debug!(total, indexed, "system fonts scanned");
    map
}

fn font_dirs() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(r"C:\Windows\Fonts")];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    out
}

fn is_supported_font(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(OsStr::to_str) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc")
}

/// Read the family name from a font file's `name` table. Prefers the
/// "typographic family" record (ID 16) over the "family" record
/// (ID 1) per OpenType convention; both restricted to platform 3
/// (Windows) so we get the canonical English-ish name on the rare
/// font that ships multiple platform variants.
///
/// Returns `None` for malformed fonts, name-tableless fonts (rare),
/// or families with non-UTF-16 records that can't be decoded.
pub fn read_family_name(bytes: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let mut typographic: Option<String> = None;
    let mut family: Option<String> = None;
    for name in face.names() {
        if name.platform_id != ttf_parser::PlatformId::Windows {
            continue;
        }
        match name.name_id {
            16 if typographic.is_none() => typographic = name.to_string(),
            1 if family.is_none() => family = name.to_string(),
            _ => {}
        }
    }
    typographic.or(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_returns_none() {
        assert!(resolve_family("").is_none());
    }

    #[test]
    fn missing_family_returns_none() {
        assert!(resolve_family("Definitely Not A Real Family 12345").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn segoe_ui_resolves_on_windows() {
        let path = resolve_family("Segoe UI")
            .expect("Segoe UI is present on every supported Windows install");
        let s = path.to_string_lossy().to_ascii_lowercase();
        assert!(s.ends_with(".ttf"), "expected .ttf, got {path:?}");
        // Must come from one of the two known font dirs.
        assert!(
            s.contains(r"\fonts\"),
            "expected a Fonts-dir path, got {path:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn read_family_name_recognizes_segoe_ui() {
        let bytes =
            std::fs::read(r"C:\Windows\Fonts\segoeui.ttf").expect("segoeui.ttf present on Windows");
        let name = read_family_name(&bytes).expect("name table parses");
        // Different locales / Windows builds have used "Segoe UI" or
        // "Segoe UI Variable" — assert the prefix.
        assert!(
            name.starts_with("Segoe UI"),
            "expected Segoe UI prefix, got {name:?}"
        );
    }
}
