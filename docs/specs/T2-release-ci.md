# T2 — Release CI workflow

## Goal

Tag a commit `v*`, get a GitHub release with a built `kree.exe` attached.

## Trigger

- `push` events with tags matching `v*` (e.g. `v0.1.0`, `v1.0.0-alpha.1`).
- Optional manual `workflow_dispatch` for testing the workflow itself.

## Workflow shape

`.github/workflows/release.yml` on `windows-latest`:

1. `actions/checkout@v4`
2. `dtolnay/rust-toolchain@stable`
3. `Swatinem/rust-cache@v2`
4. (Optional follow-up — out of scope here) Generate Windows
   versioninfo + icon via the `winres` build dep so Explorer shows
   the right metadata. Not part of this commit.
5. `cargo build --release`
6. Verify the artifact exists at `target/release/kree.exe`.
7. Use `softprops/action-gh-release@v2` to:
   - Create / update the release for the pushed tag.
   - Attach `target/release/kree.exe` and the
     `assets/fonts/LICENSE-*.txt` files.
   - Use auto-generated release notes (`generate_release_notes: true`).
   - Mark prerelease automatically if the tag contains `-` (semver
     pre-release).

## Permissions

`contents: write` only — needed by `action-gh-release` to publish.
No `packages: write`, no other scopes.

## Acceptance

1. Workflow YAML passes `actionlint` (best-effort — we don't run it
   in CI, but the structure should be valid).
2. Pushing `v0.0.0-test` to a fork produces a release with the
   binary attached. (Local verification: dry-run with
   `act` is **not** required given the workflow is pure
   `cargo build` + an action-gh-release step.)
3. `release.yml` lives next to `ci.yml`; existing `ci.yml`
   continues to gate `main` PRs unchanged.

## Out of scope

- Authenticode signing (covered in T6 winget research).
- winget submission automation (T6 follow-up).
- Cross-platform builds.
- Versioninfo / icon embedding via `winres`.
- Source archive uploads — GitHub auto-attaches source zips/tarballs
  to every release.
