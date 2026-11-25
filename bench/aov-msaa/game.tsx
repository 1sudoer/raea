// #159: a game's 3D (its rows set on the live engine each frame, so raea-sys-post's msaa_for_3d sees them): a
// multisampled camera, a still bloom and a box that moves, rendered with --aov (check.sh).
import { AbsoluteFill, registerRoot, Composition, useGameState } from "raea";
import { Bloom, Canvas3D, ToneMapping, manifold } from "raea/3d";
import { Camera3d } from "raea/camera";
import { AmbientLight, DirectionalLight } from "raea/light";
import { Mesh3d } from "raea/mesh";
import { StandardMaterial } from "raea/pbr";
import { Transform } from "raea/transform";

const GROUND = manifold.box(20, 1, 20).move(0, -0.5, 0);
const BOX = manifold.box(1.5, 1.5, 1.5);

function Game() {
  const t = useGameState({ t: 0 }, (s) => ({ t: s.t + 1 })).t;
  return (
    <AbsoluteFill style={{ background: "#202830" }}>
      <Canvas3D>
        <Camera3d transform={Transform.fromXyz(6, 3, 8).lookingAt([0, 0.8, 0], [0, 1, 0])} projection={{ Perspective: { fov: 0.8, near: 0.1, far: 100 } }} />
        <DirectionalLight name="sun" transform={Transform.fromXyz(10, 20, 5).lookingAt([0, 0, 0], [0, 1, 0])} illuminance={20000} shadow_maps_enabled />
        <AmbientLight brightness={200} />
        <Mesh3d name="ground" shape={GROUND}><StandardMaterial base_color="#6a7a50" perceptual_roughness={0.9} /></Mesh3d>
        <Mesh3d name="box" shape={BOX} transform={Transform.fromXyz(-1.5 + t * 0.2, 0.75, 0)}><StandardMaterial base_color="#d03020" emissive="#ffe0a0" emissive_intensity={4} /></Mesh3d>
        <ToneMapping mode="agx" />
        <Bloom intensity={0.3} threshold={0.8} />
      </Canvas3D>
    </AbsoluteFill>
  );
}

registerRoot(() => <Composition id="game" component={Game} width={320} height={180} fps={30} durationInFrames={10} />);
