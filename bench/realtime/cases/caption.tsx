// HTML over 3D with one control (docs/realtime.feature RT0a): raster.tsx's scene under a caption and a lower third
// drawn by the page, the hero cube's colour a prop. The page's text over 3D is where play and render already differ by
// a level or two (docs/rerender.feature RR1: SSIM >= 0.999473).
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
import { Scene } from "./scene";

function Caption({ color = "#d04a3a" }: { color?: string }) {
  const f = useCurrentFrame();
  return (
    <AbsoluteFill style={{ background: "#9fb7c8" }}>
      <Scene color={color} />
      <div style={{ position: "absolute", left: 60, bottom: 70, padding: "14px 22px", borderRadius: 10, background: "rgba(12,16,24,0.72)", fontFamily: "Helvetica, Arial, sans-serif", color: "#fff" }}>
        <div style={{ fontSize: 18, letterSpacing: "0.25em", opacity: 0.75 }}>SHOT 1</div>
        <div style={{ fontSize: 44, fontWeight: 700, opacity: Math.min(1, f / 15) }}>The hero cube</div>
      </div>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="caption" component={Caption} defaultProps={{ color: "#d04a3a" }} width={1280} height={720} fps={30} durationInFrames={150} />);
