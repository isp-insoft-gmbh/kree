# Winget submission — research notes

Research deliverable for [T6](specs/T6-winget-research.md). Goal: enough
detail that a follow-up commit can produce a real submission once kree
has a stable GitHub origin.

## 1. Manifest format

Five YAML files per version, all under
`manifests/k/<publisher>/<package>/<version>/`:

| file                                | purpose                                            |
|-------------------------------------|----------------------------------------------------|
| `<publisher>.<package>.yaml`        | "version manifest" — points at the locale + installer manifests by ID. |
| `<publisher>.<package>.installer.yaml` | URL(s), SHA256, architecture, install switches.    |
| `<publisher>.<package>.locale.<tag>.yaml` | One per locale (e.g. `en-US`, `de-DE`).            |
| `<publisher>.<package>.locale.en-US.yaml` (default) | The "default-locale" manifest — required.          |

Schema reference:
<https://github.com/microsoft/winget-cli/blob/master/doc/ManifestSpecv1.10.0.md>

For kree, plausible coordinates:

```
PackageIdentifier: ISPInSoft.kree
PackageVersion:    0.1.0
Publisher:         ISP InSoft
PackageName:       kree
ShortDescription:  Cron-scheduled reminders in your Windows tray.
License:           MIT
Moniker:           kree
Tags:
  - reminders
  - tray
  - cron
  - rust
  - stargate
```

## 2. Submission flow

1. Fork [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).
2. Add the five YAMLs at the path above.
3. Open a PR.
4. Validation runs as GitHub Actions in the upstream repo
   (`Service-Schema-Validation`, `winget-validation` jobs). The pipeline:
   - downloads the URL listed in the installer manifest
   - hashes it, compares against the manifest's `InstallerSha256`
   - runs `winget validate` on the manifests
   - posts a comment with success / failure
5. On green, a maintainer merges. The package shows up in the public
   community repo within a few hours.

`wingetcreate` (`https://learn.microsoft.com/windows/package-manager/winget/wingetcreate`)
automates manifest authoring and PR creation. Recommended for the
first submission and every version bump.

## 3. Signing

**Winget itself does not require Authenticode.** The chain of trust is:

1. Manifest pins an HTTPS URL.
2. Manifest pins a SHA256.
3. Validation downloads + verifies the hash.

Unsigned EXEs install fine. The user-facing pain is **Microsoft Defender
SmartScreen**, which warns on unsigned binaries the first time they run.
SmartScreen reputation is built up by:

- using an EV (extended-validation) Authenticode certificate for
  immediate reputation, or
- a regular code-signing cert plus accumulated user installs over weeks.

For a first kree release, ship unsigned and accept the SmartScreen
warning. Document it in the user manual. Re-evaluate signing if usage
grows.

## 4. Update cadence

- Every release: bump the `PackageVersion`, point `InstallerUrl` at the
  new GitHub release asset, bump `InstallerSha256`, open a new PR.
- `wingetcreate update <id> --urls <url> --version <ver> --submit` does
  the whole bump + PR in one shot. We can wire this into a follow-up
  GitHub Actions job after a release publishes; out of scope for the
  current release CI workflow (T2).

## 5. Installer wrapper

`InstallerType: portable` is accepted by winget. With it set, no MSI /
EXE installer wrapper is needed:

```yaml
InstallerType: portable
Commands:
  - kree
PortableCommandAlias:
  - kree
```

Winget handles registration of the binary as a "portable" install
(adds an entry under `%LOCALAPPDATA%\Microsoft\WinGet\Packages\` and
exposes the alias on `PATH`). Uninstall removes the alias and the
folder.

For kree this is the right default — single self-contained EXE, no
DLLs, no per-machine state. We can revisit MSI wrapping later if we
need shell integration (right-click install, custom uninstaller UI),
but neither is needed today.

## 6. Open questions deferred until kree has a GitHub home

- **PackageIdentifier prefix.** `ISPInSoft.kree` assumes that's the
  publisher namespace. If the org uses a different convention on
  winget-pkgs, mirror it.
- **EV vs OV cert.** Future call. SmartScreen reputation tracking
  requires a published cert; the cert lives outside the repo.
- **Auto-bump CI.** Once kree has a GitHub remote and a release
  workflow that publishes a `kree.exe` asset, a follow-up workflow can
  use `wingetcreate update --submit` keyed on the release published
  event.

## Status

Research only. Actual submission deferred until kree has a stable
GitHub origin (TODO.md item: "Push to the `isp-insoft` GitHub org").
TODO.md updated to point at this doc.
