## Summary

<!-- One or two sentences. What changed and why. -->

## Build-order step

<!-- If this implements a step from `docs/specs/spec.md` § 8, name it. Otherwise: N/A. -->

## Checklist

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-features`
- [ ] Spec / docs updated if behavior changed
- [ ] No new features beyond `docs/specs/spec.md` (or explicitly approved)
