# Assets

All assets in this directory must be either CC0 (public domain) or otherwise compatible with redistribution.

## Inventory

| File | Source | License | Notes |
|---|---|---|---|
| `chime.ogg` | Procedurally synthesized — `scripts/generate-chime.sh` (ffmpeg) | CC0 | ~480 ms, ~5 KB. A 110 Hz rumble fading into a soft C5 + G5 bell — meant to evoke a Stargate dial / chevron lock. |
| `tray-icon.png` | Procedurally generated — `scripts/generate-icons.ps1` (GDI+) | CC0 | 64×64 RGBA. Stargate ring with cyan event horizon and seven yellow chevrons. |
| `tray-icon-paused.png` | Same as above | CC0 | 64×64 RGBA. Desaturated variant for the paused state. |
| `app-icon.png` | Same as above | CC0 | 256×256 RGBA. Used for README and future Windows binary embedding. |
| `app-icon.ico` | Converted from `app-icon.png` via ffmpeg | CC0 | 256×256 single-resolution ICO. |

## Regenerating

```bash
# icons (requires PowerShell 7+)
pwsh -File scripts/generate-icons.ps1

# chime (requires ffmpeg with libvorbis)
bash scripts/generate-chime.sh

# app-icon.ico
ffmpeg -y -i assets/app-icon.png -vf "scale=256:256" assets/app-icon.ico
```

All output is deterministic; regenerating produces byte-identical files modulo encoder version drift. Commit any drift alongside the script change that caused it.
