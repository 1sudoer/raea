#!/bin/bash
# Does a window render cost the window or the whole film? (docs/realtime.feature RT1; the plan's experiment 2.) For each
# row: a copy of the film with `edit={{ segments: [[FROM, TO]], scale: SCALE }}` on its composition, as the app's
# host/draft.ts writes the seconds around the playhead, rendered as the host renders a draft (`raea render COPY -o
# OUT.mp4 --quality draft`, from the film's folder) with RAEA_SIGN=0, RAEA_EXPORT_CACHE=0 (a draft is a file never
# rendered before: the export cache can't have it) and RAEA_STARTUP_TRACE=1 RAEA_BAKE_TRACE=1:
#   cold   RAEA_BUNDLE_CACHE=0 as well: bundled afresh, as a draft's new file is
#   warm   the same copy again with its bundle cached, RUNS times (2)
#   signed once more as the host runs it, signing on (no RAEA_SIGN or RAEA_MARK), by RAEA_SIGNED_BIN (default
#          RAEA_BIN; the public raea has signing's seam and no signer, so pass the build the app runs), then the
#          file searched for a C2PA manifest (its "c2pa" JUMBF label), beside the cold run's; RT_VERIFY, a command,
#          also reads both. Only rows with `sign`: each signing is one call to the signer, which counts them
# One line a run: wall seconds, the startup marks after the process began (bundled, the first frame, the 3D restart
# and its first frame), the frame time raea reports, the video's frames and seconds (ffprobe), and RAEA_BAKE_TRACE's
# line when a bake ran (none did on 11-16: a draft's fed render steps only its window's frames). A row:
#   FILE COMPOSITION FROM TO SCALE [sign]       (rows from RT_WINDOW_ROWS, default bench/realtime/window.tsv)
#   bash bench/realtime/window.sh [ROW-SUBSTRING...]    RAEA_BIN: the raea to run; RT_OUT: where videos go
cd "$(dirname "$0")/../.." || exit 2
ROOT=$PWD
export RAEA_AUDIO_MUTE=1 RAEA_HOME=$ROOT RAEA_EXPORT_CACHE=0 RAEA_STARTUP_TRACE=1 RAEA_BAKE_TRACE=1
BIN=${RAEA_BIN:-$ROOT/target/release/raea}
ROWS=${RT_WINDOW_ROWS:-$ROOT/bench/realtime/window.tsv}
RUNS=${RUNS:-2}
OUT=${RT_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/raea-rt-window.XXXXXX")}
mkdir -p "$OUT"
copies=()
trap 'for c in "${copies[@]+"${copies[@]}"}"; do rm -f -- "$c"; done' EXIT
now() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; }
# one render, timed; prints its line
run() {
  local tag=$1 copy=$2 comp=$3 out=$4 log=$5; shift 5
  local t0 t1
  t0=$(now)
  (cd "$(dirname "$copy")" && env "$@" "$BIN" render "$copy" --composition "$comp" -o "$out" --quality draft) > "$log" 2>&1
  local rc=$?
  t1=$(now)
  # the startup marks, in seconds after the process's first ("main")
  local marks
  marks=$(awk '/^raea startup:/ { t = $3; sub(/^raea startup: [0-9.]+ ms  /, ""); sub(/  \(gpu.*/, ""); if (!t0) t0 = t; printf "%s %.2f s; ", $0, (t - t0) / 1000 }' "$log")
  local per frames secs
  per=$(grep -oE '\([0-9.]+ ms a frame' "$log" | head -1 | tr -dc '0-9.')
  frames=$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$out" 2>/dev/null)
  secs=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$out" 2>/dev/null)
  local bake
  bake=$(grep -m1 '^raea bake:' "$log" | sed 's/^raea bake: //')
  printf '%s\texit %s\t%.2f s\t%s ms a frame\t%s frames, %s s of video\tbake: %s\tstartup: %s\n' "$tag" "$rc" "$(echo "$t1 - $t0" | bc)" "${per:-?}" "${frames:-?}" "${secs:-?}" "${bake:-none}" "$marks"
}
while IFS=$'\t' read -r path comp from to scale sign; do
  [[ -z $path || $path == \#* ]] && continue
  if [ $# -gt 0 ]; then hit=0; for s in "$@"; do [[ "$path $comp" == *"$s"* ]] && hit=1; done; [ $hit = 1 ] || continue; fi
  [[ $path == /* ]] && src=$path || src=$ROOT/$path
  slug=$comp
  dir=$OUT/$slug && rm -rf "${dir:?}" && mkdir -p "$dir"
  copy=$(dirname "$src")/.rt-window-$slug.tsx
  copies+=("$copy")
  COMP=$comp EDIT="edit={{ segments: [[$from, $to]], scale: $scale }} " perl -pe 's/(<Composition id="\Q$ENV{COMP}\E" )/$1$ENV{EDIT}/' "$src" > "$copy"
  if cmp -s "$src" "$copy"; then echo "FAIL $path: no <Composition id=\"$comp\" to give the window"; continue; fi
  echo "ROW $path #$comp window [$from, $to) at scale $scale"
  run cold "$copy" "$comp" "$dir/cold.mp4" "$dir/cold.log" RAEA_SIGN=0 RAEA_BUNDLE_CACHE=0
  for ((i = 1; i <= RUNS; i++)); do run "warm$i" "$copy" "$comp" "$dir/warm$i.mp4" "$dir/warm$i.log" RAEA_SIGN=0; done
  if [ "$sign" = sign ]; then
    BIN=${RAEA_SIGNED_BIN:-$BIN} run signed "$copy" "$comp" "$dir/signed.mp4" "$dir/signed.log" -u RAEA_SIGN -u RAEA_MARK
    for v in cold signed; do
      n=$(LC_ALL=C grep -a -c c2pa "$dir/$v.mp4")
      echo "credentials $v: $([ "${n:-0}" -gt 0 ] && echo "a C2PA manifest" || echo none)${RT_VERIFY:+; $($RT_VERIFY "$dir/$v.mp4" 2>&1 | head -c 400)}"
    done
  fi
done < "$ROWS"
echo "videos and logs in $OUT"
