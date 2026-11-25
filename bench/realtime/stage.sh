#!/bin/bash
# Is the live frame the render's frame? (docs/realtime.feature RT0a, RT0b; the plan's experiment 1.) For each case: the
# composition shown on the engine's stage in a headless raea window, paused on frame N, then one control changed with
# stage.props on that running world; against `raea render --frame N` of the same file and of the copy with the new
# value written into its defaultProps. SSIM (ffmpeg's, over RGB) between:
#   a   the stage on the file as written        and its render: the stage against the render, no change
#   ab  the stage after stage.props {PROP: B}   and the copy's render: a props change on the running world
#   abs the same, the frame then seeked to again (what the app sends after props on a paused stage), and the
#       copy's render: what experiment 1 measures; and against the copy shown fresh on the stage
#   b   the copy shown fresh                    and the copy's render
# each after every one of UPDATES draws of the frame (k = 0 is the draw the seek or the props made): a ray-traced
# frame's history converges over draws, and the first k at SSIM >= 0.995 is reported. A row:
#   FILE PROP A B FRAME UPDATES [SEED|-] [seq]   (B as JSON; SEED: RAEA_SEED for the window, so a live session takes
#                                          the render's seed; seq: also against frame N of a run rendering 0 to N;
#                                          rows from RT_ROWS, default bench/realtime/stage.tsv)
#   bash bench/realtime/stage.sh [ROW-SUBSTRING...]     RAEA_BIN, RAEA_PAGE_HOST_BIN: the binaries (target/release's);
#                                                       RT_OUT: where pictures go; RT_PORT: the window's MCP port (7631)
# Headless only: the window is raea-page-host without --window, on its own port, its sound off.
cd "$(dirname "$0")/../.." || exit 2
ROOT=$PWD
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_AUDIO_MUTE=1 RAEA_HOME=$ROOT
BIN=${RAEA_BIN:-$ROOT/target/release/raea}
HOST=${RAEA_PAGE_HOST_BIN:-$ROOT/target/release/raea-page-host}
ROWS=${RT_ROWS:-$ROOT/bench/realtime/stage.tsv}
PORT=${RT_PORT:-7631}
OUT=${RT_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/raea-rt-stage.XXXXXX")}
mkdir -p "$OUT"
bun "$ROOT/bench/realtime/stage/build.ts" "$OUT/page.js" || { echo "FAIL the stage page didn't build"; exit 1; }
ssim() { ffmpeg -nostdin -hide_banner -nostats -i "$1" -i "$2" -lavfi "[0:v]format=rgb24[a];[1:v]format=rgb24[b];[a][b]ssim" -f null - 2>&1 | grep -o 'All:[0-9.]*' | cut -d: -f2; }
dim() { grep '<Composition' "$1" | head -1 | grep -oE "$2=\{[0-9]+\}" | head -1 | tr -dc 0-9; }
host=""
copies=()
stop() {
  [ -n "$host" ] && kill "$host" 2>/dev/null
  for c in "${copies[@]+"${copies[@]}"}"; do rm -f -- "$c"; done
}
trap stop EXIT
pass=0 fail=0
while IFS=$'\t' read -r path prop a b frame updates seed seq; do
  [[ -z $path || $path == \#* ]] && continue
  if [ $# -gt 0 ]; then hit=0; for s in "$@"; do [[ $path == *"$s"* ]] && hit=1; done; [ $hit = 1 ] || continue; fi
  [[ $path == /* ]] && src=$path || src=$ROOT/$path
  [ "$seed" = - ] && seed=""
  slug=$(basename "$src" .tsx)${seed:+-seed}
  dir=$OUT/$slug && rm -rf "${dir:?}" && mkdir -p "$dir"
  w=$(dim "$src" width); h=$(dim "$src" height)
  # the copy with B written into its defaultProps, beside the file (its imports resolve as the file's): as an edit's
  # draft is written (the app's host/draft.ts), and removed when this ends
  copy=$(dirname "$src")/.rt-$slug-b.tsx
  copies+=("$copy")
  PROP=$prop A=$a B=$b perl -0pe 's/(defaultProps=\{\{[^}]*\b\Q$ENV{PROP}\E:\s*)\Q$ENV{A}\E/$1$ENV{B}/' "$src" > "$copy"
  if cmp -s "$src" "$copy"; then echo "FAIL $path: couldn't write $prop: $b into its defaultProps"; fail=$((fail + 1)); continue; fi
  # the renders: the frame alone, in a process of its own (a still: `render --frame N`); with seq also frames 0 to N
  # in one run (`--at 0,...,N`), frame N as a film's render reaches it, one update a frame
  for v in a b; do
    f=$src; [ $v = b ] && f=$copy
    t0=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
    (cd "$(dirname "$src")" && RAEA_READBACK=rgba "$BIN" render "$f" --frame "$frame" -o "$dir/render-$v.png") > "$dir/render-$v.log" 2>&1 || echo "note $path: render $v exited 1 (its checks found errors; the picture is still compared): $(grep -m1 -E "^raea: error" "$dir/render-$v.log" | cut -c1-160)"
    [ -f "$dir/render-$v.png" ] || { echo "FAIL $path: render $v: $(grep -m1 -E 'REFUSED|error' "$dir/render-$v.log" | cut -c1-200)"; fail=$((fail + 1)); continue 2; }
    echo "render-$v $(perl -MTime::HiRes=time -e "printf '%.2f', time - $t0") s" >> "$dir/times.txt"
    if [ "$seq" = seq ]; then
      (cd "$(dirname "$src")" && RAEA_READBACK=rgba "$BIN" render "$f" --at "$(seq 0 "$frame" | paste -sd, -)" -o "$dir/seq-$v/") > "$dir/seq-$v.log" 2>&1
      [ -f "$dir/seq-$v/$(printf "frame-%05d.png" "$frame")" ] || { echo "FAIL $path: sequence render $v"; fail=$((fail + 1)); continue 2; }
      cp "$dir/seq-$v/$(printf 'frame-%05d.png' "$frame")" "$dir/film-$v.png"
      rm -rf "${dir:?}/seq-$v"
    fi
  done
  echo "rendered $path frame $frame: $(tr '\n' ' ' < "$dir/times.txt")"
  # the window: one window point a stage pixel, the stage at the composition's size
  [ -n "$host" ] && kill "$host" 2>/dev/null && wait "$host" 2>/dev/null
  if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t >/dev/null; then echo "FAIL port $PORT is in use (RT_PORT)"; exit 1; fi
  ( [ -n "$seed" ] && export RAEA_SEED=$seed; RAEA_PAGE_MCP_PORT=$PORT exec "$HOST" "$OUT/page.js" --size "${w}x$h" ) > "$dir/page.log" 2>&1 &
  host=$!
  curl -s --retry 120 --retry-connrefused --retry-delay 1 --retry-max-time 120 -o /dev/null "http://127.0.0.1:$PORT/" || { echo "FAIL $path: the window didn't start: $(tail -2 "$dir/page.log")"; exit 1; }
  # its progress a line a capture on stderr (kept in stage.err), so a long converging row is seen moving
  if ! node "$ROOT/bench/realtime/stage.mjs" "$PORT" "$dir" "$src" "$copy" "$prop" "$b" "$frame" "$w" "$h" "${updates:-3}" > "$dir/stage.out" 2> >(tee "$dir/stage.err" >&2); then
    echo "FAIL $path: the stage: $(grep -m1 FAIL "$dir/stage.err" | cut -c1-240)"; fail=$((fail + 1)); continue
  fi
  # each capture against the renders: columns k, a~render-a, ab~render-b, abs~render-b, abs~b, b~render-b, and with
  # seq a~film-a, abs~film-b, b~film-b
  for k in $(ls "$dir" | sed -n 's/^a-\([0-9]*\)\.png$/\1/p' | sort -n); do
    row="$k\t$(ssim "$dir/a-$k.png" "$dir/render-a.png")\t$(ssim "$dir/ab-$k.png" "$dir/render-b.png")\t$(ssim "$dir/abs-$k.png" "$dir/render-b.png")\t$(ssim "$dir/abs-$k.png" "$dir/b-$k.png")\t$(ssim "$dir/b-$k.png" "$dir/render-b.png")"
    [ "$seq" = seq ] && row="$row\t$(ssim "$dir/a-$k.png" "$dir/film-a.png")\t$(ssim "$dir/abs-$k.png" "$dir/film-b.png")\t$(ssim "$dir/b-$k.png" "$dir/film-b.png")"
    echo -e "$row" >> "$dir/ssim.tsv"
  done
  # the first draw at which a column comes within 0.995 and 1.000000, and its value at the first and last draw
  first() { awk -F'\t' -v c="$1" -v t="$2" '$c + 0 >= t { print $1; f = 1; exit } END { if (!f) print "never" }' "$dir/ssim.tsv"; }
  ends() { awk -F'\t' -v c="$1" 'NR == 1 { a = $c } { z = $c; k = $1 } END { printf "%s (k0) .. %s (k%s)", a, z, k }' "$dir/ssim.tsv"; }
  ms=$(node -e 'const r=require(process.argv[1]); console.log(`${r.props_ms ? r.props_ms.painted : "none"} ms, props+seek->painted ${r.props_seek_ms ?? "none"} ms`)' "$dir/stage.json")
  echo "ROW $path${seed:+ (seed $seed)} $prop=$b frame $frame: stage~render a $(ends 2); props alone ab $(ends 3); props+seek abs $(ends 4) (>= 0.995 at k=$(first 4 0.995), 1 at k=$(first 4 1)); abs~fresh b $(ends 5); fresh b $(ends 6) (>= 0.995 at k=$(first 6 0.995)); props->painted $ms"
  [ "$seq" = seq ] && echo "ROW $path against the film's frame (render --at 0..$frame): a $(ends 7); abs $(ends 8) (>= 0.995 at k=$(first 8 0.995)); fresh b $(ends 9) (>= 0.995 at k=$(first 9 0.995))"
  pass=$((pass + 1))
done < "$ROWS"
echo "stage = render: $pass rows measured, $fail failed (out: $OUT)"
[ $fail = 0 ]
