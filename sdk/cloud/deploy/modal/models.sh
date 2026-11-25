#!/bin/bash
# The models raea runs on Linux, from this Mac into the Modal volume raea-models (docs/cloud-render.md, "Models"), for
# the farm's GPUs and a cloud box to mount instead of fetching them on each cold start:
#
#   MODAL_PROFILE=raea deploy/modal/models.sh            # stage and upload
#   deploy/modal/models.sh --dry                                 # stage only, and list what would go
#
# /raea     perception's folder (RAEA_MODELS): the ONNX and data files no public source serves as raea runs them;
#           Core ML and MLX ports stay on the Mac
# /library  the app library's (RAEA_LIBRARY_MODELS): TrustMark
# /feed     raea's model cache (RAEA_FEED_MODELS), left to raea: it fetches each model from Hugging Face on first use
#           (CLAP, SigLIP 2, EmbeddingGemma 2, Parakeet, Pocket TTS, ...) and the farm keeps what it fetched
#   --all   sends this Mac's copies of those too (6 GB: slow from a home connection)
#
# Staged as APFS clones (no disk), so the upload never reads a file another session is writing.
set -euo pipefail
dry=${1:-}
stage=$(mktemp -d "${TMPDIR:-/tmp}/raea-models.XXXXXX")
trap 'rm -rf "${stage:?}"' EXIT

perception=${RAEA_MODELS:-$HOME/.local/share/raea-models}
feed=${RAEA_FEED_MODELS:-$HOME/.cache/raea-feed/models}
library=${RAEA_LIBRARY_MODELS:-$HOME/.raea/models}

# name in perception's folder, then the parts that run on Linux
PERCEPTION=(
  "beat-this beat_this_small0.onnx"
  "htdemucs htdemucs_core.onnx"
  "silero-vad silero_vad.onnx"
  "transnetv2 transnetv2.onnx LICENSE SOURCE.txt"
  "yamnet yamnet.onnx yamnet_class_map.csv"
  "soma body mannequin LICENSE SOMA_neutral.npz SOMA_template_rig.usda"
  "garments garment-words.npz soma_sdf.npz"
  "overlay-seg overlay-seg.bpk cut.json"
)
# the large ones a public source serves (Hugging Face): sent only with --all
[ "$dry" = --all ] && PERCEPTION+=(
  "siglip2-base-p16-224 onnx tokenizer.json tokenizer_config.json special_tokens_map.json config.json preprocessor_config.json"
  "siglip2-base-p16-256 onnx tokenizer.json tokenizer_config.json special_tokens_map.json config.json preprocessor_config.json"
  "nomic-embed-v1.5 text"
)
for row in "${PERCEPTION[@]}"; do
  read -r name parts <<<"$row"
  for p in $parts; do
    [ -e "$perception/$name/$p" ] || { echo "models: $perception/$name/$p isn't here, left out"; continue; }
    mkdir -p "$stage/raea/$name"
    cp -cR "$perception/$name/$p" "$stage/raea/$name/"
  done
done
[ "$dry" = --all ] && for name in clap-htsat-unfused clap-htsat-unfused-fp32 parakeet-tdt-0.6b-v3 pocket-tts silero-vad smart-turn-v3; do
  [ -d "$feed/$name" ] && { mkdir -p "$stage/feed"; cp -cR "$feed/$name" "$stage/feed/"; } || echo "models: $feed/$name isn't here, left out"
done
library_sets=(trustmark)
[ "$dry" = --all ] && library_sets+=(onnx-community/embeddinggemma-2-ONNX onnx-community/whisper-base)
for name in "${library_sets[@]}"; do
  [ -d "$library/$name" ] && { mkdir -p "$stage/library/$(dirname "$name")"; cp -cR "$library/$name" "$stage/library/$(dirname "$name")/"; } || echo "models: $library/$name isn't here, left out"
done
# Core ML, MLX and anything the Mac made for itself
find "$stage" \( -name '*.mlpackage' -o -name '*.mlmodelc' -o -name coreml -o -name mlx -o -name '.DS_Store' \) -prune -exec rm -rf {} +

du -sh "$stage"/* | sed "s|$stage/||"
[ "$dry" = --dry ] && { find "$stage" -type f | sed "s|$stage/||" | sort; exit 0; }
. "$(dirname "$0")/guard.sh"
modal_guard
modal volume create raea-models --version 2 2>/dev/null || true
for top in raea feed library; do
  [ -d "$stage/$top" ] && modal volume put --force raea-models "$stage/$top" "/$top"
done
modal volume ls raea-models /
