# TODO

Open follow-ups not covered by the spec or the current commit history.

- [ ] **Push to the `isp-insoft` GitHub org.** Add a `git remote` for the
  ISP Insoft repo, push `main`, switch CI from "windows-latest scratch"
  to the org's standard runner pool if applicable, and replace the
  `../../issues` placeholder in `README.md` with a real link.
- [ ] **Explore publishing as a winget package.** Investigate
  [winget-pkgs](https://github.com/microsoft/winget-pkgs) submission
  flow: manifest format, signing requirements (Authenticode), update
  cadence, and whether the binary needs a stable installer wrapper
  (e.g. WiX / NSIS) or whether a portable single-EXE manifest is
  acceptable.
- [ ] **Build release binaries in a GitHub Actions workflow.** New
  `.github/workflows/release.yml` triggered on tag (`v*`): builds
  `kree.exe` with `cargo build --release` on `windows-latest`,
  uploads as a release asset, and (eventually) feeds the winget
  submission above. Decide whether to embed a Windows manifest /
  versioninfo via `winres` so the EXE shows a real version + icon
  in Explorer.
- [ ] **Theme: dark / light / system.** Add a Theme selector to the
  main window (and persist it — small JSON in `%APPDATA%\kree\` or
  via egui's `Memory::set_persistent`). egui already supports
  `Visuals::dark()` / `Visuals::light()`; "system" means watching
  the Windows app-mode registry key
  (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`)
  and updating on change.
- [ ] **`config.toml` in `%APPDATA%\kree\` + GUI editor.** Add a small
  user config file. Initial keys:
  - `chime = true | false` — play the bundled chime on fire
  - `speak = true | false` — TTS the body 2 s after fire if popup
    still visible
  - `popup_position = "top_left" | "top_center" | "top_right" |
    "left_center" | "center" | "right_center" | "bottom_left" |
    "bottom_center" | "bottom_right" | "tray"` — where new popups
    appear; "tray" is the current default (anchored above the tray
    icon, screen-corner fallback)

  All three are also editable via the main window. **Critical:** GUI
  changes must *mutate* `config.toml` in place, not overwrite it —
  preserve comments, key order, unknown keys, and formatting. Use
  `toml_edit` (not the lossy `toml` crate) for the round-trip. Hot
  reload via the existing `notify-debouncer-mini` plumbing so manual
  edits to `config.toml` apply without a restart.
- [ ] **Bundle a comprehensive font.** egui ships with `Hack` +
  `NotoEmoji` only — many modern emoji and all Nerd Font icons render
  as tofu (□). Embed a Nerd Font (e.g. JetBrainsMono Nerd Font Mono
  Regular, OFL) plus a broader emoji font, register them via
  `egui::FontDefinitions`, and prepend them in the proportional +
  monospace family chains. Also support an override in
  `%APPDATA%\kree\fonts\` so users can drop their own.
- [ ] **Optimized release profile.** Add a `[profile.release]`
  block to `Cargo.toml` with `lto = "fat"`, `codegen-units = 1`,
  `strip = "symbols"`, `panic = "abort"`, and `opt-level = "z"`
  (size-first — kree is GUI / I/O bound, not hot-loop CPU).
  Measure before / after on `kree.exe`; expect a meaningful drop
  from the current debug-build size. Land alongside the release
  CI item so the workflow benefits.
