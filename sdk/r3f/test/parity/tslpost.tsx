// The parity cases for three/webgpu's PostProcessing (docs/r3f.feature R3F25, src/input/tsl-post.ts): a lit scene
// drawn through a TSL chain, by three/webgpu in Chrome (its WebGL 2 backend) and by raea, which draws the chain's scene
// pass and maps its nodes to raea's effects. No node reads TSL's `time` (Chrome's would differ).
import { useFrame, useThree } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";
import { RenderPipeline } from "three/webgpu";
import * as TSLTyped from "three/tsl";
import * as BloomM from "three/examples/jsm/tsl/display/BloomNode.js";
import * as AberrationM from "three/examples/jsm/tsl/display/ChromaticAberrationNode.js";
import * as SepiaM from "three/examples/jsm/tsl/display/Sepia.js";
import type { InputCase } from "./fixtures";

// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
type Fn = (...a: unknown[]) => any;
const { pass } = TSLTyped as unknown as Record<string, Fn>;
const [{ bloom }, { chromaticAberration }, { sepia }] = [BloomM, AberrationM, SepiaM] as unknown as Record<string, Fn>[];

/** three's WebGPURenderer: its WebGL 2 backend (headless Chrome may have no WebGPU) */
async function nodeRenderer(props: object) {
  const { WebGPURenderer } = await import("three/webgpu");
  const r = new WebGPURenderer({ ...props, forceWebGL: true, antialias: true });
  await r.init();
  return r;
}

/** the scene drawn through the chain `make` builds on its pass, in place of R3F's own render (useFrame's priority 1) */
function Piped({ make }: { make: (color: any) => unknown }) {
  const { gl, scene, camera } = useThree();
  const pp = useMemo(() => {
    const p = new RenderPipeline(gl as never);
    p.outputNode = make(pass(scene, camera).getTextureNode("output")) as never;
    return p;
  }, [gl, scene, camera, make]);
  useFrame(() => pp.render(), 1);
  return null;
}

const scene = (
  <>
    <color attach="background" args={["#10131a"]} />
    <ambientLight intensity={0.4} />
    <directionalLight position={[2, 5, 1]} intensity={2.5} />
    <mesh position={[-0.9, 0.6, 0]}>
      <sphereGeometry args={[0.6, 32, 16]} />
      <meshStandardMaterial color="#e05a2b" roughness={0.4} />
    </mesh>
    <mesh position={[0.9, 0.6, -0.4]}>
      <torusKnotGeometry args={[0.4, 0.14, 128, 16]} />
      <meshStandardMaterial color="#202a3a" emissive="#58c8ff" emissiveIntensity={4} />
    </mesh>
    <mesh rotation={[-Math.PI / 2, 0, 0]}>
      <planeGeometry args={[8, 8]} />
      <meshStandardMaterial color="#9aa3b0" roughness={0.9} />
    </mesh>
  </>
);

const bloomed = (color: any) => color.add(bloom(color, 0.8, 0.3, 0.9));
const aged = (color: any) => chromaticAberration(sepia(color), 2, new THREE.Vector2(0.5, 0.5), 1.2);

export const TSLPOST_INPUTS: Record<string, InputCase> = {
  // three's own bloom on what's brighter than 0.9 (the glowing knot), added to the scene, then ACES (R3F's)
  tslpost: {
    canvas: { camera: { position: [0, 2, 4.5], fov: 45 }, gl: nodeRenderer },
    frame: 6,
    scene: () => (
      <>
        {scene}
        <Piped make={bloomed} />
      </>
    ),
  },
  // sepia, then its colours split away from the centre, before the tone mapping
  tslpost2: {
    canvas: { camera: { position: [0, 2, 4.5], fov: 45 }, gl: nodeRenderer },
    frame: 6,
    scene: () => (
      <>
        {scene}
        <Piped make={aged} />
      </>
    ),
  },
};
