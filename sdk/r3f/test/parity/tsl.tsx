// The TSL parity cases (docs/r3f.feature R3F21, R3F22): node materials three/webgpu draws in Chrome and raea's engine
// draws from three's own WGSL (src/input/tsl.ts, tsl-lit.ts). A graph's motion comes from a uniform R3F's clock sets in
// useFrame, never TSL's `time` (three's renderer counts that from performance.now(), so Chrome's frame would differ).
import { useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";
import { MeshBasicNodeMaterial, MeshStandardNodeMaterial } from "three/webgpu";
import * as TSLTyped from "three/tsl";
import type { InputCase } from "./fixtures";

// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
const { color, mix, mx_noise_float, normalLocal, positionLocal, uniform } = TSLTyped as unknown as Record<string, (...a: unknown[]) => any> & { normalLocal: any; positionLocal: any };
const { Fn, cos, float, fract, hash, instanceIndex, instancedArray, length, sin, step, texture, uv, vec3 } = TSLTyped as unknown as Record<string, any>;

/** three's WebGPURenderer for a TSL scene: its WebGL 2 backend (headless Chrome may have no WebGPU) */
async function nodeRenderer(props: object) {
  const { WebGPURenderer } = await import("three/webgpu");
  const r = new WebGPURenderer({ ...props, forceWebGL: true, antialias: true });
  await r.init();
  return r;
}

// the noise blob people post: an icosahedron its positionNode pushes out along the normal by 3D noise, coloured by the
// same noise, lit by the scene's lights (a MeshStandardNodeMaterial with nodes of its own: R3F21)
const blob = (() => {
  const t = uniform(0);
  const m = new MeshStandardNodeMaterial({ roughness: 0.4, metalness: 0.1 });
  const n = mx_noise_float(positionLocal.mul(1.6).add(t.mul(0.5)));
  m.positionNode = positionLocal.add(normalLocal.mul(n.mul(0.3)));
  m.colorNode = mix(color("#ff7744"), color("#3355ff"), n.mul(0.5).add(0.5));
  return { m, t };
})();

function Blob() {
  useFrame(({ clock }) => void (blob.t.value = clock.elapsedTime));
  return (
    <mesh material={blob.m} rotation={[0.2, 0.3, 0]}>
      <icosahedronGeometry args={[1, 32]} />
    </mesh>
  );
}

// a swirl of 65,536 particles a compute moves on the GPU (R3F22): `init` sets each on a golden-angle disk with its colour,
// one of four arms by its first angle, and `update` turns each about the centre, faster the nearer it is, once a
// frame; an InstancedMesh of small quads draws them where the buffers say (positionNode and colorNode read them
// .toAttribute()), added up. The arms wind into a spiral only if the computes ran, frame by frame.
const PARTICLES = 65536;
const swirl = (() => {
  const pos = instancedArray(PARTICLES, "vec3");
  const col = instancedArray(PARTICLES, "vec3");
  const i = float(instanceIndex);
  const init = Fn(() => {
    const turn = fract(i.mul(0.38196601));
    const a = turn.mul(6.28318531);
    const r = i.div(PARTICLES).sqrt().mul(1.9);
    pos.element(instanceIndex).assign(vec3(cos(a).mul(r), sin(a).mul(r), hash(instanceIndex).sub(0.5).mul(0.4)));
    col.element(instanceIndex).assign(mix(vec3(1, 0.45, 0.15), vec3(0.2, 0.45, 1), step(0.5, fract(turn.mul(4)))));
  })().compute(PARTICLES);
  const update = Fn(() => {
    const p = pos.element(instanceIndex);
    const w = float(0.06).div(length(p.xy).add(0.25));
    const c = cos(w);
    const s = sin(w);
    p.assign(vec3(p.x.mul(c).sub(p.y.mul(s)), p.x.mul(s).add(p.y.mul(c)), p.z));
  })().compute(PARTICLES);
  const m = new MeshBasicNodeMaterial({ transparent: true, depthWrite: false, blending: THREE.AdditiveBlending });
  m.positionNode = positionLocal.add(pos.toAttribute());
  m.colorNode = col.toAttribute().mul(0.3);
  return { init, update, m, geo: new THREE.PlaneGeometry(0.025, 0.025) };
})();

/** the computes a frame at a time by the clock's frame (Chrome's page draws its last frame twice: no second update) */
function Swirl() {
  const done = useRef(-1);
  useFrame(({ clock, gl }) => {
    const g = gl as unknown as { compute(n: unknown): void };
    const f = Math.round(clock.elapsedTime * 24);
    for (; done.current < f; done.current++) {
      if (done.current < 0) g.compute(swirl.init);
      g.compute(swirl.update);
    }
  });
  return <instancedMesh args={[swirl.geo, swirl.m, PARTICLES]} frustumCulled={false} />;
}

/** in-fbo's knot (test/parity/targets.tsx) drawn into a target every frame, shown by a node material's texture node: a
 *  target raea draws on its GPU read the way three reads it (turned over in v, its colour as drawn) */
function FboNode() {
  const fbo = useMemo(() => new THREE.RenderTarget(256, 256), []);
  const inner = useMemo(() => {
    const s = new THREE.Scene();
    s.background = new THREE.Color("#ff3366");
    const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(0.6, 0.2, 128, 16), new THREE.MeshStandardMaterial({ color: "#f2c14e", roughness: 0.35 }));
    const sun = new THREE.DirectionalLight("#ffffff", 2.5);
    sun.position.set(2, 3, 2);
    s.add(knot, sun, new THREE.AmbientLight("#ffffff", 0.4));
    const cam = new THREE.PerspectiveCamera(50, 1, 0.1, 10);
    cam.position.z = 3;
    return { s, knot, cam };
  }, []);
  const m = useMemo(() => Object.assign(new MeshBasicNodeMaterial(), { colorNode: texture(fbo.texture, uv()) }), [fbo]);
  useFrame(({ gl, clock }) => {
    inner.knot.rotation.set(0.4, clock.elapsedTime * 1.5, 0);
    gl.setRenderTarget(fbo as unknown as THREE.WebGLRenderTarget);
    gl.render(inner.s, inner.cam);
    gl.setRenderTarget(null);
  });
  return (
    <mesh rotation={[0, -0.3, 0]} material={m}>
      <planeGeometry args={[3, 3]} />
    </mesh>
  );
}

export const TSL_INPUTS: Record<string, InputCase> = {
  "tsl-lit": {
    canvas: { camera: { position: [0, 0.4, 4], fov: 45 }, gl: nodeRenderer },
    frame: 12,
    scene: () => (
      <>
        <color attach="background" args={["#14161c"]} />
        <ambientLight intensity={0.3} />
        <directionalLight position={[3, 5, 2]} intensity={2.5} />
        <Blob />
      </>
    ),
  },
  "tsl-particles": {
    canvas: { camera: { position: [0, 0, 5], fov: 45 }, gl: nodeRenderer },
    frame: 12,
    scene: () => (
      <>
        <color attach="background" args={["#0b0d12"]} />
        <Swirl />
      </>
    ),
  },
  "tsl-fbo": {
    canvas: { camera: { position: [0, 0, 4], fov: 45 }, gl: nodeRenderer },
    frame: 12,
    scene: () => (
      <>
        <color attach="background" args={["#222222"]} />
        <FboNode />
      </>
    ),
  },
};
