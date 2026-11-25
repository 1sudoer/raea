#!/bin/bash
# raea's Modal apps in one workspace, made from nothing (docs/cloud-render.feature CR20). From sdk/cloud:
#
#   MODAL_PROFILE=raea deploy/modal/workspace.sh          secrets, volumes, weights and the apps, each one checked
#   MODAL_PROFILE=raea deploy/modal/workspace.sh check    the apps' health only
#
# What it makes in the workspace: the secrets raea-farm (a new token) and raea-made (MADE_API_KEY from ~/.raea/env);
# the volume raea-models (models.sh); TripoSG in raea-headgen-weights, fetched inside Modal (weights.py); and the apps
# raea-farm, raea-step and raea-comfy (CPU). Their URLs go in ~/.config/raea-cloud/modal-PROFILE.env. On this machine,
# once each answers, ~/.config/raea-cloud/farm.env names the new farm and ~/.raea/env's RAEA_MODAL_UPSCALE the new
# raea-step. ComfyUI weights a workflow needs go in with `modal run tools/comfy/modal_comfy.py::needs` (gen.feature).
set -euo pipefail
cd "$(dirname "$0")/../.."
. deploy/modal/guard.sh
modal_guard
P=$MODAL_PROFILE
ROOT=$(git rev-parse --show-toplevel)
ENVF=${RAEA_ENV_FILE:-$HOME/.raea/env}
CONF=$HOME/.config/raea-cloud
URLS=$CONF/modal-$P.env
tmp=$(mktemp -d)
trap 'rm -rf "${tmp:?}"' EXIT
mkdir -p "$CONF"

say() { echo "workspace $P: $*"; }
# a deploy's first web URL in this workspace
deploy() {
  local out
  out=$(modal deploy "$@" 2>&1) || { echo "$out" | tail -20; return 1; }
  echo "$out" | grep -oE "https://$P--[a-z0-9-]+(\.[a-z0-9-]+)*\.modal\.run" | head -1
}
# KEY=VALUE into a file only this user reads
keep() { (umask 077 && touch "$1" && { grep -v "^$2=" "$1" >"$1.new" || true; } && echo "$2=$3" >>"$1.new" && mv "$1.new" "$1"); }

check() {
  local ok=0 farm step comfy png
  [ -f "$URLS" ] || { say "nothing deployed yet ($URLS)"; return 1; }
  farm=$(sed -n 's/^RAEA_FARM_URL=//p' "$URLS")
  step=$(sed -n 's/^RAEA_STEP_URL=//p' "$URLS")
  comfy=$(sed -n 's/^RAEA_COMFY_URL=//p' "$URLS")
  if curl -fsS -m 120 "$farm/healthz" >/dev/null; then say "raea-farm answers at $farm"; else say "raea-farm doesn't answer"; ok=1; fi
  # a 2x2 PNG through image.upscale: the contract io/gen/src/modal.rs speaks
  png=$(ffmpeg -loglevel error -f lavfi -i color=c=gray:s=2x2 -frames:v 1 -f image2pipe -vcodec png - | base64 | tr -d '\n')
  if curl -fsS -m 300 -H 'content-type: application/json' -d "{\"op\":\"image.upscale\",\"inputs\":{\"image\":\"data:image/png;base64,$png\",\"factor\":2}}" "$step" | grep -q '"files"'; then
    say "raea-step upscales at $step"
  else
    say "raea-step doesn't answer"
    ok=1
  fi
  if curl -fsS -m 600 "$comfy/system_stats" | grep -q comfyui_version; then say "raea-comfy answers at $comfy"; else say "raea-comfy doesn't answer"; ok=1; fi
  return $ok
}

if [ "${1:-}" = check ]; then
  check
  exit $?
fi

# secrets: a new farm token; made.app's key from this machine's raea settings, never printed
T=$(openssl rand -hex 32)
(umask 077 && echo "RAEA_CLOUD_TOKEN=$T" >"$tmp/farm.env")
modal secret create --force raea-farm --from-dotenv "$tmp/farm.env" >/dev/null
K=$(sed -n 's/^MADE_API_KEY=//p' "$ENVF" 2>/dev/null | tail -1)
if [ -n "$K" ]; then
  (umask 077 && echo "MADE_API_KEY=$K" >"$tmp/made.env")
  modal secret create --force raea-made --from-dotenv "$tmp/made.env" >/dev/null
else
  say "no MADE_API_KEY in $ENVF, so raea-comfy goes up without made.app's nodes"
  export RAEA_MADE_SECRET=0
fi
say "secrets raea-farm and raea-made set"

# raea's models (once: models.sh uploads them again on its own), and the head generator's weights (fetched inside Modal)
if modal volume ls raea-models /raea 2>/dev/null | grep -q .; then say "raea-models already holds raea's models"; else deploy/modal/models.sh; fi
RAEA_WEIGHTS_VOLUME=raea-headgen-weights modal run deploy/modal/weights.py --repo VAST-AI/TripoSG --dest TripoSG 2>&1 | grep --line-buffered -E "^weights:|rror"

# the apps
farm=$(deploy deploy/modal/farm.py)
step=$(deploy "$ROOT/tools/comfy/modal_step.py")
comfy=$(deploy "$ROOT/tools/comfy/modal_comfy.py")
for kv in "RAEA_FARM_URL=$farm" "RAEA_STEP_URL=$step" "RAEA_COMFY_URL=$comfy"; do keep "$URLS" "${kv%%=*}" "${kv#*=}"; done
say "deployed: $farm, $step, $comfy"

check || { say "not everything answers, so this machine still points at the old ones"; exit 1; }
keep "$CONF/farm.env" RAEA_CLOUD_GPU_URL "$farm"
keep "$CONF/farm.env" RAEA_CLOUD_GPU_TOKEN "$T"
keep "$ENVF" RAEA_MODAL_UPSCALE "$step"
say "this machine now renders on $farm and upscales on $step"
