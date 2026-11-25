#!/bin/bash
# IX14 (docs/index.feature): the Index's fast pass, measured on a fixed public set. Prints pictures a second (as the
# indexer counts them and from the command's start to its last row searchable), the seconds until the first pictures
# are searchable, peak memory, warm search time, and precision@5 for a fixed list of words.
#
#   bench/index/run.sh [RAEA_BINARY] [N]     (default: raea on PATH, 10000 pictures)
#
# The set: the first N of Open Images V4's 41,620 validation pictures by id (CC BY 2.0, served at 1024 px on their long
# side from the dataset's public bucket), downloaded once into RAEA_BENCH_CACHE (default ~/.cache/raea-bench/index,
# about 300 KB a picture). Precision@5 asks "a photo of a <label>" for each label in LABELS with at least 10 pictures in
# the set, and counts a hit right when Open Images' human-verified labels (CC BY 4.0) give the picture that label.
# The labels aren't exhaustive (a dog nobody labelled counts as a miss), so the figure is a floor, for comparing runs.
# Each run indexes into a new home; nothing touches ~/.raea/library/fast.
set -uo pipefail
RAEA=${1:-raea}
N=${2:-10000}
C=${RAEA_BENCH_CACHE:-$HOME/.cache/raea-bench/index}
OI=https://storage.googleapis.com/openimages
LABELS="Dog Cat Car Bird Flower Tree Building Boat Horse Bicycle Airplane Food Guitar Train Bus Pizza Clock Chair Book
Sunglasses Skyscraper Beer Cake Butterfly Fish Motorcycle Truck Tower Castle Wine Coffee Hat Drum Piano Cattle Sheep
Elephant Lighthouse Fountain Tent Umbrella Laptop Table Bed Window Sculpture"
command -v jq >/dev/null || { echo "bench/index: needs jq" >&2; exit 2; }
now() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; }
mkdir -p "$C/pictures" || exit 2
cd "$C" || exit 2
for f in 2018_04/validation/validation-images-with-rotation.csv v5/validation-annotations-human-imagelabels-boxable.csv v5/class-descriptions-boxable.csv; do
  [ -s "${f##*/}" ] || curl -sfL -o "${f##*/}" "$OI/$f" || { echo "bench/index: couldn't fetch $OI/$f" >&2; exit 2; }
