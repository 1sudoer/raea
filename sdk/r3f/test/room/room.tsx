// Scenes lit by three's RoomEnvironment through PMREMGenerator.fromScene, three's own recipe for soft studio light
// (#165), for check.sh to measure against what three draws (three.ts works three's pixels out on the CPU).
//   repro: #131's repro, an orange box under a directional light and the room (the issue measured its front face)
//   grey: two grey boxes (roughness 0.5 and 1) lit by the room alone at half its intensity, so no channel clips
import { useMemo } from "react";
import { useThree } from "@react-three/fiber";
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { AbsoluteFill, Composition, registerRoot } from "raea";
import { R3F } from "raea/r3f";

function Studio({ intensity = 1 }: { intensity?: number }) {
  const { gl, scene } = useThree();
  useMemo(() => {
    scene.environment = new THREE.PMREMGenerator(gl).fromScene(new RoomEnvironment(), 0.04).texture;
    scene.environmentIntensity = intensity;
  }, [gl, scene, intensity]);
  return null;
}

const Repro = () => (
  <AbsoluteFill style={{ background: "#00f" }}>
    <R3F flat camera={{ position: [0, 0, 5], fov: 32 }}>
      <Studio />
      <directionalLight position={[2, 4, 4]} intensity={1.5} />
      <mesh>
        <boxGeometry args={[2, 1.3, 1.3]} />
        <meshStandardMaterial color="#D97757" roughness={0.5} />
      </mesh>
    </R3F>
  </AbsoluteFill>
);

const Grey = () => (
  <AbsoluteFill style={{ background: "#00f" }}>
    <R3F flat camera={{ position: [0, 0, 6], fov: 32 }}>
      <Studio intensity={0.5} />
      {([[-0.8, 0.5], [0.8, 1]] as const).map(([x, roughness]) => (
        <mesh key={x} position={[x, 0, 0]}>
          <boxGeometry args={[1.2, 1.2, 1.2]} />
          <meshStandardMaterial color="#808080" roughness={roughness} />
        </mesh>
      ))}
    </R3F>
  </AbsoluteFill>
);

registerRoot(() => (
  <>
    <Composition id="repro" component={Repro} width={400} height={400} fps={30} durationInFrames={1} />
    <Composition id="grey" component={Grey} width={400} height={400} fps={30} durationInFrames={1} />
  </>
));
