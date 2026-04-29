# T3 — `config.toml` + GUI editor

## Goal

User-tunable runtime knobs. Editable from the GUI **and** from a text
editor. Round-tripping must be lossless: GUI changes mutate the file
in place, preserving comments / order / unknown keys.

## File location

`%APPDATA%\kree\config.toml` (next to `reminders.txt`).

## Initial keys (this commit)

```toml
# Play the bundled chime when a reminder fires.
chime = true

# Speak the body via TTS 2 s after fire if the popup is still visible.
speak = true

# Where new popups appear. "tray" anchors above the tray icon (with
# screen-corner fallback when unavailable).
popup_position = "tray"
```

`popup_position` enum:
`tray`, `top_left`, `top_center`, `top_right`,
`left_center`, `center`, `right_center`,
`bottom_left`, `bottom_center`, `bottom_right`.

## Read path (parse, lossy is OK)

- `serde` + `toml` for *reading* into a strongly-typed `Config`
  struct with defaults.
- Missing keys → defaults. Unknown keys → preserved on write (see
  below) but not surfaced to the typed struct.

```rust
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub chime: ChimeOn,
    pub speak: SpeakOn,
    pub popup_position: PopupPosition,
}
```

(Wrap booleans in newtype with `Default::default = true` so derive works
with `#[serde(default)]`. Or use `#[serde(default = "fn")]`.)

## Write path (mutate, lossless)

- `toml_edit` crate for *writing*. Round-trip preserves:
  - comments (line + trailing)
  - key order
  - whitespace
  - unknown keys / sections
- The GUI sends `ConfigPatch { key, value }` to the tokio side; the
  tokio side opens `config.toml`, deserializes via `toml_edit`,
  mutates the single key, writes atomically (temp file + rename).
- If the file doesn't exist on first GUI write, seed with a
  documented template (mirrors the example block above).

## Hot reload

- `notify-debouncer-mini` watcher (already wired for `reminders.txt`).
  Add the config path to the watch set; same reload signal channel.
- On reload: re-parse, push a `Snapshot { reminders, config }` to the
  UI. (Currently the snapshot is `Vec<Reminder>` — extend to a
  small struct.)

## GUI

Add a "Settings" section above the reminders table in `main_window.rs`:

- `Chime`: checkbox.
- `Speak`: checkbox.
- `Popup position`: combo box (10 values, including `tray`).

Each control change sends a `MainWindowAction::SetConfig(ConfigPatch)`
to the App, which forwards through tokio → `toml_edit` → file.

## Use sites

- `audio::play_chime()` — caller passes `if config.chime { play_chime() }`.
- 2 s TTS task — `if config.speak { ... }`.
- `popup::compute_position` — branch on `config.popup_position`. The
  10-anchor enum maps to a screen-rect anchor. `tray` keeps the
  current behavior (rect from `tray-icon`, fallback bottom-right).

## Tests

Pure-logic functions get unit tests:

- `Config` parses with all keys present.
- `Config` parses with all keys missing (uses defaults).
- `Config` rejects an invalid `popup_position` value (one negative
  test; `serde` does the heavy lifting).
- `apply_patch_preserving_comments` round-trip:
  given a TOML with `# comment` lines + `unknown_key = 1`, mutate
  `chime = false`, expect output retains the comments + unknown key
  unchanged.
- `popup::compute_position` returns expected coordinates for the
  9 fixed anchors against a fake 1920×1080 work area.

## Acceptance

1. Defaults compile-in: a brand-new `%APPDATA%\kree\config.toml`
   doesn't need to exist; defaults match the table above.
2. Editing `config.toml` by hand applies on save (≤ 500 ms debounce).
3. GUI toggles persist to disk and survive a restart.
4. Hand-edited comments survive a GUI write — verified by a unit
   test, not by hand.
5. Unknown keys survive a GUI write.
6. Popup at `bottom_right` lands within 16 px of the bottom-right
   corner of the primary monitor work area.

## Risks

- `toml_edit` on Windows file handles plus our existing watcher could
  produce reload feedback loops (we write → watcher fires → reparse
  → no diff → no further write — safe). Verify by ensuring the writer
  side doesn't push a `Snapshot` that re-triggers a write.

## Out of scope

- `font.*` keys — those land in T5.
- Theme key — lands in T4.
- Per-reminder overrides.
