# T1 — Optimized release profile

## Goal

Smaller `kree.exe` for shipping. kree is GUI-/IO-bound, not hot-loop CPU,
so size is the right primary axis.

## Change

Add to `Cargo.toml`:

```toml
[profile.release]
lto = "fat"
codegen-units = 1
strip = "symbols"
panic = "abort"
opt-level = "z"
```

Rationale per knob:

| knob | value | why |
|---|---|---|
| `lto` | `"fat"` | Whole-program inlining; biggest single size win, big build-time cost. |
| `codegen-units` | `1` | One unit = better cross-module optimization. Pairs with `lto`. |
| `strip` | `"symbols"` | Removes debug + symbol info from the final binary. |
| `panic` | `"abort"` | No unwinding tables. Saves ~50–100 KB. We have no panic-recovery code. |
| `opt-level` | `"z"` | Optimize for size. `s` is also valid; `z` is more aggressive. |

## Acceptance

1. `cargo build --release` succeeds.
2. `cargo clippy --all-targets --release -- -D warnings` succeeds.
3. `cargo test --release` succeeds (panic=abort is permitted in tests since
   Rust 1.60+ via `-Z panic-abort-tests` is **not** required at stable; tests
   use the `#[should_panic]` machinery which works with the dev profile).
   *Note*: tests run under the **dev** profile by default — `panic=abort`
   on release does not break `cargo test`.
4. **Measure**: record `kree.exe` size before and after. Append the numbers
   to this spec when done.

## Risks

- `lto = "fat"` adds ~30–60 s to release builds. Acceptable for releases.
- `panic = "abort"` means a panic terminates without running destructors.
  We accept this — kree has no critical cleanup beyond stdio flush, and
  the existing Quit path already uses `std::process::exit`.

## Out of scope

- Tweaking dependency profiles (`[profile.release.package."*"]`).
- Cross-arch builds (we're shipping windows-x86_64 only for now).

## Measurements

(filled in at implementation time)

- Before: `_____` bytes
- After: `_____` bytes
- Build time before: `_____`
- Build time after: `_____`
