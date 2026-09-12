# Instructions for Claude Code — kree

This file is read by Claude Code at the start of every session in this repo. It defines how to work, not what to build. For *what* to build, read `docs/specs/spec.md`.

## Workflow rules

1. **Read `docs/specs/spec.md` in full before writing any code.** Don't skim. The spec is the source of truth.
2. **Work in small commits.** One build-order step from `docs/specs/spec.md` § 8 per commit. Don't combine steps.
3. **After every code change, run:**
   - `cargo check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test` (if tests are relevant to the change)
   Do not commit if any of these fail. On Linux these do not work as written —
   see "Local verification on Linux" below for the cross-compiled equivalents.
   Run them; do not push unverified work and let CI find it.
4. **Before starting build-order step 1**, propose the exact crate versions you intend to pin in `Cargo.toml` and wait for the user's approval. Don't guess versions; check crates.io for the current stable.
5. **Stop and ask** when blocked or uncertain. Don't guess at:
   - Ambiguities in the spec
   - Choice of asset (sound file URL, icon style)
   - Design decisions not covered in the spec
   - Whether to add a feature beyond the spec
6. **Do not add features outside the spec** without asking. No config files, CLI flags, telemetry, analytics, or "nice-to-haves" unless explicitly approved.
7. **Lean on crates.** If a problem has a well-maintained crate, use it. Custom code is for glue and UI only. The spec lists the chosen crates — use those, don't substitute without asking.

## Local verification on Linux

CI runs on `windows-latest`, but the whole suite — check, clippy and the tests
— can be run from a Linux box against the real Windows target. Do that before
pushing rather than using CI as the first checker.

Setup, once per machine:

```sh
rustup target add x86_64-pc-windows-gnu
pip install ziglang cargo-zigbuild
printf '#!/bin/sh\nexec python3 -m ziglang "$@"\n' > /usr/local/bin/zig
chmod +x /usr/local/bin/zig
apt-get install -y --no-install-recommends wine64
```

Then, per change:

```sh
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=/usr/lib/wine/wine64
export WINEDEBUG=-all

cargo fmt --check
cargo-zigbuild check  --all-targets --target x86_64-pc-windows-gnu
cargo-zigbuild clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo-zigbuild test                 --target x86_64-pc-windows-gnu
```

`cargo fmt` is host-native and needs none of this.

Why the indirection: a plain `cargo check` on Linux fails because `eframe` has
no backend for the host, and cross-compiling to `x86_64-pc-windows-msvc` dies
in `alloca`'s build script, which shells out to MSVC's `lib.exe`. `zig cc`
supplies a mingw C toolchain that the build scripts accept, and `wine` runs the
resulting test binaries.

**Caveat:** this targets `*-windows-gnu` while CI targets `*-windows-msvc`. The
only conditional compilation in our source is a bare `cfg(windows)`, true for
both, so the gap is confined to dependency internals and linking. Close enough
to catch our own mistakes; CI remains the authority on a green build.

## Commit message style

```
<area>: <imperative summary>

<optional body explaining why>
```

Examples:
- `parser: handle leading-emoji messages`
- `scheduler: recompute next fire from now after each event`
- `ci: add cargo-deny job`

Reference the build-order step in the body when relevant: "Implements step 4 of docs/specs/spec.md § 8."

## Code style

- Run `cargo fmt` before every commit. `rustfmt.toml` is in the repo root.
- Keep modules focused. The file layout in `docs/specs/spec.md` § 3 reflects the intended module boundaries — follow it.
- Prefer `anyhow::Result<T>` in application code. Use `thiserror`-derived error types only in the parser (`src/parser.rs`).
- Log at `info` for normal events, `warn` for parser issues and recoverable problems, `error` for actual failures.
- No `unwrap()` or `expect()` in non-test code unless the invariant is documented in a comment.
- No `unsafe` without explicit user approval.

## Testing

- Unit tests live in the same file as the code under test (`#[cfg(test)] mod tests { ... }`).
- The parser must have full coverage of all cases listed in `docs/specs/spec.md` § 4.
- UI rendering itself does not need tests. UI *logic* (e.g. popup stacking math) does.

## What to do when something in the spec doesn't work

If during implementation you discover a constraint or bug that contradicts the spec — for example, the `tts` crate doesn't compile on the target Windows version, or `tray-icon` doesn't expose the rect we need — **stop, document the problem, and ask the user**. Don't silently work around it. The user wants to know about these tradeoffs.

The spec already flags two known fragile spots:
- **`tts` crate**: if it breaks, Plan B is direct SAPI via the `windows` crate (`ISpVoice::Speak`).
- **Tray rect**: if `tray-icon` doesn't expose the icon's screen rect, fall back to bottom-right of primary monitor work area.

For anything else not flagged, ask.

## Don'ts

- Don't write a real Windows Service. The spec explains why (Session 0 isolation). Tray app with autostart is the correct architecture.
- Don't invent a custom cron DSL. Standard 5-field cron via `croner` only.
- Don't add a config file. Behavior that would be configurable is hardcoded by design.
- Don't introduce additional async runtimes (no `async-std`, no `smol`). Tokio only.
- Don't pull in heavyweight crates we don't need (e.g. `serde` is fine where used, but don't add it for its own sake).
