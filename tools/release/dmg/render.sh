#!/bin/bash
# The installer window's picture, rendered by raea from background.tsx: the 1x and 2x stills joined into one
# multi-resolution TIFF (background.tiff, which dmg.sh puts in the image), so Finder draws it sharp on any display.
# With --loop OUT.mp4 it also renders the same scene moving (the leap, 3 s at 60 fps), for places that can play it:
# Finder only ever shows a window's picture still.
#   tools/release/dmg/render.sh [--loop OUT.mp4]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
loop=
[ "${1:-}" = --loop ] && loop=${2:?usage: render.sh [--loop OUT.mp4]}
work=$(mktemp -d)
trap 'rm -rf "${work:?}"' EXIT
raea render "$here/background.tsx" --composition dmg-1x --frame 0 -o "$work/background.png"
raea render "$here/background.tsx" --composition dmg-2x --frame 0 -o "$work/background@2x.png"
# Finder reads the 2x picture as 144 dpi
sips -s dpiWidth 72 -s dpiHeight 72 "$work/background.png" >/dev/null
sips -s dpiWidth 144 -s dpiHeight 144 "$work/background@2x.png" >/dev/null
tiffutil -cathidpicheck "$work/background.png" "$work/background@2x.png" -out "$here/background.tiff"
tiffutil -info "$here/background.tiff" | grep -E "Image Width|Resolution:"
if [ -n "$loop" ]; then
  raea render "$here/background.tsx" --composition dmg-loop -o "$loop" --quality master
fi
echo "ok $here/background.tiff"
