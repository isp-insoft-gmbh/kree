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

## Crate features

Reading the registry needs `Win32_System_Registry` on the existing
`windows` crate dependency. Bump the feature list in `Cargo.toml`:

```toml
windows = { version = "0.62", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
    "Win32_System_Registry",
] }
```

No new crate needed.

## System change watching

Two options; pick the simpler:

1. **Polling**: poll the registry key every 2 s. Cheap, simple, no
   extra crates. The change is rare and not user-initiated from kree's
   side, so 2 s latency is fine.
2. **WM_SETTINGCHANGE**: listen for the broadcast. Higher fidelity
   but requires hooking a window proc. Since eframe owns the loop and
   we already pump tray-icon on the same thread, we'd add a hidden
   message-only window. Disproportionate for the gain.

**Decision: poll every 1 s** in the existing tokio runtime. A single
task that loops, reads the registry, and pushes a
`UiMessage::SystemThemeChanged(bool)` whenever the value differs.
1 s is the worst-case latency; 1 s of CPU per second on a registry
read is negligible.

The poll task respects shutdown: it `tokio::select!`s between the
1 s tick and a `tokio::sync::watch::Receiver<()>` that the main
shutdown path closes when eframe exits. Without this the
`runtime.shutdown_timeout(2s)` in `main` would have to wait for the
sleep to fire before the task could observe shutdown.

## GUI

In the Settings section (added in T3), add a `Theme:` selector
(`dark` / `light` / `system` radio or combo). Same patch
mechanism as the other config keys.

## Tests

- `apply_light` builds a `Visuals` whose `panel_fill` equals the
  light backdrop `#dbdbdb` (specific assertion, not a smoke test).
- `apply_dark` (renamed from current `apply`) keeps `panel_fill` at
  `#242424`.
- `system_theme_from_registry` is split into a pure helper
  `decode_theme(value: u32) -> Theme` (1 → Light, 0 → Dark,
  any other → Dark) so the decoder is unit-testable without
  hitting the registry. The registry-reading caller is integration-only.
- `Config::theme` round-trips through `toml_edit` (covered by the T3
  patch-round-trip test, parametrized over keys).

## Acceptance

1. Switching the GUI selector applies in real time.
2. Setting `system` and toggling Windows light/dark in Settings →
   Personalization causes kree to flip within ≤ 2 s (1 s poll + the
   per-frame egui repaint).
3. Light theme passes a basic eyeball check: text contrasts against
   the light backdrop, accent magenta still recognizable.
4. The system-theme polling task exits within 100 ms of the runtime
   being asked to shut down (no 1 s sleep delay on quit).

## Out of scope

- High-contrast mode.
- Per-popup theme overrides.
- Animated theme transition (just snap).
