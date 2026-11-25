#!/bin/bash
# text-overflow: ellipsis cuts only text that doesn't fit. A label shrink-wrapped by its box (a grid of picker cells,
# each name centred under a dot) drew "Fr…" and "Mou…" for words that fit: the ellipsis was measured against the box's
# rounded width, up to a pixel narrower than the text it wraps (each edge rounds on its own), and blitz-paint now
# measures against the unrounded width the text was laid out in. Checked, each page rendered twice, once with
# `text-overflow: ellipsis` and once with `clip`:
#   fit    shrink-wrapped labels at many x offsets: the two renders are byte-identical (no ellipsis drawn)
#   over   a word wider than its 40 px box: the two renders differ (the ellipsis is drawn)
# Exits 1 on a failure.
#   bench/ellipsis-fit/check.sh      (RAEA_BIN: the raea to run, default target/release/raea)
cd "$(dirname "$0")/../.." || exit 2
BIN=${RAEA_BIN:-$PWD/target/release/raea}
export RAEA_SIGN=0 RAEA_MARK=0 RAEA_EXPORT_CACHE=0
OUT=$(mktemp -d "${TMPDIR:-/tmp}/raea-ellipsis-fit.XXXXXX")
trap 'rm -rf "$OUT"' EXIT
status=0
page() { # NAME OVERFLOW BODY
  cat > "$OUT/$1-$2.html" <<HTML
<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;width:640px;height:200px;background:#0a0a0a;font:11px/16px system-ui,sans-serif;color:#aaa}
.grid{display:grid;grid-template-columns:repeat(7,minmax(0,1fr));gap:3px;width:617px;padding:9px 11.5px}
.cell{display:flex;min-width:0;flex-direction:column;align-items:center;gap:4px;padding:8px 4px 6px}
.dot{width:30px;height:30px;border-radius:15px;background:#6c6}
.t{max-width:100%;overflow:hidden;white-space:nowrap;text-overflow:$2}
.narrow{width:40px;margin:20px}
</style></head><body><div id="root" data-composition-id="main" data-start="0" data-width="640" data-height="200" data-duration="1">$3</div></body></html>
HTML
  "$BIN" render "$OUT/$1-$2.html" -o "$OUT/$1-$2.png" > "$OUT/$1-$2.log" 2>&1 || { echo "FAIL $1 ($2) didn't render"; cat "$OUT/$1-$2.log"; status=1; }
}
cells=""
for w in Owl Frog Dog Mouse Bunny Bear Fox Cat Tabby Snow Rose Sky Mint Slate; do cells+="<div class=\"cell\"><span class=\"dot\"></span><span class=\"t\">$w</span></div>"; done
for ov in ellipsis clip; do
  page fit "$ov" "<div class=\"grid\">$cells</div>"
  page over "$ov" "<div class=\"narrow t\">Mousetrap</div>"
done
cmp -s "$OUT/fit-ellipsis.png" "$OUT/fit-clip.png" && echo "PASS fit: shrink-wrapped labels draw no ellipsis" || { echo "FAIL fit: an ellipsis was drawn on a label that fits"; status=1; }
cmp -s "$OUT/over-ellipsis.png" "$OUT/over-clip.png" && { echo "FAIL over: a word wider than its box drew no ellipsis"; status=1; } || echo "PASS over: a word wider than its box ends in an ellipsis"
exit $status
