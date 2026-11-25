#!/bin/bash
# #162: every still of a batch (--at A,B,C in one run) shows its own frame's time, as it does alone and as a scene
# stepped through every frame before it does. Since #77 a still of a scene no plugin carries state for isn't stepped to,
# and the plugins' clock then never moved: every still sampled globals.time 0, and a batch's later frames drew its
# first frame's <Material> uniforms (they land as keyed rows). time.tsx's face shows globals.time on its left half and
# a uniform the frame gives its time on its right, so the halves match only when Time is the frame's. Its "time"
# composition is drawn at 90, 30, 60 in one run (the clock forwards and back) and at 90 alone, and must step no frame
# to reach them; "stepped" (the same face with an animation player, so its stills are stepped to, as 0.1.0 drew every
# still) at the same three. Each frame's halves must match within 1 level, each "time" frame must match the "stepped"
# one and 90 alone the batch's 90 (within 1 level), and 90 must be brighter than 60 and 60 than 30. Exits 1 on a failure.
#   bench/at-time/check.sh      (RAEA_BIN: the raea to run, default target/release/raea)
cd "$(dirname "$0")/../.." || exit 2
BIN=${RAEA_BIN:-$PWD/target/release/raea}
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0
OUT=$(mktemp -d "${TMPDIR:-/tmp}/raea-at-time.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
draw() { # NAME COMPOSITION AT: the frames in $OUT/NAME, the plugin clock's trace in $OUT/NAME.log
  RAEA_PLUGIN_TRACE=1 "$BIN" render bench/at-time/time.tsx --composition "$2" --at "$3" -o "$OUT/$1" > "$OUT/$1.log" 2>&1 && [ -f "$OUT/$1/frame-00000.png" ] || { echo "FAIL $1: the render failed"; grep -v "^plugin clock" "$OUT/$1.log" | tail -4 | sed 's/^/    /'; exit 1; }
}
draw batch time 90,30,60
draw lone time 90
draw stepped stepped 90,30,60
status=0
fail() { echo "FAIL $*"; status=1; }
steps() { grep -c "^plugin clock: .* step$" "$OUT/$1.log"; }
(( $(steps batch) == 0 && $(steps lone) == 0 )) || fail "the time stills stepped $(steps batch) and $(steps lone) frames, wanted none (it has nothing a plugin carries)"
(( $(steps stepped) > 0 )) || fail "the stepped stills stepped no frame, so they show nothing of the stepped clock"
half() { magick "$1" -crop "24x48+$2+8" +repage -format "%[fx:int(255*mean.r+0.5)]" info:; } # FILE X: a half's grey
apart() { magick "$1" "$2" -compose difference -composite -format "%[fx:int(255*maxima+0.5)]" info:; }
k=0
for at in 90 30 60; do
  f=$(printf 'frame-%05d.png' $k)
  for run in batch stepped; do
    t=$(half "$OUT/$run/$f" 4) u=$(half "$OUT/$run/$f" 36)
    (( t - u <= 1 && u - t <= 1 )) && echo "ok   $run frame $at: globals.time's half $t, the uniform's $u" || fail "$run frame $at: globals.time's half is $t and the uniform's $u: Time isn't the frame's"
  done
  d=$(apart "$OUT/batch/$f" "$OUT/stepped/$f")
  (( d <= 1 )) || fail "frame $at differs from the stepped scene's by up to $d levels"
  eval "grey_$at=$(half "$OUT/batch/$f" 4)"
  k=$((k + 1))
done
d=$(apart "$OUT/lone/frame-00000.png" "$OUT/batch/frame-00000.png")
(( d <= 1 )) && echo "ok   frame 90 alone matches the batch's frame 90" || fail "frame 90 alone differs from the batch's frame 90 by up to $d levels"
(( grey_90 > grey_60 && grey_60 > grey_30 )) || fail "the batch's frames 30, 60 and 90 are $grey_30, $grey_60 and $grey_90: frozen, not moving on"
[ $status = 0 ] && echo "PASS every still of a batch shows its own frame's time, as alone and as stepped to"
exit $status
