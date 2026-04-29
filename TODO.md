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
