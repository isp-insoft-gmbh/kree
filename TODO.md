# TODO

Open follow-ups not covered by the spec or the current commit history.

- [ ] **Push to the `isp-insoft` GitHub org.** Add a `git remote` for the
  ISP Insoft repo, push `main`, switch CI from "windows-latest scratch"
  to the org's standard runner pool if applicable, and replace the
  `../../issues` placeholder in `README.md` with a real link.
- [x] **Explore publishing as a winget package.** Research notes
  in [`docs/winget-research.md`](docs/winget-research.md). Summary:
  manifest = 5 YAMLs under `manifests/k/<pub>/<pkg>/<ver>/`,
  `InstallerType: portable` is accepted (no MSI wrapper), winget
  itself doesn't require Authenticode (SmartScreen does — separate
  concern), `wingetcreate update --submit` handles version bumps.
  Actual submission deferred until kree has a GitHub home.
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
- [x] **`config.toml` + GUI editor.** Implemented across
  T3 steps 1-3 (`docs/specs/T3-config-toml.md`). Three keys —
  `chime`, `speak`, `popup_position` (10 anchor values) — round-trip
  via `toml_edit` preserving comments / order / unknown keys; GUI
  edits and hand-edits share the same hot-reload pipeline; a
  feedback-loop guard skips re-broadcast when our own atomic write
  echoes back through the watcher.
- [ ] **Pick fonts from installed system fonts via `config.toml`.**
  Add `font.proportional`, `font.monospace`, `font.fallbacks` keys to
  the planned `config.toml`. Resolve each value to a file by
  enumerating installed fonts (Win32 `EnumFontFamiliesEx` over
  `GetDC(NULL)`, or scan `C:\Windows\Fonts` + `%LOCALAPPDATA%\Microsoft\Windows\Fonts`
  for `*.ttf`/`*.otf`/`*.ttc` and parse the `name` table for the
  family name) and prepend the resolved bytes to the matching
  egui `FontFamily`. Keep the bundled JetBrains Mono Nerd Font as
  the always-present fallback so a missing or mistyped family name
  never tofus the entire UI.
- [ ] **Optimized release profile.** Add a `[profile.release]`
  block to `Cargo.toml` with `lto = "fat"`, `codegen-units = 1`,
  `strip = "symbols"`, `panic = "abort"`, and `opt-level = "z"`
  (size-first — kree is GUI / I/O bound, not hot-loop CPU).
  Measure before / after on `kree.exe`; expect a meaningful drop
  from the current debug-build size. Land alongside the release
  CI item so the workflow benefits.
