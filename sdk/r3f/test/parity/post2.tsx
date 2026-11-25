// The parity cases for postprocessing's effects raea draws as <FramePass>es (docs/r3f.feature R3F25, src/input/effects.ts):
// each a lit scene under a composer of them, drawn by raea and by three.js in Chrome.
import * as PP from "@react-three/postprocessing";
import { BlendFunction } from "postprocessing";
import { useMemo } from "react";
import * as THREE from "three";
import type { InputCase } from "./fixtures";

const scene = (
  <>
    <color attach="background" args={["#1d2330"]} />
    <ambientLight intensity={0.5} />
    <directionalLight position={[2, 5, 1]} intensity={2.5} />
    <mesh position={[0, 0.6, 0]}>
      <sphereGeometry args={[0.6, 32, 16]} />
      <meshStandardMaterial color="#e05a2b" roughness={0.4} />
    </mesh>
    <mesh position={[1.4, 0.4, -0.6]}>
      <boxGeometry args={[0.8, 0.8, 0.8]} />
      <meshStandardMaterial color="#3a8fd8" roughness={0.5} />
    </mesh>
    <mesh rotation={[-Math.PI / 2, 0, 0]}>
      <planeGeometry args={[8, 8]} />
      <meshStandardMaterial color="#cfd6e0" roughness={0.9} />
    </mesh>
  </>
);

// a ShaderMaterial's colour with no tone mapping of its own (output "display"), shaded by a fixed light, beside a lit one
const BALL_FRAGMENT = /* glsl */ `
varying vec3 vNormal;
void main() {
  float light = 0.3 + 0.7 * max(dot(normalize(vNormal), normalize(vec3(0.4, 0.8, 0.6))), 0.0);
  gl_FragColor = vec4(vec3(0.85, 0.42, 0.2) * light, 1.0);
}`;
const BALL_VERTEX = /* glsl */ `
varying vec3 vNormal;
void main() {
  vNormal = normalize(normalMatrix * normal);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

function ShadedBall() {
  const m = useMemo(() => new THREE.ShaderMaterial({ vertexShader: BALL_VERTEX, fragmentShader: BALL_FRAGMENT }), []);
  return (
    <mesh position={[-0.7, 0.7, 0]} material={m}>
      <sphereGeometry args={[0.6, 48, 24]} />
    </mesh>
  );
}

export const POST2_INPUTS: Record<string, InputCase> = {
  // sepia, scanlines overlaid, a grid at a third, the colours cut to 5 bits, then tone-mapped
  post2: {
    canvas: { camera: { position: [3, 2.5, 4], fov: 45 } },
    frame: 6,
    scene: () => (
      <>
        {scene}
        <PP.EffectComposer>
          <PP.Sepia intensity={0.8} />
          <PP.Scanline density={1.2} opacity={0.25} />
          <PP.Grid scale={1.5} lineWidth={0.05} blendFunction={BlendFunction.MULTIPLY} />
          <PP.ColorDepth bits={5} />
          <PP.ToneMapping mode={6} />
        </PP.EffectComposer>
      </>
    ),
  },
  // after the tone mapping (a dot screen's rgb*10-5 goes far below 0, which three's ACES curve turns bright and Bevy's
  // clamps): a dot screen at half, the colours averaged, screen-blended noise
  post3: {
    canvas: { camera: { position: [3, 2.5, 4], fov: 45 } },
    frame: 6,
    scene: () => (
      <>
        {scene}
        <PP.EffectComposer>
          <PP.ToneMapping mode={6} />
          <PP.DotScreen angle={Math.PI / 4} scale={0.9} opacity={0.5} />
          <PP.ColorAverage opacity={0.6} />
          <PP.Noise opacity={0.08} />
        </PP.EffectComposer>
      </>
    ),
  },
  // a ShaderMaterial under a composer whose bloom makes raea's camera HDR (its glow faint: only the knot passes the
  // threshold), with the renderer's ACES: three draws the ShaderMaterial's colour into the composer's buffer
  shaderbloom: {
    canvas: { camera: { position: [0, 1.6, 4.2], fov: 45 } },
    frame: 6,
    scene: () => (
      <>
        <color attach="background" args={["#1d2330"]} />
        <ambientLight intensity={0.5} />
        <directionalLight position={[2, 5, 1]} intensity={2.5} />
        <ShadedBall />
        <mesh position={[0.8, 0.7, -0.3]}>
          <torusKnotGeometry args={[0.35, 0.12, 128, 16]} />
          <meshStandardMaterial color="#202a3a" emissive="#58c8ff" emissiveIntensity={4} />
        </mesh>
        <mesh rotation={[-Math.PI / 2, 0, 0]}>
          <planeGeometry args={[8, 8]} />
          <meshStandardMaterial color="#cfd6e0" roughness={0.9} />
        </mesh>
        <PP.EffectComposer>
          <PP.Bloom intensity={0.6} luminanceThreshold={0.9} mipmapBlur />
        </PP.EffectComposer>
      </>
    ),
  },
};
