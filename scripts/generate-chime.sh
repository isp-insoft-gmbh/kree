#!/usr/bin/env bash
# Synthesizes a short Stargate-flavored chime via ffmpeg.
# Output: assets/chime.ogg (≤ 50 KB, ≤ 500 ms, mono, Vorbis).
#
# Vibe: a brief low rumble (gate dialing) into a soft two-note bell (chevron lock).
# Synthesis is original — CC0.

set -euo pipefail

OUT="$(dirname "$0")/../assets/chime.ogg"
OUT="$(realpath "$OUT")"

ffmpeg -y -hide_banner -loglevel error \
  -f lavfi -i "sine=frequency=110:duration=0.22" \
  -f lavfi -i "sine=frequency=523.25:duration=0.32" \
  -f lavfi -i "sine=frequency=783.99:duration=0.32" \
  -filter_complex "
    [0]afade=t=in:d=0.01,afade=t=out:st=0.16:d=0.06,volume=0.55[low];
    [1]adelay=160|160,afade=t=in:d=0.005,afade=t=out:st=0.22:d=0.10,volume=0.50[mid];
    [2]adelay=160|160,afade=t=in:d=0.005,afade=t=out:st=0.22:d=0.10,volume=0.45[hi];
    [low][mid][hi]amix=inputs=3:duration=longest:normalize=0,
    aformat=channel_layouts=mono,
    volume=0.85
  " \
  -c:a libvorbis -q:a 4 -ar 44100 -ac 1 \
  "$OUT"

bytes=$(stat -c %s "$OUT" 2>/dev/null || stat -f %z "$OUT")
echo "wrote $OUT ($bytes bytes)"
