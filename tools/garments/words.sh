#!/bin/bash
# Words to garments on SOMA through the garment model (docs/character.feature CH48c): each line becomes a GarmentCode
# design (garment_model.py make), cut to SOMA, draped on the GPU, baked and pictured, and one sheet of them all.
#   tools/garments/words.sh MODEL.npz OUT_DIR "a pencil skirt" "a top with a hood" ...
# Needs what engine.sh needs (GARMENTCODE, GARMENT_PY, GARMENT_DATA). $0, about 20 s a line.
set -euo pipefail
T=$(cd "$(dirname "$0")" && pwd)
GC=${GARMENTCODE:?a GarmentCode clone, see tools/garments/README.md}
PY=${GARMENT_PY:?the venv python, see tools/garments/README.md}
DATA=${GARMENT_DATA:-$HOME/.cache/raea-garments}
BODY=${SOMA_BODY:-$HOME/.local/share/raea-models/soma/body/body.glb}
MODEL=${1:?model}; OUT=${2:?out dir}; shift 2
mkdir -p "$OUT"
DIRS=()
i=0
for words in "$@"; do
  i=$((i + 1)); n=w$(printf %02d $i); d=$OUT/$n
  mkdir -p "$d"; printf '%s\n' "$words" > "$d/words.txt"
  "$PY" -I "$T/garment_model.py" make "$MODEL" "$words" "$d" | grep -E "^make:"
  (cd "$T" && DESIGN="$d/design.yaml" GID=$n "$PY" -I sample_designs.py "$GC" "$T/soma.yaml" "$OUT" 1 1) | grep -E "^sample: (w|rejected)" | cut -c1-160
  # refused (a combination GarmentCode's checks don't allow): the training design nearest the words instead
  if [ ! -f "$d/$n.panels.json" ] && [ -f "$d/nearest.yaml" ]; then
    echo "words: $n ($words): GarmentCode refused the design; the nearest training design instead"
    cp "$d/nearest.yaml" "$d/design.yaml"; echo nearest > "$d/fallback.txt"
    (cd "$T" && DESIGN="$d/design.yaml" GID=$n "$PY" -I sample_designs.py "$GC" "$T/soma.yaml" "$OUT" 1 1) | grep -E "^sample: (w|rejected)" | cut -c1-160
  fi
  [ -f "$d/$n.panels.json" ] || { echo "words: $n ($words): GarmentCode refused the design"; continue; }
  log=$( (cd "$T" && "$PY" -I drape_mlx.py "$DATA/soma_sdf.npz" "$d/$n.panels.json" "$d" 150 -0.064 "$T/soma.yaml") 2>&1 ) && grep -E '^drape: \{' <<<"$log" | cut -c1-160 \
    || echo "drape: $n failed: $(grep -vE '^\s*$' <<<"$log" | tail -1 | sed 's/Traceback/trace/')"
  [ -f "$d/$n.draped.npz" ] && DIRS+=("$d")
done
[ ${#DIRS[@]} -gt 0 ] || exit 1
raea blender run "$T/bake_batch.py" "$BODY" "${DIRS[@]}" 2>&1 | grep --line-buffered -E '^bake|Error|Traceback'
"$PY" -I "$T/sheet.py" "$OUT/sheet.jpg" "${DIRS[@]}"
