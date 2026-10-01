# GitHub Actions

Read `../docs/specs/spec.md` and `../CLAUDE.md` before changing workflows.

- Preserve the required check names `Build & Test`, `Lint (fmt + clippy)`, and `Supply chain (cargo-deny)` plus their meaningful execution. Update branch protection in the same change if a rename or removal is explicitly approved.
- Keep Windows work on the configured Blacksmith Windows runner. Keep platform-independent work on standard GitHub-hosted Ubuntu runners.
- Grant each workflow only the permissions it needs.
- Pin every external action to a full commit SHA and retain its upstream tag or tracked branch in a comment.
- Give every job a measured `timeout-minutes` limit.
- Cancel superseded pull-request work, never trunk validation or release publication.
- Prefer checks for status, annotations for actionable problems, and `GITHUB_STEP_SUMMARY` for concise human reports.
- Use outputs only for small machine-readable values. Use artifacts only when data must be downloaded or passed between jobs.
- Publish durable binaries as GitHub Release assets, not Actions artifacts.
- Save Rust caches only from `trunk`. Avoid per-pull-request and per-tag cache writes. Keep a cache only when its key allows measured reuse by that job.
- Preserve trust boundaries: untrusted pull-request code gets read-only credentials and cannot consume privileged secrets.
