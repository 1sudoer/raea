#!/bin/bash
# An <R3F> object scaled to 0 draws nothing, as in three (zero.tsx, #164): no green pixel in either still (unlit, and
# lit with shadows), and the red control box drawn. Before the fix the three green boxes drew at full size. Exits 1 on
# a failure.
#   sdk/r3f/test/scale/check.sh      (RAEA_BIN: the raea to run, default target/release/raea)
cd "$(dirname "$0")/../../../.." || exit 2
BIN=${RAEA_BIN:-$PWD/target/release/raea}
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0 RAEA_AUDIO_MUTE=1
OUT=$(mktemp -d "${TMPDIR:-/tmp}/raea-r3f-scale.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
for c in zero zero-lit; do
  "$BIN" render sdk/r3f/test/scale/zero.tsx --composition $c --frame 0 -o "$OUT/$c.png" > "$OUT/$c.log" 2>&1 || { echo "FAIL: $c: the render failed"; tail -3 "$OUT/$c.log"; exit 1; }
  # pixels of a colour: its channel 0.2 over each other (a lit face, tone mapped, is a paler green but still green)
  count() { magick "$OUT/$c.png" -alpha off -fx "$1" -format "%[fx:round(mean*w*h)]" info:; }
  green=$(count "g > r + 0.2 && g > b + 0.2")
  red=$(count "r > g + 0.2 && r > b + 0.2")
  [ "$red" -gt 0 ] || { echo "FAIL: $c: the control box isn't drawn (no red pixel)"; exit 1; }
  [ "$green" -eq 0 ] || { echo "FAIL: $c: $green green pixels: an object scaled to 0 is drawn"; exit 1; }
  echo "PASS: $c: an <R3F> object scaled to 0 draws nothing ($red red pixels of the control, 0 green)"
done
