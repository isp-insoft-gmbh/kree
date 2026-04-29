# kree — Build Spec

> **For Claude Code:** Read this entire document before writing any code. Then read `CLAUDE.md` for working instructions. Before starting step 1 of the build order, list the exact crate versions you intend to use and wait for the user's approval. Work in small commits — one build-order step per commit. Run `cargo check` and `cargo clippy` after every change. If a step is blocked by a question, **stop and ask** rather than guessing.

---

## 1. Product summary

**kree** (from the Goa'uld for *"attention / listen up"*) is a lightweight Windows tray app that fires user-defined reminders on a cron schedule. Reminders live in a plain text file (one per line). When a reminder triggers it:

1. Plays a bundled sound.
2. Shows a popup anchored above the tray icon.
3. After 2 seconds, if the popup hasn't been dismissed, speaks the text aloud.

Users edit reminders in their preferred editor (resolved from `$VISUAL` → `$EDITOR` → fallback). No config file. Single binary.

Project name: `kree`. Crate name: `kree`. Binary name: `kree.exe`.

## 2. Stack

All crate choices are deliberate. **Lean on crates aggressively** — custom code only for glue and UI.

| Concern | Crate | Notes |
|---|---|---|
| Language | Rust stable, edition 2021 | |
| GUI | `eframe` / `egui` | Immediate-mode, ships well as single binary |
| Tray icon | `tray-icon` | |
| Cron parsing | `croner` | POSIX 5-field expressions, actively maintained |
| Async runtime | `tokio` | Features: `rt-multi-thread`, `time`, `sync`, `macros` only |
| Audio playback | `rodio` | Default features (includes vorbis/ogg decoding) |
| TTS | `tts` | Wraps SAPI on Windows. Plan B if it breaks: call SAPI directly via `windows` crate (`ISpVoice::Speak`, ~30 lines). |
| File watching | `notify-debouncer-mini` | Debounces editor-save bursts; raw `notify` triggers 2–5 events per save |
| App data paths | `directories` | Use `ProjectDirs::from("", "", "kree")` |
| Editor launch | `edit` | Handles `$VISUAL`/`$EDITOR`/fallback/PATH lookup/quoted args. Features: `better-path`, `quoted-env`. |
| Single-instance guard | `single-instance` | Named mutex on Windows |
| Autostart | `auto-launch` | Writes/removes `HKCU\...\Run` |
| Logging | `tracing` + `tracing-appender` | Rolling daily file |
| Errors | `anyhow` (app) + `thiserror` (parser) | |
| Grapheme splitting | `unicode-segmentation` | For emoji detection in reminders |
| Emoji detection | `unicode-properties` | Check Emoji property on first grapheme |

**Do not** add a config file, CLI flags, custom DSL, or features beyond this spec without asking the user.

## 3. File layout

Runtime data:

```
%APPDATA%\kree\                   ← resolved via `directories` crate
  reminders.txt                   ← user-editable, hot-reloaded
  logs\
    app.log                       ← rolling daily
```

Project repo layout:

```
.
├── Cargo.toml                    ← package name = "kree"
├── Cargo.lock
├── deny.toml
├── rustfmt.toml
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   └── outdated.yml
│   └── dependabot.yml
├── assets/
│   ├── chime.ogg                 ← CC0 sound, embedded via include_bytes!
│   ├── tray-icon.png             ← 32x32 or 64x64
│   ├── tray-icon-paused.png
│   └── README.md                 ← document asset sources + licenses
├── src/
│   ├── main.rs
│   ├── parser.rs                 ← reminders.txt parser
│   ├── scheduler.rs              ← spawn-per-reminder loop
│   ├── popup.rs                  ← popup window + stacking
│   ├── main_window.rs            ← table view
│   ├── tray.rs                   ← tray menu + state
│   ├── audio.rs                  ← sound + TTS
│   ├── editor.rs                 ← thin wrapper around `edit` crate
│   ├── paths.rs                  ← directories wrapper
│   └── icon.rs                   ← emoji extraction from message
├── tests/
│   └── parser_tests.rs           ← parser unit tests live with parser; integration tests here if needed
├── SPEC.md                       ← this file
├── CLAUDE.md                     ← instructions for Claude Code
└── README.md                     ← user-facing
```

## 4. Reminder file format

```
# Lines starting with # are comments. Blank lines ignored.
# Format: <cron-expression> | <message>
# The message may start with an emoji (use Win+. picker) which becomes the popup icon.

*/30 9-17 * * 1-5    | 💧 Drink water
0 17 * * 1-5         | 🛑 Shutdown ritual
0 9 * * 1            | 📋 Weekly review
0 0 * 9-17 * *       | 🧘 Stand and stretch
*/15 * * * *         | Plain reminder with no emoji
```

### Parser rules

- Trim each line.
- Skip blank lines and lines starting with `#`.
- Split on the **first** `|`. Both sides trimmed.
- If no `|` present → log warning with line number, skip.
- If cron expression doesn't validate via `croner::CronParser` → log warning with line number + parser error, skip.
- If message is empty after trim → log warning, skip.
- Valid lines become scheduled reminders.
- **The parser never panics. Bad lines never block good lines.**

### Icon handling

- Use `unicode-segmentation` to extract the first grapheme cluster of the message.
- Use `unicode-properties` to check whether that grapheme has the Emoji property.
- If yes: that grapheme is the reminder's icon; the message body is everything after it (left-trimmed).
- If no: icon defaults to `🔔` and the message is unchanged.
- Icon is rendered prominently in the popup; the body is the smaller text below or beside it.

## 5. Core behavior

### 5.1 Background scheduler

- On start, parse `reminders.txt` and spawn one `tokio` task per valid reminder.
- Each task loops: `find_next_occurrence(now) → sleep_until(next) → send ReminderEvent → repeat`.
- **Always recompute `find_next_occurrence` from `now`, never chain from the prior scheduled time.** This handles laptop sleep/suspend correctly — after wake, you fire once for the next future occurrence rather than firing late or losing it.
- Events flow over a `tokio::sync::mpsc` channel to the UI thread.
- `notify-debouncer-mini` watches `reminders.txt`. On change: cancel all reminder tasks, re-parse, re-spawn.

### 5.2 Fire sequence

Per `ReminderEvent`:

1. Play bundled sound on a worker thread (`rodio`, non-blocking).
2. Show popup above tray icon immediately.
3. Start a 2-second timer. When it expires: if popup is still visible (not dismissed), call TTS to speak the message body. If dismissed before 2s, no speech.

### 5.3 Tray icon

- **Left-click:** toggle main window.
- **Right-click menu:**
  - Pause All / Resume (toggles based on state)
  - Edit Reminders
  - Reload
  - Open Log
  - Quit
- Icon swaps to the paused variant when paused.
- "Edit Reminders" calls `edit::get_editor()` and spawns it with `reminders.txt`. The `edit` crate handles terminal-vs-GUI editors, PATH resolution, and quoted env vars. We don't need to detect terminal editors ourselves.

### 5.4 Popup window

- Frameless, always-on-top, no taskbar entry, no focus steal.
- Anchored above the tray icon. Get the icon's screen rect from `tray-icon`. **Fallback if rect is unavailable:** use the bottom-right of the primary monitor's work area as the anchor.
- Content:
  - Icon (large) — emoji from message, or default 🔔
  - Body text
  - Fire timestamp (small, muted)
  - Single **Dismiss** button
- **Stacking:** when multiple popups are visible at once, stack them vertically upward. Each new popup's bottom edge sits 8px above the previous popup's top edge.
- Auto-dismiss after 30 seconds. Hardcoded; not configurable.
- Subtle fade in/out (egui `Animation` or manual lerp on alpha).

### 5.5 Main window

- Opened from tray left-click or "Edit Reminders" menu (the latter also opens the editor).
- Closing the window hides to tray. Does not quit. Quit only via tray menu.
- Contents:
  - Table: schedule | message | icon | next fire time | last fired
  - Buttons: **Edit Reminders**, **Reload**, **Pause / Resume**
  - Autostart toggle (writes/removes `HKCU\...\Run` via `auto-launch`)

### 5.6 Single-instance guard

- At startup, attempt to acquire a named mutex via `single-instance` (e.g. `"kree-singleton-{user}"`).
- If another instance holds it: log a line, exit 0 silently. Don't show an error dialog.

### 5.7 Logging

- `tracing-subscriber` configured with a rolling daily file appender at `<data_dir>/logs/app.log`.
- Default level: `info`. Honor `RUST_LOG` if set.
- Parser warnings, scheduler events, popup fires, errors all logged.

## 6. Sound asset

Bundle one short, pleasant chime. Requirements:

- ≤ 500 ms.
- ≤ 50 KB.
- `.ogg` (Vorbis), so `rodio` decodes it with default features.
- **CC0 license** (public domain). Source from freesound.org or similar.
- Loaded with `include_bytes!("../assets/chime.ogg")` and decoded by `rodio` from `Cursor`.

**Action item:** download a CC0 chime, commit to `assets/chime.ogg`, document its source URL and license in `assets/README.md`. If you can't find a suitable CC0 chime, ask the user before substituting another license.

## 7. CI

GitHub Actions, `windows-latest` runner.

Workflows:

- `.github/workflows/ci.yml` — runs on every push/PR:
  1. Build & test — `cargo build --release`, `cargo test --all-features`.
  2. Lint — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
  3. Security audit — `cargo-audit` against RustSec advisory DB. Fails on vulnerabilities.
  4. Supply chain — `cargo-deny check` (licenses, bans, advisories, sources). Uses `deny.toml`.
- `.github/workflows/outdated.yml` — scheduled weekly, opens/updates an issue if any deps are behind. Does not fail main CI.
- `.github/dependabot.yml` — covers `cargo` and `github-actions`, weekly cadence.

Use `Swatinem/rust-cache@v2` for build caching.

## 8. Build order (one commit per step)

Each step must end in a green `cargo check` and `cargo clippy`. Run the test suite if applicable.

1. **Skeleton** — `cargo new kree`, populate `Cargo.toml` with all crate dependencies (versions to be approved by user before this step), CI scaffold, `deny.toml`, `dependabot.yml`. Empty `main` that prints "kree". Green CI.
2. **Single-instance guard** — `single-instance` mutex check at startup. If held, log + exit 0.
3. **Logging** — `tracing` + `tracing-appender` writing to `<data_dir>/logs/app.log` (rolling daily). Resolve `<data_dir>` via `directories`.
4. **Parser** — `src/parser.rs` with full unit tests covering: valid line, comment, blank, missing `|`, invalid cron, empty message, leading-emoji message, plain message (default icon). Use `croner` for cron validation, `unicode-segmentation` + `unicode-properties` for icon extraction.
5. **Scheduler** — `src/scheduler.rs`. Spawn one tokio task per valid reminder, looping `find_next_occurrence(now) → sleep_until → send event`. UI thread receives events and logs them. No UI yet.
6. **Tray icon** — `src/tray.rs`. `tray-icon` with quit menu item. Verify event loop coexists with scheduler. App can now be quit via tray.
7. **Sound on fire** — `src/audio.rs`. `rodio` plays the embedded chime when an event arrives.
8. **TTS with 2s delay** — Add SAPI speech via `tts` crate. After fire, schedule a 2-second timer; if popup is still visible (placeholder for now — popup arrives next step), speak. For this step, "popup visible" can be a stub that always returns true.
9. **Popup window** — `src/popup.rs`. Frameless, always-on-top, anchored above tray. Renders icon + body + timestamp + Dismiss. Auto-dismiss at 30s. Wire 2s TTS check to actual popup state.
10. **Popup stacking** — Multiple concurrent popups stack vertically.
11. **Main window** — `src/main_window.rs`. Table view, Edit/Reload/Pause/Resume buttons, autostart toggle.
12. **Editor launch** — `src/editor.rs`. Wrap `edit::get_editor()`, spawn on `reminders.txt`. Wire to tray "Edit Reminders" and main window button.
13. **Hot reload** — `notify-debouncer-mini` on `reminders.txt`. On change: cancel all scheduler tasks, re-parse, re-spawn.
14. **Autostart** — `auto-launch` toggle in main window.
15. **Polish** — tray icons (active + paused), fade animations, README, screenshots if you want.

## 9. Non-goals (do not build)

- Config file or CLI flags.
- Per-reminder sounds (single global chime only).
- Snooze.
- Cloud sync, mobile, calendar integration.
- Custom cron DSL.
- Real Windows Service (`services.msc`) — Session 0 isolation prevents UI; tray-app-with-autostart is the correct architecture for this product. **Do not build a service.**

## 10. Open questions Claude Code should ask before starting

If anything in this spec is ambiguous, stop and ask before writing code. The user wants to be consulted on:

- Specific crate version pins for the initial `Cargo.toml`.
- Choice of CC0 chime sound (link the candidate before committing the asset).
- Whether the popup design (icon-left, text-right vs icon-top, text-bottom) matches the user's mental picture — show a wireframe in markdown if uncertain.

End of spec.

---

*Jaffa, kree!*
