#!/usr/bin/env bash
# RR3 (docs/rerender.feature): a video is made of shot segments, joined without encoding again. Three checks, each
# writing OUT/<check>.json:
#   equal  film.tsx's "cut" (RR0's film, each shot a Sequence) rendered as segments and in one run (RAEA_SEGMENTS=0) at
#          the same --quality: the segmented file's frames against the one run's, SSIM on every frame (>= 0.999) and
#          VMAF over the film (>= 99); how many segments, and both render times
#   join   "cut-long" (the film four times over: 2 minutes, 3,600 frames at 1080p) rendered as segments: the join's
#          time from RAEA_SEGMENTS_TRACE (< 1 s), and the frame count read back
#   page   a composition without 3D whose Sequences are its shots (bench/rerender/pages.tsx): its segments
#   bench/rerender/rr3.sh [OUT] [CHECKS]
#     RAEA_BIN  the raea to run (target/release/raea)
#     QUALITY   the --quality both renders take (high)
#     CHECKS    "equal join page" by default
# Exits 1 when a check fails.
set -u
cd "$(dirname "$0")/../.." || exit 2
ROOT=$PWD
BIN=${RAEA_BIN:-$ROOT/target/release/raea}
OUT=${1:-${TMPDIR:-/tmp}/raea-rr3}
CHECKS=${2:-equal join page}
Q=${QUALITY:-high}
FILM=$ROOT/bench/rerender/film.tsx
# segments on (RAEA_SEGMENTS=1, while RR3 is opt-in); the one-run render turns them off with RAEA_SEGMENTS=0
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0 RAEA_SEGMENTS=1 RAEA_SEGMENTS_TRACE=1 RAEA_AUDIO_MUTE=1
mkdir -p "$OUT"
fail=0

now() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; }
frames() { ffprobe -v error -count_packets -select_streams v:0 -show_entries stream=nb_read_packets -of csv=p=0 "$1"; }
# the trace's line: "N frames in K segments joined in T ms"
segs() { sed -n 's/.*frames in \([0-9]*\) segments joined in \([0-9.]*\) ms.*/\1 \2/p' "$1" | tail -1; }

# render SRC ID OUT.mp4 [ENV...]: prints its seconds, its log beside it
render() {
  local src=$1 id=$2 out=$3 t0 t1
  shift 3
  t0=$(now)
  env "$@" "$BIN" render "$src" --composition "$id" --quality "$Q" -o "$out" > "$out.log" 2>&1 || { echo "$id failed: $(grep -E 'REFUSED|error' "$out.log" | head -2)" >&2; return 1; }
  t1=$(now)
  echo "$t0 $t1" | awk '{printf "%.1f", $2 - $1}'
}

equal() {
  local ts t1 seg one min vmaf n
  ts=$(render "$FILM" cut "$OUT/cut-seg.mp4") || return 1
  t1=$(render "$FILM" cut "$OUT/cut-one.mp4" RAEA_SEGMENTS=0) || return 1
  read -r n _ <<< "$(segs "$OUT/cut-seg.mp4.log")"
  ffmpeg -hide_banner -nostats -i "$OUT/cut-seg.mp4" -i "$OUT/cut-one.mp4" -lavfi "ssim=stats_file=$OUT/ssim.txt" -f null - > /dev/null 2>&1
  min=$(sed -n 's/.*All:\([0-9.]*\).*/\1/p' "$OUT/ssim.txt" | sort -n | head -1)
  seg=$(awk '{for (i = 1; i <= NF; i++) if ($i ~ /^All:/) { split($i, a, ":"); if (a[2] < 0.999) n++ }} END {print n + 0}' "$OUT/ssim.txt")
  ffmpeg -hide_banner -nostats -i "$OUT/cut-seg.mp4" -i "$OUT/cut-one.mp4" -lavfi "libvmaf=log_fmt=json:log_path=$OUT/vmaf.json" -f null - > /dev/null 2>&1
  vmaf=$(jq '.pooled_metrics.vmaf.mean' "$OUT/vmaf.json")
  one=$(frames "$OUT/cut-seg.mp4")
  printf '{"segments": %s, "frames": %s, "ssim_min": %s, "frames_under_0.999": %s, "vmaf": %s, "seconds_segmented": %s, "seconds_one_run": %s}\n' \
    "${n:-0}" "$one" "$min" "$seg" "$vmaf" "$ts" "$t1" | tee "$OUT/equal.json"
  [ "${n:-0}" -ge 2 ] && [ "$seg" = 0 ] && awk -v v="$vmaf" 'BEGIN {exit !(v >= 99)}' && [ "$one" = 900 ]
}

join() {
  local t n ms got
  t=$(render "$FILM" cut-long "$OUT/long.mp4") || return 1
  read -r n ms <<< "$(segs "$OUT/long.mp4.log")"
  got=$(frames "$OUT/long.mp4")
  printf '{"segments": %s, "join_ms": %s, "frames": %s, "seconds": %s}\n' "${n:-0}" "${ms:-null}" "$got" "$t" | tee "$OUT/join.json"
  [ "$got" = 3600 ] && awk -v m="${ms:-99999}" 'BEGIN {exit !(m < 1000)}'
}

page() {
  local t n got
  t=$(render "$ROOT/bench/rerender/pages.tsx" pages "$OUT/pages.mp4") || return 1
  read -r n _ <<< "$(segs "$OUT/pages.mp4.log")"
  got=$(frames "$OUT/pages.mp4")
  printf '{"segments": %s, "frames": %s, "seconds": %s}\n' "${n:-0}" "$got" "$t" | tee "$OUT/page.json"
  [ "${n:-0}" = 3 ] && [ "$got" = 270 ]
}

for c in $CHECKS; do
  echo "== $c"
  if ! "$c"; then echo "FAIL $c" >&2; fail=1; fi
done
rm -f "$OUT"/*.mp4
exit $fail
