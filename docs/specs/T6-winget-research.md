# T6 — Winget research

Research-only deliverable. No code change in this commit; the output
is `docs/winget-research.md`, a write-up of:

1. Manifest format: 5 YAML files
   (`<publisher>.<package>.installer.yaml`,
   `.locale.<lang>.yaml`, `.yaml` (default-locale), and the
   per-version directory under `manifests/k/<publisher>/<package>/<version>/`).
2. Submission flow: fork
   [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs),
   add the manifests, open a PR. Validation pipeline lives in the
   repo's GitHub Actions; it expects the installer to be reachable
   over HTTPS and to match a SHA256 hash recorded in the manifest.
3. Signing: winget itself does **not** require Authenticode; the
   manifest can point at an unsigned EXE (the installer hash check
   replaces signing for trust). Microsoft Defender SmartScreen will
   warn on unsigned EXEs run by users — separate concern.
4. Update cadence: the manifests are versioned; bump the version
   string + URL + SHA256 per release, send a new PR. Tools like
   `wingetcreate` automate the bump.
5. Installer wrapper question: portable single-EXE manifests are
   accepted (`Portable` `InstallerType`). No WiX / NSIS needed for
   the v1 submission.

The doc lives at `docs/winget-research.md`. `docs/specs/open-items.md` keeps the
"explore winget" item open since this is research only — actually
publishing the package is a follow-up that shouldn't be done until
the project has a real GitHub home (T1 in `docs/specs/open-items.md`, intentionally
deferred).

## Acceptance

1. `docs/winget-research.md` exists and answers each of the five
   bullets above with concrete references (URLs, file paths,
   action names).
2. `docs/specs/open-items.md` item for winget is updated to point at the doc and
   explicitly marks "actual submission" as deferred until the repo
   has a stable origin.
