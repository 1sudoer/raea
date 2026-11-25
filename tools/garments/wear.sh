#!/bin/bash
# Worn hard pieces from words (docs/character.feature CH49): SOMA's mannequin dressed by an image model, made 3D by Idol on
# this machine, laid on SOMA, the body cut away, each piece riding the bone it covers.
#   tools/garments/wear.sh NAME "what is worn" full|keyed
# full: the piece covers the whole body (a suit of armour, a space suit): the dressed surface is kept whole and lifted
#   4 mm off the skin wherever it meets it. keyed: it leaves skin bare (a hat, a helmet, a breastplate): the mannequin is
#   drawn magenta and what comes back magenta is cut.
# Needs: GARMENT_PY (the garments venv's python, see README.md), Idol's server on IDOL_URL (default 127.0.0.1:11399),
# SHEET3D (its .venv with tools/idol_serve.py and splat_mesh.py) and SPLAT_NPZ (the creature flow's splat_npz.py).
# Writes $WEAR/NAME/: art/out.jpg, idol/raw.glb, pieces.npz, NAME.glb and NAME.png. About 5 cents and 5 minutes.
set -euo pipefail
T=$(cd "$(dirname "$0")" && pwd)
NAME=${1:?name}; WORDS=${2:?what is worn}; MODE=${3:-keyed}
PY=${GARMENT_PY:?the garments venv python}
WEAR=${WEAR:-$HOME/.cache/raea-garments/wear}
BODY=${SOMA_DIR:-$HOME/Documents/raea/raea/projects/sheet3d/assets/characters/soma}
S=${SHEET3D:-$HOME/Studio/internal/projects/sheet3d}
NPZ=${SPLAT_NPZ:-$HOME/Documents/raea/raea/projects/p-new-mwhhwp6q4/tools/splat_npz.py}
URL=${IDOL_URL:-http://127.0.0.1:11399}
D=$WEAR/$NAME; M=$WEAR/_mannequin-$MODE
mkdir -p "$D/art" "$D/idol"
[ "$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 "$URL/" || true)" != 000 ] || { echo "FAILED  Idol's server is not up on $URL" >&2; exit 1; }
[ -f "$M/mannequin.png" ] || raea blender run "$T/wear_mannequin.py" "$BODY/body.glb" "$M" 30 6 1024 "$([ "$MODE" = keyed ] && echo 1 || echo 0)" | grep -E "wrote|Error"
if [ "$MODE" = keyed ]; then
  KEEP="Keep every bare part of the mannequin exactly as it is, the same flat magenta colour and shape: only the worn piece is added, nothing else is dressed."
  FIT=(0.004 0.004 1)
else
  KEEP="The piece covers the whole body."
  FIT=(-1 0.004 0)
fi
PROMPT="A studio product photograph of a physical object, not a drawing. Dress the mannequin in the image in $WORDS. Keep EXACTLY the mannequin's pose, its proportions, the place of each arm, hand, leg and foot, the head's direction and the camera view; the piece sits close on the body as it is worn. $KEEP Nothing held, no weapon, no base. Shown alone, the whole figure in frame with a margin. UNLIT: a flat albedo pass, every part in its own true colour, no cast shadows, no shading gradients, no highlights, no rim light. Plain solid light grey background (#C8C8C8), no floor, no ground shadow, no text, no watermark. Sharp detail."
[ -f "$D/art/out.jpg" ] || raea gen image "$PROMPT" --model flux_2_pro --image "$M/mannequin.png" --aspect 1:1 -o "$D/art" --max-cents 12 | tail -1
[ -f "$D/idol/raw.glb" ] || {
  "$S/.venv/bin/python" "$S/tools/idol_serve.py" "$D/art/out.jpg" "$D/idol/splat.glb" --url "$URL" --steps 100 --gaussians 1048576 --geometry splat | tail -1
  "$S/.venv/bin/python" "$NPZ" "$D/idol" | tail -1
  "$S/.venv/bin/python" "$S/tools/splat_mesh.py" "$D/idol/splat.npz" "$D/idol/raw.glb" --voxel 0.004 | tail -1
  rm -f "${D:?}/idol/splat.npz"
}
"$PY" -I "$T/wear_fit.py" "$D/idol/raw.glb" "$BODY/body.glb" "$BODY/skeleton.json" "$D/pieces.npz" "${FIT[@]}" | grep -E "^fit:|Error|Traceback"
raea blender run "$T/wear_bake.py" "$BODY/body.glb" "$D/pieces.npz" "$D/$NAME.glb" "${METAL:-0.0}" | grep -E "^wear:|Error|Traceback"
