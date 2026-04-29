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
3. `cargo test` (dev profile, default) continues to pass — `panic = "abort"`
   only applies to the release profile. We deliberately **do not** run
   `cargo test --release`: `#[should_panic]` requires unwinding, which
   `panic = "abort"` strips, and `-Z panic-abort-tests` is nightly-only.
4. **Measure**: record `kree.exe` size before and after. Try both
   `opt-level = "z"` and `opt-level = "s"`; pick whichever is smaller
   (sometimes `s` wins by a few KB because `z` disables loop vectorization
   in ways that cost more than they save).

## Risks

- `lto = "fat"` adds ~30–60 s to release builds. Acceptable for releases.
- `panic = "abort"` means a panic terminates without running destructors.
  We accept this — kree has no critical cleanup beyond stdio flush, and
  the existing Quit path already uses `std::process::exit`.

## Out of scope

- Tweaking dependency profiles (`[profile.release.package."*"]`).
- Cross-arch builds (we're shipping windows-x86_64 only for now).

## Measurements

Local Windows 11 / rustc 1.95 — `release` profile of the
`auto/todos` branch:

| profile                                               | bytes        | size   | wall-clock |
|-------------------------------------------------------|--------------|--------|------------|
| baseline (`opt-level = 3` default)                    | 19,506,688   | 18.6 MB | ~unmeasured (cached) |
| optimized, `opt-level = "z"`                          | 13,141,504   | 12.5 MB | 3 m 04 s   |
| optimized, `opt-level = "s"` (chosen)                 | 12,474,368   | 11.9 MB | 3 m 18 s   |

`s` won by 667 KB. Final shipped Cargo.toml uses `opt-level = "s"`.
