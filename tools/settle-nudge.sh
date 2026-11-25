#!/bin/bash
# #158: a frame a render's runtime loses is asked for again, and one lost for good fails with a named error instead of
# a render that waits forever. RAEA_FAULT_DROP_EVENT (runtime/react thread.rs) loses the 3D rows' raea-frames event:
#   1. the first one only: the render finishes, saying it asked for the frames again
#   2. every one: the render fails within a minute, naming the frame and that its JS thread sat idle
# Usage: tools/settle-nudge.sh [RAEA_BINARY]   (default: raea on PATH)
set -uo pipefail
RAEA=${1:-raea}
T=$(mktemp -d "${TMPDIR:-/tmp}/settle-nudge.XXXX")
trap 'rm -rf "${T:?}"' EXIT
# a command killed (SIGALRM, exit 142) once it has run SECS: macOS has no timeout(1)
limit() { local s=$1; shift; perl -e 'alarm shift @ARGV; exec @ARGV or die "exec: $!"' "$s" "$@"; }
cat >"$T/comp.tsx" <<'EOF'
import React from "react";
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
const C = () => {
  const f = useCurrentFrame();
  return <AbsoluteFill style={{ background: "#123" }}><div style={{ position: "absolute", left: f % 200, top: 40, width: 40, height: 40, background: "#fc0" }} /></AbsoluteFill>;
};
registerRoot(() => <Composition id="nudge" component={C} width={320} height={180} fps={30} durationInFrames={600} />);
EOF
fail=0
# 1: the first raea-frames lost
out=$(cd "$T" && RAEA_AUDIO_MUTE=1 RAEA_FAULT_DROP_EVENT=raea-frames limit 120 "$RAEA" render comp.tsx -o "$T/one.png" --frame 300 2>&1)
code=$?
if [ $code -eq 0 ] && [ -s "$T/one.png" ] && grep -q "asked for frames" <<<"$out"; then
  echo "ok 1: a lost frame was asked for again and the render finished"
else
  echo "FAIL 1: exit $code (142 is the time limit: it hung)"; tail -5 <<<"$out"; fail=1
fi
# 2: every raea-frames lost
t0=$(date +%s)
out=$(cd "$T" && RAEA_AUDIO_MUTE=1 RAEA_FAULT_DROP_EVENT='raea-frames*' limit 150 "$RAEA" render comp.tsx -o "$T/two.png" --frame 300 2>&1)
code=$?
dt=$(( $(date +%s) - t0 ))
if [ $code -ne 0 ] && [ $code -ne 142 ] && grep -q "made no progress on frame" <<<"$out" && grep -q "idle for" <<<"$out"; then
  echo "ok 2: a frame lost for good failed in ${dt}s, naming it"
else
  echo "FAIL 2: exit $code after ${dt}s (142 is the time limit: it hung)"; tail -5 <<<"$out"; fail=1
fi
exit $fail
