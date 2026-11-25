// A 2D piece with one control (docs/realtime.feature RT0a): a title card whose background hue is a prop, its words
// sliding in and a bar filling over 5 s. No 3D, no media, no randomness: its frame is a function of the frame number
// and `hue` alone, so the stage and a render of the same frame and props must be the same pixels.
import { AbsoluteFill, Composition, interpolate, registerRoot, useCurrentFrame } from "raea";

function Title({ hue = 210 }: { hue?: number }) {
  const f = useCurrentFrame();
  const y = interpolate(f, [0, 24], [60, 0], { extrapolateRight: "clamp" });
  const bar = interpolate(f, [0, 150], [0, 1], { extrapolateRight: "clamp" });
  return (
    <AbsoluteFill style={{ background: `hsl(${hue}, 55%, 28%)`, alignItems: "center", justifyContent: "center" }}>
      <div style={{ fontFamily: "Helvetica, Arial, sans-serif", fontSize: 120, fontWeight: 700, color: "#fff", transform: `translateY(${y}px)`, opacity: Math.min(1, f / 18) }}>
        Live controls
      </div>
      <div style={{ position: "absolute", left: 160, right: 160, bottom: 120, height: 14, borderRadius: 7, background: "rgba(255,255,255,0.2)" }}>
        <div style={{ width: `${bar * 100}%`, height: "100%", borderRadius: 7, background: `hsl(${(hue + 160) % 360}, 80%, 62%)` }} />
      </div>
      <div style={{ position: "absolute", left: 160 + bar * 800, top: 140, width: 80, height: 80, borderRadius: 40, background: "#fff", opacity: 0.85, transform: `rotate(${f * 3}deg)` }} />
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="title" component={Title} defaultProps={{ hue: 210 }} width={1280} height={720} fps={30} durationInFrames={150} />);
