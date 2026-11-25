// The parity cases for materials patched by onBeforeCompile (docs/r3f.feature R3F24, src/input/patched.ts): three's own
// program with the patch applied, its surface lit by raea's engine; each drawn by raea and by three.js in Chrome.
import { MeshDistortMaterial, MeshReflectorMaterial, MeshWobbleMaterial } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";
import type { InputCase } from "./fixtures";

/** an app's own patch: a plane's vertices raised in waves by a uniform it sets on the patched program each frame (the
 *  shader kept from onBeforeCompile, as the three.js forum's examples keep it) */
function Waves() {
  const m = useMemo(() => {
    const mat = new THREE.MeshStandardMaterial({ color: "#3d8bfd", roughness: 0.45, side: THREE.DoubleSide });
    mat.onBeforeCompile = (shader) => {
      shader.uniforms.uTime = { value: 0 };
      shader.vertexShader = `uniform float uTime;\n${shader.vertexShader}`.replace(
        "#include <begin_vertex>",
        "vec3 transformed = position;\ntransformed.z += 0.18 * sin( position.x * 3.0 + uTime * 2.0 ) * cos( position.y * 2.0 + uTime );",
      );
      mat.userData.shader = shader;
    };
    return mat;
  }, []);
  useFrame(({ clock }) => {
    const s = m.userData.shader as THREE.WebGLProgramParametersWithUniforms | undefined;
    if (s) s.uniforms.uTime.value = clock.elapsedTime;
  });
  return (
    <mesh material={m} position={[0, -1.1, -0.6]} rotation={[-1.2, 0, 0]}>
      <planeGeometry args={[6, 3, 96, 48]} />
    </mesh>
  );
}

const lights = (
  <>
    <ambientLight intensity={0.4} />
    <directionalLight position={[3, 4, 2]} intensity={2.2} />
  </>
);

export const PATCHED_INPUTS: Record<string, InputCase> = {
  // drei's MeshWobbleMaterial and MeshDistortMaterial (vertex patches) and an app's own waves
  patched: {
    canvas: { camera: { position: [0, 0.6, 4.2], fov: 45 } },
    frame: 12,
    scene: () => (
      <>
        <color attach="background" args={["#14161c"]} />
        {lights}
        <mesh position={[-1.25, 0.35, 0]}>
          <torusKnotGeometry args={[0.55, 0.18, 160, 20]} />
          <MeshWobbleMaterial color="#f2a541" factor={0.8} speed={1.5} roughness={0.35} />
        </mesh>
        <mesh position={[1.25, 0.35, 0]}>
          <sphereGeometry args={[0.7, 96, 64]} />
          <MeshDistortMaterial color="#e05a8b" distort={0.45} speed={2} roughness={0.25} metalness={0.1} />
        </mesh>
        <Waves />
      </>
    ),
  },
  // drei's MeshReflectorMaterial: the scene drawn from the mirrored camera into a target (R3F23) and read through its
  // patch's projective lookup
  reflector: {
    canvas: { camera: { position: [0, 1.4, 4.4], fov: 45 } },
    frame: 12,
    scene: () => (
      <>
        <color attach="background" args={["#101218"]} />
        {lights}
        <mesh position={[-0.9, 0.6, 0]} rotation={[0.4, 0.6, 0]}>
          <boxGeometry args={[0.8, 0.8, 0.8]} />
          <meshStandardMaterial color="#f2a541" roughness={0.4} />
        </mesh>
        <mesh position={[0.9, 0.7, -0.3]}>
          <sphereGeometry args={[0.6, 48, 32]} />
          <meshStandardMaterial color="#3d8bfd" roughness={0.3} />
        </mesh>
        <mesh rotation={[-Math.PI / 2, 0, 0]}>
          <planeGeometry args={[8, 8]} />
          <MeshReflectorMaterial resolution={256} mirror={0.8} mixStrength={2} roughness={0.6} metalness={0.2} color="#3a3d46" />
        </mesh>
      </>
    ),
  },
};
