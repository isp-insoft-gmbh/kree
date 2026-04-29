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
