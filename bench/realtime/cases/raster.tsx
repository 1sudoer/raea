// A raster-lit 3D piece with one control (docs/realtime.feature RT0a): a few parts on a floor under a sun, the camera
// circling them over 5 s, the hero cube's colour a prop. Colour is on the live scene's keyed list
// (runtime/scene/src/live.rs), so a running world takes the change without rebuilding the part.
import { AbsoluteFill, Composition, registerRoot } from "raea";
import { Scene } from "./scene";

function Raster({ color = "#d04a3a" }: { color?: string }) {
  return (
    <AbsoluteFill style={{ background: "#9fb7c8" }}>
      <Scene color={color} />
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="raster" component={Raster} defaultProps={{ color: "#d04a3a" }} width={1280} height={720} fps={30} durationInFrames={150} />);
