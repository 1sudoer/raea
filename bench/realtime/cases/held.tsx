// A prop the running world can't take (docs/realtime.feature RT4g): the hero cube's size, which lands in the mesh's
// shape, a column the live scene keeps from the part's first frame (runtime/scene/src/live.rs live_dynamic). The stage
// answers stage.props with the part it held, and a written file with only the value changed is opened again rather
// than taken in place, so the frame matches the render after all.
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
import { Camera3d } from "raea/camera";
import { AmbientLight, DirectionalLight } from "raea/light";
import { Cuboid, Mesh3d } from "raea/mesh";
import { StandardMaterial } from "raea/pbr";
import { Transform } from "raea/transform";

function Held({ size = 0.6 }: { size?: number }) {
  const f = useCurrentFrame();
  const a = 0.4 + f * 0.012;
  return (
    <AbsoluteFill style={{ background: "#9fb7c8" }}>
      <Camera3d transform={Transform.fromXyz(Math.cos(a) * 6, 2.6, Math.sin(a) * 6).lookingAt([0, 0.6, 0], [0, 1, 0])} projection={{ Perspective: { fov: (45 * Math.PI) / 180 } }} />
      <DirectionalLight name="sun" transform={Transform.fromXyz(4, 8, 3).lookingAt([0, 0, 0], [0, 1, 0])} illuminance={6000} shadow_maps_enabled />
      <AmbientLight brightness={120} />
      <Mesh3d name="floor" transform={Transform.fromXyz(0, -0.05, 0)}><Cuboid half_size={[8, 0.05, 8]} /><StandardMaterial base_color="#a8a196" perceptual_roughness={0.9} /></Mesh3d>
      <Mesh3d name="hero" transform={Transform.fromXyz(0, size, 0)}><Cuboid half_size={[size, size, size]} /><StandardMaterial base_color="#d04a3a" perceptual_roughness={0.45} /></Mesh3d>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="held" component={Held} defaultProps={{ size: 0.6 }} controls={{ size: { kind: "number", min: 0.2, max: 1.5, step: 0.05, cost: "reopen" } }} width={1280} height={720} fps={30} durationInFrames={150} />);
