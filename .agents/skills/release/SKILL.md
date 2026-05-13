---
name: release
description: Use for preparing kree releases, drafting terse RELEASE_NOTES.md files, checking release readiness, and explaining or running the tag-based GitHub release flow.
---

# Release

Use this skill for kree release work.

## Release Flow

1. Make sure `trunk` is green.
2. Create `RELEASE_NOTES.md` for every release.
3. Tag the release commit with `v*`.
4. Push the tag.
5. GitHub Actions builds `kree.exe` and publishes the GitHub Release.

## RELEASE_NOTES.md

`RELEASE_NOTES.md` is for the next release only.

Before drafting a new release:
- Clear old contents.
- Write fresh notes for the new version.
- Keep the file committed with the release prep.

The release workflow publishes the full file above GitHub's auto-generated notes.

## Style

Write for normal users.

Rules:
- No fluff.
- No marketing.
- No implementation details unless users need them.
- No dependency section.
- No "we are excited".
- Max 6 bullets.
- Prefer sections: `Changes`, `Fixes`.
- Do not invent changes.

Template:

```markdown
## Changes

- ...

## Fixes

- ...
```

## Before Tagging

Run:

```powershell
cargo fmt --all -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --all-targets
cargo audit
cargo deny check
git status --short --branch
```
