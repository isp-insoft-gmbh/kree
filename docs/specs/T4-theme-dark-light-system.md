# T4 — Theme: dark / light / system

## Goal

Let the user choose the kree palette: dark (current), light (new), or
system (follow Windows app-mode setting and update on change).

## Config key (depends on T3)

```toml
theme = "dark" | "light" | "system"
```

Default: `"system"`.

## Mapping

- `dark`: existing `theme::apply_dark` (renamed from `theme::apply`).
- `light`: new `theme::apply_light` mapping the *light* variant of the
  nugu output (`~/Workspaces/dotfiles/nugu/output/light.toml`).
- `system`: read the registry key
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
  Value `0` → dark, value `1` → light. Missing → dark.

## Light palette source

From `~/Workspaces/dotfiles/nugu/output/light.toml`:

```
[palette]                  [palette.ui]
debug   = #a112a1          normal             = #242424
error   = #c0223c          backdrop           = #dbdbdb
warning = #242317          accent             = #922792
info    = #13616e          minor              = #292929
                           focus              = #15333b
                           unfocus            = #5d5d5d
                           important_local    = #604980
                           important_global   = #3e2f53
```

(Note: nugu's `important_*` colors flip semantics in light mode —
`important_global` is the *darkest* magenta-tinted ink, used for headings.)

## System change watching

Two options; pick the simpler:

1. **Polling**: poll the registry key every 2 s. Cheap, simple, no
   extra crates. The change is rare and not user-initiated from kree's
   side, so 2 s latency is fine.
2. **WM_SETTINGCHANGE**: listen for the broadcast. Higher fidelity
   but requires hooking a window proc. Since eframe owns the loop and
   we already pump tray-icon on the same thread, we'd add a hidden
   message-only window. Disproportionate for the gain.

**Decision: poll every 2 s** in the existing tokio runtime. A single
task that loops, reads the registry, and pushes a
`UiMessage::SystemThemeChanged(bool)` whenever the value differs.

## GUI

In the Settings section (added in T3), add a `Theme:` selector
(`dark` / `light` / `system` radio or combo). Same patch
mechanism as the other config keys.

## Tests

- Light palette `apply_light` produces a `Visuals` with non-trivial
  colors (smoke test that the function compiles + runs).
- `system_theme_from_registry` returns `Theme::Light` when the
  registry key value is `1`, `Theme::Dark` when `0`, and `Theme::Dark`
  when the key is missing (default fallback).
- `Config::theme` round-trips through `toml_edit` (covered by the T3
  patch-round-trip test, parametrized over keys).

## Acceptance

1. Switching the GUI selector applies in real time.
2. Setting `system` and toggling Windows light/dark in Settings →
   Personalization causes kree to flip within ~2 s.
3. Light theme passes a basic eyeball check: text contrasts against
   the light backdrop, accent magenta still recognizable.

## Out of scope

- High-contrast mode.
- Per-popup theme overrides.
- Animated theme transition (just snap).
