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

## Crate versions (per CLAUDE.md rule 4 — pin coarse, list before impl)

Add to `Cargo.toml` `[dependencies]`:

| crate       | pin    | features                | notes |
|-------------|--------|-------------------------|-------|
| `serde`     | `"1"`  | `["derive"]`            | Already in `Cargo.lock` as a transitive. |
| `toml`      | `"1"`  | default                 | Latest is `1.1`; coarse pin per house style. |
| `toml_edit` | `"0.25"` | default               | Format-preserving editor. |

`anyhow` + `thiserror` already declared.

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

### Feedback-loop guard

The atomic write (temp file + rename) the GUI uses **will** fire the
debouncer. Mitigation:

- After a GUI write, the writer task records a `last_self_write_at:
  Instant` timestamp on a shared `Arc<Mutex<Option<Instant>>>`.
- On the reload-rx side, when a reload event arrives, compare the
  parsed `Config` against the in-memory `Config`. If equal, drop the
  event (no re-broadcast, no re-write attempt). Equality check is on
  the typed `Config` struct, so unrelated formatting changes still
  round-trip.

A unit test asserts the no-op behavior: write same config back,
reload event arrives, no `Snapshot` pushed.

## GUI

Add a "Settings" section above the reminders table in `main_window.rs`:

- `Chime`: checkbox.
- `Speak`: checkbox.
- `Popup position`: combo box (10 values, including `tray`).

Each control change sends a `MainWindowAction::SetConfig(ConfigPatch)`
to the App, which forwards through tokio → `toml_edit` → file.

## Use sites (concrete)

Both gates currently fire unconditionally in `src/main.rs::handle_fire`
(audio::play_chime around line 184, audio::speak around line 192). Wire
each gate to the live `Config` snapshot held by the tokio side:

- `handle_fire(runtime, ui_tx, event, &Config)` — accept the config
  by reference.
- `if config.chime { audio::play_chime() }` — line 184 use site.
- `if config.speak { ... audio::speak(...) }` — wraps the spawned 2 s
  TTS task; gating happens inside the spawned task (so the 2 s timer
  + popup-visible check still runs even when speak is off — just no
  speech at the end). Decision: gate at task spawn, skip the timer
  entirely when speak is disabled.

The shared `Config` lives behind `Arc<RwLock<Config>>` on the tokio
side; the reload path replaces it on file change.

### Popup positioning

`popup::compute_position` gains a `position: PopupPosition` arg. The
9 fixed anchors compute coordinates from
`SystemParametersInfoW(SPI_GETWORKAREA)` (we already use this for the
`tray` fallback). `tray` keeps the current logic. Stacking math
(step 10) anchors to the chosen corner — top-anchored positions stack
*downward*, bottom-anchored stack *upward*, center positions stack
upward, sides stack toward the screen center.

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
  9 fixed anchors against a fake 1920×1080 work area. Each corner
  asserts a specific `(x, y)` within 1 px tolerance.
- `apply_patch` writing the *same* value back is idempotent and
  emits no diff (used to verify the feedback-loop guard).
- `Config::eq`: two parsed `Config`s with identical typed values are
  equal, regardless of formatting differences in the source TOML.
  Backs the no-broadcast guard above.
- **Byte-identical no-op round-trip.** Load a fixture with
  comments + unknown keys + odd whitespace + a final-newline,
  write it back without mutating any keys, assert
  `bytes_in == bytes_out`. Off-by-one trailing-newline mismatches
  are the classic failure mode — they'd make every GUI write
  produce a real diff and re-trigger the watcher.

## Acceptance

1. Defaults compile-in: a brand-new `%APPDATA%\kree\config.toml`
   doesn't need to exist; defaults match the table above.
2. Editing `config.toml` by hand applies on save (≤ 500 ms debounce).
3. GUI toggles persist to disk and survive a restart.
4. Hand-edited comments survive a GUI write — verified by a unit
   test, not by hand.
5. Unknown keys survive a GUI write.
6. Popup at `bottom_right` produces a top-left position whose
   `x = work_area.right - POPUP_WIDTH - 16` and
   `y = work_area.bottom - POPUP_HEIGHT - 16`, asserted with 1 px
   tolerance against a faked work-area rect in tests.
7. Toggling `chime = false` in the GUI silences the chime on the
   next fire (verified at runtime, not in tests).
8. Toggling `speak = false` skips the 2 s TTS task entirely on the
   next fire (verified at runtime).

## Risks

- `toml_edit` on Windows file handles plus our existing watcher could
  produce reload feedback loops (we write → watcher fires → reparse
  → no diff → no further write — safe). Verify by ensuring the writer
  side doesn't push a `Snapshot` that re-triggers a write.

## Implementation order (3 commits)

T3 lands as three atomic commits — each green at `cargo clippy
--all-targets -- -D warnings` + `cargo test`. Splitting limits blast
radius if any one step regresses.

1. `config: parse, write, watch` — new `src/config.rs` + `Config`
   plumbed through the existing watcher's reload signal. No
   behavior changes yet; chime/speak/position gates still always
   on at default config.
2. `config: gate chime / speak / popup_position` — wire the
   existing `handle_fire` and `popup::compute_position` use
   sites. Default config behaves identical to current `main`.
3. `main_window: settings panel` — checkboxes + combo box,
   dispatch `ConfigPatch` actions, GUI writes round-trip the
   file.

## Out of scope

- `font.*` keys — those land in T5.
- Theme key — lands in T4.
- Per-reminder overrides.