done
tail -n +2 validation-images-with-rotation.csv | cut -d, -f1 | LC_ALL=C sort | head -n "$N" > ids.txt
[ "$(wc -l < ids.txt)" -eq "$N" ] || { echo "bench/index: the set has fewer than $N pictures" >&2; exit 2; }
# the pictures not here yet, 16 at a time
missing=$(while read -r id; do [ -s "pictures/$id.jpg" ] || echo "$id"; done < ids.txt)
if [ -n "$missing" ]; then
  echo "bench/index: downloading $(wc -l <<< "$missing" | tr -d ' ') pictures into $C/pictures"
  rm -f pictures/*.part
  xargs -P 16 -I{} sh -c 'curl -sf -o "pictures/{}.part" "https://s3.amazonaws.com/open-images-dataset/validation/{}.jpg" && mv "pictures/{}.part" "pictures/{}.jpg" && echo {}' <<< "$missing" \
    | awk 'NR % 500 == 0 { print "  " NR " downloaded"; fflush() }'
fi
lost=$(while read -r id; do [ -s "pictures/$id.jpg" ] || echo "$id"; done < ids.txt | wc -l | tr -d ' ')
[ "$lost" -le $((N / 100)) ] || { echo "bench/index: $lost of $N pictures couldn't be downloaded" >&2; exit 2; }
# the run's own folder of the set (links, so the cache's folder can hold more than N)
W=$(mktemp -d "${TMPDIR:-/tmp}/ixbench.XXXX")
trap 'rm -rf "${W:?}"' EXIT
mkdir "$W/set"
sed 's/$/.jpg/' ids.txt | (cd pictures && xargs -J % ln % "$W/set/" 2>/dev/null)
have=$(ls "$W/set" | wc -l | tr -d ' ')

# the fast pass, from a cold start: the command's own time, its peak memory, and its progress lines
t0=$(now)
/usr/bin/time -l "$RAEA" index add "$W/set" --json --home "$W/home" > "$W/add.jsonl" 2> "$W/add.err"
code=$?
t1=$(now)
[ $code -eq 0 ] || { echo "bench/index: raea index add exited $code" >&2; tail -5 "$W/add.err" >&2; exit 1; }
last=$(grep '"stage":"done"' "$W/add.jsonl" | tail -1)
[ -n "$last" ] || { echo "bench/index: the fast pass never said done" >&2; tail -3 "$W/add.jsonl" >&2; exit 1; }
done_n=$(jq -r .done <<< "$last"); failed=$(jq -r .failed <<< "$last"); rate=$(jq -r .per_sec <<< "$last"); secs=$(jq -r .seconds <<< "$last")
first=$(jq -r 'select(.done > 0) | .seconds' "$W/add.jsonl" | head -1)
peak=$(awk '/maximum resident set size/ {print $1}' "$W/add.err")
wall=$(perl -e "printf '%.1f', $t1 - $t0")

# precision@5 and warm search time, through serve (the text tower loaded once, as the app keeps it)
tr -d '\r' < class-descriptions-boxable.csv > "$W/classes.csv"
awk -F, 'NR > 1 && $4 == 1 {print $1, $3}' validation-annotations-human-imagelabels-boxable.csv | tr -d '\r' | LC_ALL=C sort -u > "$W/pos.txt"
LC_ALL=C sort ids.txt > "$W/ids.sorted"
asks="$W/asks.jsonl"; : > "$asks"; : > "$W/asked.txt"
echo '{"id":"warm","op":"warm"}' >> "$asks"
for l in $LABELS; do
  m=$(awk -F, -v n="$l" '$2 == n {print $1}' "$W/classes.csv")
  [ -n "$m" ] || { echo "bench/index: no class $l" >&2; continue; }
  n=$(awk -v m="$m" '$2 == m {print $1}' "$W/pos.txt" | LC_ALL=C join - "$W/ids.sorted" | wc -l | tr -d ' ')
  [ "$n" -ge 10 ] || continue
  q="a photo of a $(tr '[:upper:]' '[:lower:]' <<< "$l")"
  jq -nc --arg id "$l" --arg q "$q" '{id: $id, op: "search", q: $q, k: 5}' >> "$asks"
  echo "$l $m $n" >> "$W/asked.txt"
done
"$RAEA" index serve --home "$W/home" < "$asks" > "$W/answers.jsonl" 2> "$W/serve.err"
right=0; asked=0; : > "$W/ms.txt"
while read -r l m n; do
  a=$(jq -c --arg id "$l" 'select(.id == $id)' "$W/answers.jsonl")
  [ -n "$a" ] || { echo "bench/index: no answer for $l" >&2; continue; }
  hits=$(jq -r '.hits[].name | sub("\\.jpg$"; "")' <<< "$a")
  r=$(for h in $hits; do grep -qx "$h $m" "$W/pos.txt" && echo 1; done | wc -l | tr -d ' ')
  right=$((right + r)); asked=$((asked + 1))
  jq -r '.ms.words + .ms.search' <<< "$a" >> "$W/ms.txt"
  [ -n "${IXBENCH_VERBOSE:-}" ] && echo "  $l ($n in the set): $r of 5"
done < "$W/asked.txt"
p5=$(perl -e "printf '%.3f', $right / (5 * ($asked || 1))")
ms=$(sort -n "$W/ms.txt" | awk '{a[NR] = $1} END {printf "%.1f", a[int((NR + 1) / 2)]}')

# videos: two Blender open films (Elephants Dream, CC BY 2.5; Tears of Steel, CC BY 3.0; 720p H.264) cut once into
# 30 s clips without re-encoding, as a library of short clips is; the fast pass reads each clip's poster and a frame
# every 4 s (IX2). IXBENCH_NO_VIDEO=1 skips them
vids=0; vmin=0; vwall=0
if [ -z "${IXBENCH_NO_VIDEO:-}" ]; then
  command -v ffmpeg >/dev/null || { echo "bench/index: the videos need ffmpeg (or IXBENCH_NO_VIDEO=1)" >&2; exit 2; }
  mkdir -p clips
  if [ -z "$(ls clips 2>/dev/null)" ]; then
    for f in ED/elephantsdream-720-h264-st-aac.mov demo/movies/ToS/tears_of_steel_720p.mov; do
      src="$W/${f##*/}"
      echo "bench/index: fetching https://download.blender.org/$f"
      curl -sfL -o "$src" "https://download.blender.org/$f" || { echo "bench/index: couldn't fetch $f" >&2; exit 2; }
      ffmpeg -v error -i "$src" -map 0:v:0 -map 0:a:0? -c copy -f segment -segment_time 30 -reset_timestamps 1 "clips/${f##*/}-%03d.mov" || exit 2
      rm -f "$src"
    done
  fi
  vids=$(ls clips | wc -l | tr -d ' ')
  vmin=$(for c in clips/*.mov; do ffprobe -v error -show_entries format=duration -of csv=p=0 "$c"; done | awk '{s += $1} END {printf "%.1f", s / 60}')
  mkdir "$W/clips" && ln clips/*.mov "$W/clips/"
  t0=$(now)
  "$RAEA" index add "$W/clips" --json --home "$W/vhome" > "$W/vadd.jsonl" 2> "$W/vadd.err" || { echo "bench/index: raea index add (videos) failed" >&2; tail -5 "$W/vadd.err" >&2; exit 1; }
  t1=$(now)
  vdone=$(grep '"stage":"done"' "$W/vadd.jsonl" | tail -1 | jq -r .done)
  [ "$vdone" = "$vids" ] || echo "bench/index: only ${vdone:-0} of $vids clips searchable" >&2
  vwall=$(perl -e "printf '%.1f', $t1 - $t0")
fi

echo "bench/index: $have pictures (Open Images V4 validation, first $N by id), $RAEA ($("$RAEA" --version 2>/dev/null | head -1))"
echo "  fast pass        $done_n searchable, $failed failed: $(printf '%.0f' "$rate") pictures a second as the indexer counts ($(printf '%.1f' "$secs") s)"
echo "  from cold start  $wall s for the command, $(perl -e "printf '%.0f', $done_n / $wall") pictures a second; the first searchable at $(printf '%.2f' "${first:-0}") s"
echo "  peak memory      $(perl -e "printf '%.0f', ${peak:-0} / 1048576") MB"
echo "  search           $ms ms warm (median of $asked; words embedded and every row scored)"
echo "  precision@5      $p5 over $asked labels (a floor: Open Images' labels aren't exhaustive)"
[ "$vids" -gt 0 ] && echo "  videos           $vids clips ($vmin min) in $vwall s from cold start: $(perl -e "printf '%.1f', $vmin / $vwall") video minutes a second, $(perl -e "printf '%.1f', $vids / $vwall") clips a second"
jq -nc --argjson n "$have" --argjson done "$done_n" --argjson rate "$rate" --argjson wall "$wall" --argjson first "${first:-0}" \
  --argjson peak "${peak:-0}" --argjson ms "$ms" --argjson p5 "$p5" --argjson asked "$asked" \
  --argjson vids "$vids" --argjson vmin "$vmin" --argjson vwall "$vwall" \
  '{pictures: $n, searchable: $done, per_sec: $rate, wall_s: $wall, first_s: $first, peak_bytes: $peak, search_ms: $ms, p5: $p5, labels: $asked, clips: $vids, clip_minutes: $vmin, clips_wall_s: $vwall}'
