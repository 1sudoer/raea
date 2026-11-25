// A ray-traced piece with one control (docs/realtime.feature RT0b): a closed room lit by Solari from a glowing panel
// and a sun through a window, the camera dollying in over 5 s, the panel's glow a prop. Solari and MetalFX keep light
// history between updates, so a frame jumped to isn't converged at once: the bench counts the updates a seeked frame
// takes to come within SSIM 0.995 of the render's frame.
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
import { Camera3d } from "raea/camera";
import { LinearRgba } from "raea/color";
import { DirectionalLight, SolariLighting } from "raea/light";
import { Cuboid, Mesh3d, Sphere } from "raea/mesh";
import { StandardMaterial } from "raea/pbr";
import { Transform } from "raea/transform";

type V3 = [number, number, number];
// a 6 x 3 x 6 m room of thick slabs, a window in the east wall
const SLABS: { name: string; min: V3; max: V3; c: string }[] = [
  { name: "floor", min: [-3.2, -0.2, -3.2], max: [3.2, 0, 3.2], c: "#8a7b66" },
  { name: "ceiling", min: [-3.2, 3, -3.2], max: [3.2, 3.2, 3.2], c: "#d8d4c8" },
  { name: "wall-w", min: [-3.2, 0, -3.2], max: [-3, 3, 3.2], c: "#c9bfa4" },
  { name: "wall-n", min: [-3.2, 0, -3.2], max: [3.2, 3, -3], c: "#b9c4cf" },
  { name: "wall-s", min: [-3.2, 0, 3], max: [3.2, 3, 3.2], c: "#c9bfa4" },
  { name: "wall-e1", min: [3, 0, -3.2], max: [3.2, 3, -0.8], c: "#c9bfa4" },
  { name: "wall-e2", min: [3, 0, 0.8], max: [3.2, 3, 3.2], c: "#c9bfa4" },
  { name: "wall-e3", min: [3, 0, -0.8], max: [3.2, 1, 0.8], c: "#c9bfa4" },
  { name: "wall-e4", min: [3, 2.2, -0.8], max: [3.2, 3, 0.8], c: "#c9bfa4" },
];
const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
const half = (a: V3, b: V3): V3 => [(b[0] - a[0]) / 2, (b[1] - a[1]) / 2, (b[2] - a[2]) / 2];

function Room({ glow = 600 }: { glow?: number }) {
  const f = useCurrentFrame();
  const z = 2.4 - f * 0.008;
  return (
    <AbsoluteFill style={{ background: "#000000" }}>
      <Camera3d transform={Transform.fromXyz(-1.8, 1.5, z).lookingAt([1.2, 0.9, -1.2], [0, 1, 0])} projection={{ Perspective: { fov: (60 * Math.PI) / 180 } }}>
        <SolariLighting bounces={2} />
      </Camera3d>
      <DirectionalLight name="sun" transform={Transform.fromXyz(10, 6, 1).lookingAt([0, 0, 0], [0, 1, 0])} illuminance={30000} />
      {SLABS.map(({ name, min, max, c }) => (
        <Mesh3d key={name} name={name} transform={Transform.fromXyz(...mid(min, max))}>
          <Cuboid half_size={half(min, max)} />
          <StandardMaterial base_color={c} perceptual_roughness={0.85} />
        </Mesh3d>
      ))}
      <Mesh3d name="panel" castShadow={false} transform={Transform.fromXyz(0, 2.96, -1)}>
        <Cuboid half_size={[0.8, 0.02, 0.4]} />
        <StandardMaterial base_color="#ffffff" emissive={new LinearRgba(glow, glow * 0.95, glow * 0.85, 1)} />
      </Mesh3d>
      <Mesh3d name="table" transform={Transform.fromXyz(0.8, 0.4, -1.2)}><Cuboid half_size={[0.7, 0.4, 0.5]} /><StandardMaterial base_color="#6b4a32" perceptual_roughness={0.6} /></Mesh3d>
      <Mesh3d name="ball" transform={Transform.fromXyz(0.6, 1.1, -1.1)}><Sphere radius={0.3} /><StandardMaterial base_color="#d04a3a" perceptual_roughness={0.4} /></Mesh3d>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="solari" component={Room} defaultProps={{ glow: 600 }} width={1280} height={720} fps={30} durationInFrames={150} />);
