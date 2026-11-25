#!/bin/bash
# This machine's logins for raea's own clouds, beside any others it has and without changing their defaults
# (docs/cloud-render.feature CR19): a Modal profile for raea's workspace, and a Sprites token for raea's Fly org, whose
# name goes in ~/.config/raea-cloud/sprites.env (where deploy/sprites/box.sh reads it).
#
#   sdk/cloud/deploy/login.sh          each one that's missing:
#                                        Modal: a browser page asks you to approve; pick the raea workspace there.
#                                        Sprites: first make a token at https://fly.io/dashboard/raea/sprites/tokens and
#                                        copy it. It's read from the clipboard (never typed, so it stays out of chats
#                                        and shell history), then the clipboard is cleared.
#   sdk/cloud/deploy/login.sh check    what this machine reaches, changing nothing
#
# RAEA_MODAL_WORKSPACE and RAEA_SPRITES_ORG (both raea) name them.
set -euo pipefail
WS=${RAEA_MODAL_WORKSPACE:-raea}
ORG=${RAEA_SPRITES_ORG:-raea}
CONF=$HOME/.config/raea-cloud/sprites.env
DENIED=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null)/private/cloud/sprites-denied.txt

profile_ws() {
  modal profile list --json 2>/dev/null | python3 -c 'import json, sys; print(next((x["workspace"] for x in json.load(sys.stdin) if x["name"] == sys.argv[1]), ""))' "$WS"
}
has_org() { sprite org list 2>/dev/null | grep -qE "^ *[0-9]+\. $ORG( |$)"; }
# a profile named raea whose token reaches another workspace would send raea's deploys there: it goes
drop_profile() {
  python3 - "$HOME/.modal.toml" "$WS" <<'EOF'
import re, sys
path, name = sys.argv[1], sys.argv[2]
text = open(path).read()
open(path, "w").write(re.sub(r"(?ms)^\[" + re.escape(name) + r"\]\n.*?(?=^\[|\Z)", "", text))
EOF
}

check() {
  local w
  w=$(profile_ws)
  if [ "$w" = "$WS" ]; then echo "modal: profile $WS reaches workspace $WS"; else echo "modal: no profile for workspace $WS"; fi
  if has_org; then echo "sprites: a token for org $ORG"; else echo "sprites: no token for org $ORG"; fi
  if [ -f "$CONF" ]; then echo "box.sh org: $(sed -n 's/^RAEA_SPRITES_ORG=//p' "$CONF")"; else echo "box.sh org: not set ($CONF)"; fi
}

if [ "${1:-}" = check ]; then
  check
  exit 0
fi

if [ "$(profile_ws)" != "$WS" ]; then
  echo "modal: approve in the browser page that opens, with the $WS workspace picked"
  modal token new --profile "$WS" --no-activate
  w=$(profile_ws)
  if [ "$w" != "$WS" ]; then
    drop_profile
    echo "modal: that approval was for workspace ${w:-unknown}, not $WS, so the profile was removed; run again and pick $WS"
    exit 1
  fi
fi

if ! has_org; then
  t=$(pbpaste 2>/dev/null | tr -d '[:space:]')
  case "$t" in
    "" | *" "*) echo "sprites: copy a token from https://fly.io/dashboard/$ORG/sprites/tokens, then run this again"; exit 1 ;;
    */*) ;;
    *) echo "sprites: the clipboard doesn't hold a Sprites token (they look like ORG/...); copy one and run this again"; exit 1 ;;
  esac
  if [ -f "$DENIED" ] && grep -qxF "${t%%/*}" "$DENIED"; then
    echo "sprites: that token is for ${t%%/*}, which belongs to another business; make one in $ORG"
    exit 1
  fi
  sprite auth setup --token "$t" >/dev/null
  printf '' | pbcopy
  has_org || { echo "sprites: the token was taken, but org $ORG isn't among this machine's orgs (sprite org list)"; exit 1; }
fi

mkdir -p "$(dirname "$CONF")"
(umask 077 && echo "RAEA_SPRITES_ORG=$ORG" >"$CONF")
check
