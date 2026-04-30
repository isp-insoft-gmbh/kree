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
- [x] **Build release binaries in a GitHub Actions workflow.**
  Implemented as T2. `.github/workflows/release.yml` runs on tag
  pushes (`v*`), builds via T1's size-first profile, attaches the
  binary + bundled-font licenses to the release. Authenticode
  signing + winres versioninfo deliberately out of scope — see
  `docs/winget-research.md` for the SmartScreen story.
- [x] **Theme: dark / light / system.** Implemented as T4.
  Selector lives in the main-window settings panel; "system"
  polls `HKCU\...\AppsUseLightTheme` once per second and applies
  within ≤ 2 s of a Windows toggle. Light variant of nugu mapped
  from `output/light.toml`.
- [x] **`config.toml` + GUI editor.** Implemented across
  T3 steps 1-3 (`docs/specs/T3-config-toml.md`). Three keys —
  `chime`, `speak`, `popup_position` (10 anchor values) — round-trip
  via `toml_edit` preserving comments / order / unknown keys; GUI
  edits and hand-edits share the same hot-reload pipeline; a
  feedback-loop guard skips re-broadcast when our own atomic write
  echoes back through the watcher.
- [x] **Pick fonts from installed system fonts via `config.toml`.**
  Implemented as T5. `config.font.{proportional,monospace}` plus
  `fallbacks` resolve against an `OnceLock`-cached scan of
  `C:\Windows\Fonts` + `%LOCALAPPDATA%\Microsoft\Windows\Fonts`,
  using `ttf-parser` to read each font's name table. Bundled
  JetBrains Mono Nerd Font + the hard-coded Segoe / Cascadia
  chain remain as the secondary fallback.
- [x] **Optimized release profile.** Implemented as T1. Local
  measurements (rustc 1.95 / Win11): 19.5 MB baseline → 11.9 MB
  with `lto=fat` + `codegen-units=1` + `strip=symbols` +
  `panic=abort` + `opt-level=s`. Picked `s` over `z` after
  comparing both — `s` shipped 667 KB smaller because some of
  egui's hot tessellator paths regressed under `z`.
