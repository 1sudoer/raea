// RR3's page check (rr3.sh): a composition with no 3D whose shots are Sequences, three 3 s title cards at 1280x720,
// 30 fps (270 frames). Its video splits into a segment at each Sequence's start.
import { AbsoluteFill, Composition, Sequence, interpolate, registerRoot, useCurrentFrame } from "raea";

const CARDS = [
  { title: "One", bg: "#1d3557" },
  { title: "Two", bg: "#e63946" },
  { title: "Three", bg: "#2a9d8f" },
];

function Card({ title, bg }: { title: string; bg: string }) {
  const f = useCurrentFrame();
  return (
    <AbsoluteFill style={{ background: bg, alignItems: "center", justifyContent: "center" }}>
      <div style={{ fontFamily: "Helvetica, Arial, sans-serif", fontSize: 140, fontWeight: 700, color: "#fff", transform: `translateY(${interpolate(f, [0, 30], [40, 0], { extrapolateRight: "clamp" })}px)` }}>{title}</div>
    </AbsoluteFill>
  );
}

function Pages() {
  return (
    <>
      {CARDS.map((c, i) => (
        <Sequence key={i} from={i * 90} durationInFrames={90}>
          <Card {...c} />
        </Sequence>
      ))}
    </>
  );
}

registerRoot(() => <Composition id="pages" component={Pages} width={1280} height={720} fps={30} durationInFrames={270} />);
