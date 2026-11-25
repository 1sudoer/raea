#!/usr/bin/env bash
# R3F parity (docs/r3f.feature R3F8): each case in test/parity/fixtures.tsx drawn by raea's engine (`raea render`) and
# by three.js in headless Chrome (test/parity/page.tsx, served by tools/serve.ts, or at $R3F_PARITY_URL), compared by
# SSIM. Fails under $MIN (0.92). Needs raea, Chrome, ImageMagick and sdk/r3f/wasm (tools/shape-wasm.sh builds it).
#   tools/parity.sh [OUT_DIR]
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=${1:-$(mktemp -d)}
URL=${R3F_PARITY_URL:-serve}
[ -f wasm/raea_shape_web.js ] || tools/shape-wasm.sh
MIN=${MIN:-0.92}
CASES=(in-basic:24 in-instanced:12 in-lights:0 in-things:0 in-catch:0 in-lettra:12 in-lettrafx:12 in-shader:12 in-post:0 in-drei:12 in-approx:0 in-glass:0 in-sky:0 out-basic:24 out-tony:24 out-lights:0 out-bloom:0 out-grade:0 out-lens:0 out-catch:0 out-entities:10 src-basic:24 src-tony:24 src-lights:0 src-bloom:0 src-grade:0 src-lens:0 src-catch:0 src-entities:10)
# R3F_CASES="in-sky:0 in-shader:12": just those (a lane checking its own cases)
# the TSL cases (test/parity/tsl.tsx): a lit node material (R3F21)
CASES+=(in-tsl-lit:12)
# TSL compute: particles a compute moves, drawn by a node material on an InstancedMesh (R3F22)
CASES+=(in-tsl-particles:12)
# R3F22: a node material reading a target raea draws on its GPU (test/parity/tsl.tsx)
CASES+=(in-tsl-fbo:12)
# a plain three.js program (test/parity/programs.ts) through <Three> (R3F19)
CASES+=(in-vanilla:24)
# postprocessing's effects as raea <FramePass>es (test/parity/post2.tsx), before and after an ACES tone mapping (R3F25)
CASES+=(in-post2:6 in-post3:6 in-shaderbloom:6)
# three/webgpu's PostProcessing: its scene pass and TSL nodes as raea's effects (test/parity/tslpost.tsx, R3F25)
CASES+=(in-tslpost:6 in-tslpost2:6)
# render targets an app draws itself, on raea's GPU: a useFBO, and GPUComputationRenderer's ping-pong (test/parity/targets.tsx, R3F23)
CASES+=(in-fbo:12 in-pingpong:12)
# materials patched by onBeforeCompile (drei's wobble, distort and reflector, an app's waves): three's own program with
# the patch, lit by raea (test/parity/patched.tsx, R3F24)
CASES+=(in-patched:12 in-reflector:12)
[ -z "${R3F_CASES:-}" ] || read -ra CASES <<<"$R3F_CASES"
mkdir -p "$OUT"
for c in "${CASES[@]}"; do
  # a src-* case is its out-* composition, drawn from the R3F source the source export wrote for it
  (cd test/parity && raea render native.tsx --composition "$(echo "${c%%:*}" | sed 's/^src-/out-/')" --frame "${c##*:}" -o "$OUT/${c%%:*}-native.png" >/dev/null)
done
# Chrome shoots exactly these cases (shoot.ts's own list is for running it alone)
bun tools/shoot.ts "$URL" "$OUT" $(for c in "${CASES[@]}"; do echo "${c%%:*}"; done)
fail=0
for c in "${CASES[@]}"; do
  id=${c%%:*}
  magick "$OUT/$id-three.png" -crop 640x360+0+0 +repage "$OUT/$id-three.png"
  # ImageMagick 7.1.2 reports SSIM's distance (1 - SSIM)
  d=$(magick compare -metric SSIM "$OUT/$id-native.png" "$OUT/$id-three.png" null: 2>&1 | sed -E 's/.*\(([0-9.e-]+)\).*/\1/' || true)
  s=$(echo "1 - $d" | bc -l)
  ok=$(echo "$s >= $MIN" | bc -l)
  printf '%-11s SSIM %.4f %s\n' "$id" "$s" "$([ "$ok" = 1 ] && echo ok || echo LOW)"
  [ "$ok" = 1 ] || fail=1
done
exit $fail
