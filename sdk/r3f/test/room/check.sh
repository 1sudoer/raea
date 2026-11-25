#!/bin/bash
# <R3F> lit by three's RoomEnvironment (#165): room.tsx's scenes rendered by raea, each checked pixel within 8 levels
# of what three draws there (three.ts works three's pixels out on the CPU: bun sdk/r3f/test/room/three.ts). Before
# raea made the room, the scene was drawn without it: the repro's box face 136,76,58 against three's 255,195,160, the
# grey boxes black. Exits 1 on a failure.
#   sdk/r3f/test/room/check.sh      (RAEA_BIN: the raea to run, default target/release/raea)
cd "$(dirname "$0")/../../../.." || exit 2
BIN=${RAEA_BIN:-$PWD/target/release/raea}
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0 RAEA_AUDIO_MUTE=1
OUT=$(mktemp -d "${TMPDIR:-/tmp}/raea-r3f-room.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
T=sdk/r3f/test/room/room.tsx
fail=0
# composition, pixel x, y, three's r,g,b (three.ts)
while read -r comp x y want; do
  [ -f "$OUT/$comp.png" ] || {
    "$BIN" render $T --composition "$comp" --frame 0 -o "$OUT/$comp.png" > "$OUT/$comp.log" 2>&1 || { echo "FAIL: $comp: the render failed"; tail -3 "$OUT/$comp.log"; exit 1; }
    ! grep -q 'PMREMGenerator.fromScene' "$OUT/$comp.log" || { echo "FAIL: $comp: raea left the room out ($(grep -m1 -o 'scene.environment.*' "$OUT/$comp.log"))"; fail=1; }
  }
  # the 9 x 9 pixels round it, averaged
  got=$(magick "$OUT/$comp.png" -crop "9x9+$((x - 4))+$((y - 4))" +repage -scale '1x1!' -format '%[fx:int(255*r+.5)],%[fx:int(255*g+.5)],%[fx:int(255*b+.5)]' info:)
  if awk -F, -v a="$got" -v b="$want" 'BEGIN { split(a, g, ","); split(b, w, ","); for (i = 1; i <= 3; i++) if (g[i] - w[i] > 8 || w[i] - g[i] > 8) exit 1 }'; then
    echo "ok    $comp $x,$y: raea $got, three $want"
  else
    echo "FAIL  $comp $x,$y: raea $got, three $want"
    fail=1
  fi
done <<'EOF'
repro 200 200 255,195,160
grey 97 200 140,140,140
grey 303 200 127,127,127
EOF
[ $fail = 0 ] && echo "PASS: an <R3F> scene is lit by three's RoomEnvironment as three lights it"
exit $fail
