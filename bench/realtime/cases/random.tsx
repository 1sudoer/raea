// The 2D piece of title.tsx with confetti placed by Math.random and a clock read from Date (docs/realtime.feature
// RT0a): a render seeds Math.random from the composition and dates frame 0 at the render's epoch (sdk/raea/src/pure.ts),
// a live session from its own seed and start (runtime/live/src/lib.rs new_session), so this case says what the seed
// and the epoch alone cost a live frame.
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";

function Confetti({ hue = 210 }: { hue?: number }) {
  const f = useCurrentFrame();
  const dots = Array.from({ length: 40 }, (_, i) => ({ x: Math.random() * 1200, y: Math.random() * 660, r: 8 + Math.random() * 16, h: (hue + i * 9) % 360 }));
  const clock = new Date().getUTCSeconds();
  return (
    <AbsoluteFill style={{ background: `hsl(${hue}, 40%, 18%)` }}>
      {dots.map((d, i) => (
        <div key={i} style={{ position: "absolute", left: d.x, top: (d.y + f * 2) % 700, width: d.r * 2, height: d.r * 2, borderRadius: d.r, background: `hsl(${d.h}, 80%, 60%)` }} />
      ))}
      <div style={{ position: "absolute", right: 40, top: 30, fontFamily: "Helvetica, Arial, sans-serif", fontSize: 40, color: "#fff" }}>{clock}</div>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="random" component={Confetti} defaultProps={{ hue: 210 }} width={1280} height={720} fps={30} durationInFrames={150} />);
