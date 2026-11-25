#!/bin/bash
# #159: --aov and RAEA_PASS on a multisampled 3D camera write every frame. Both read the camera's prepasses as
# single-sampled textures, so they turn its multisampling off; the camera row's post-processing turned it back on: in
# a render, on the first frame whose post changed (ramp.tsx's bloom ramps); in a game, on every frame its rows set
# (game.tsx). Frame 2 was then refused ("Texture binding 0 expects multisampled = false, but given a view with
# samples = 4"), with frames 0 and 1 written. Here frames 0 to 5 of each are rendered: --aov must write each frame's
# depth and motion at the frame's size (320 x 180 floats: 4 and 8 bytes a pixel), the depth not all zero (something
# drawn), and RAEA_PASS=depth all six pictures. Exits 1 on a failure.
#   bench/aov-msaa/check.sh      (RAEA_BIN: the raea to run, default target/release/raea)
cd "$(dirname "$0")/../.." || exit 2
BIN=${RAEA_BIN:-$PWD/target/release/raea}
AT=0,1,2,3,4,5
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0 RAEA_AUDIO_MUTE=1
OUT=$(mktemp -d "${TMPDIR:-/tmp}/raea-aov-msaa.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
refused() { grep -m1 REFUSED "$1" || tail -3 "$1"; }
for c in ramp game; do
  "$BIN" render "bench/aov-msaa/$c.tsx" --at "$AT" --aov -o "$OUT/$c" > "$OUT/$c.log" 2>&1 || { echo "FAIL: $c --aov: the render failed"; refused "$OUT/$c.log"; exit 1; }
  for i in 0 1 2 3 4 5; do
    n=$(printf '%05d' "$i")
    [ "$(wc -c < "$OUT/$c/depth/frame-$n.f32" 2>/dev/null | tr -d ' ')" = 230400 ] || { echo "FAIL: $c --aov: frame $i's depth is missing or not 320 x 180"; exit 1; }
    [ "$(wc -c < "$OUT/$c/motion/frame-$n.f32" 2>/dev/null | tr -d ' ')" = 460800 ] || { echo "FAIL: $c --aov: frame $i's motion is missing or not 320 x 180"; exit 1; }
    [ "$(LC_ALL=C tr -d '\0' < "$OUT/$c/depth/frame-$n.f32" | head -c 1 | wc -c | tr -d ' ')" = 1 ] || { echo "FAIL: $c --aov: frame $i's depth is all zero"; exit 1; }
  done
  echo "PASS: $c --aov: frames 0 to 5, each with its depth and motion"
done
RAEA_PASS=depth RAEA_READBACK=rgba "$BIN" render bench/aov-msaa/ramp.tsx --at "$AT" -o "$OUT/pass" > "$OUT/pass.log" 2>&1 || { echo "FAIL: ramp RAEA_PASS=depth: the render failed"; refused "$OUT/pass.log"; exit 1; }
[ "$(ls "$OUT/pass"/frame-*.png 2>/dev/null | wc -l | tr -d ' ')" = 6 ] || { echo "FAIL: ramp RAEA_PASS=depth: not six frames"; exit 1; }
echo "PASS: ramp RAEA_PASS=depth: frames 0 to 5"
