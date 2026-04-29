# T5 — System font selection via `config.toml`

## Goal

Let the user pick fonts from their installed system fonts. The bundled
JetBrains Mono Nerd Font remains as the always-present fallback — a
mistyped or missing family name never tofus the entire UI.

## Config keys (depends on T3)

```toml
[font]
proportional = "Segoe UI Variable"
monospace    = "Cascadia Mono"
fallbacks    = ["Segoe UI Emoji", "Segoe UI Symbol"]
```

All optional. Empty / missing → fall back to the existing
hard-coded chain.

## Resolution

For each named family, find a TTF/OTF/TTC file by either:

1. **`Win32 EnumFontFamiliesEx`** over `GetDC(NULL)` — yields family
   names but not file paths. We'd need a second hop to map name → file.
2. **Filesystem scan** of:
   - `C:\Windows\Fonts\`
   - `%LOCALAPPDATA%\Microsoft\Windows\Fonts\` (per-user installs)

   Parse each font's `name` table (record IDs 1, 4, 16) and match
   against the requested family name.

**Decision: filesystem scan.** Single mechanism, no platform-specific
FFI beyond what we already pull through `windows`. Use a small
crate for `name` table parsing — `read-fonts` (already a transitive
dep of egui via `skrifa`) exposes the OpenType name table.
**Alternative:** `font-kit` does the whole job in one call but is a
larger dep; if `read-fonts` isn't ergonomic enough for direct use,
`font-kit` is the fallback.

## Loader changes (`src/theme.rs`)

`install_fonts(ctx, &Config)`:

1. Scan font directories once (lazy static / `OnceLock`) into a
   `HashMap<family_name_lower, PathBuf>`.
2. For each requested family in `config.font.{proportional,monospace}`,
   look up the path; on hit, prepend its bytes to the matching egui
   `FontFamily`.
3. For `fallbacks`, same lookup, append rather than prepend.
4. The bundled JetBrains Mono Nerd Font is always present — appended
   to *both* families regardless of config.
5. The hard-coded Segoe / Cascadia / Segoe Emoji chain stays as a
   *secondary* fallback when nothing is configured.

## Hot reload

Changing `font.*` in `config.toml` calls `install_fonts` again. Note
that `set_fonts` rebuilds the egui font atlas — this is mildly
expensive but acceptable since reloads are user-initiated (not per
frame).

## Tests

- `enumerate_system_fonts` returns at least one entry on Windows
  (smoke; uses `C:\Windows\Fonts\segoeui.ttf` as a known-present
  family name to assert on).
- `resolve_family("Segoe UI")` finds a `PathBuf`. Skipped on non-Windows.
- `resolve_family("Definitely Not A Real Family 12345")` returns
  `None`.
- TTC support: at least probed (a TTC contains multiple faces — we
  pick face index 0 unless a config sub-key specifies otherwise; out
  of scope for this commit, document as TODO).

## Acceptance

1. Setting `font.proportional = "Segoe UI"` actually changes the
   visible font.
2. Setting it to `"Definitely Not A Real Family 12345"` falls
   through to the bundled NF + hard-coded chain — no tofu, no
   crash, a single warning logged.
3. Config hot-reload triggers a font atlas rebuild and the new font
   appears within ~1 s (debouncer + atlas rebuild).

## Risks

- `name` table parsing has edge cases (preferred-family vs
  legacy-family record IDs, mac vs windows platform IDs). Pull
  the canonical name preferring ID 16 → 1 in that order, platform
  ID 3 (Windows).
- TTC files contain multiple faces; we pick face 0 and document
  the limitation.

## Out of scope

- Per-font weight / style selection (`font.proportional.bold = ...`).
  egui will fall back to default weight via the family chain.
- Fontconfig on Linux — kree is Windows-only.
- Variable-font axis selection (Segoe UI Variable's optical-size,
  weight axes).
