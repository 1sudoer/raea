// The parity cases for render targets an app draws into itself, drawn on raea's GPU (docs/r3f.feature R3F23,
// src/input/passes.ts): each drawn by raea and by three.js in Chrome.
import { useFBO } from "@react-three/drei";
import { useFrame, useThree } from "@react-three/fiber";
import { useMemo } from "react";
import * as THREE from "three";
import { GPUComputationRenderer } from "three/examples/jsm/misc/GPUComputationRenderer.js";
import type { InputCase } from "./fixtures";

/** the 11-15 audit's fbo probe: a lit, turning knot drawn into a useFBO from its own scene and camera, every frame,
 *  shown on a plane in the screen's scene (0.1.4 drew the plane white: the target was never filled) */
function FboPlane() {
  const fbo = useFBO(256, 256);
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
  useFrame(({ gl, clock }) => {
    inner.knot.rotation.set(0.4, clock.elapsedTime * 1.5, 0);
    gl.setRenderTarget(fbo);
    gl.render(inner.s, inner.cam);
    gl.setRenderTarget(null);
  });
  return (
    <>
      <color attach="background" args={["#222222"]} />
      <mesh rotation={[0, -0.3, 0]}>
        <planeGeometry args={[3, 3]} />
        <meshBasicMaterial map={fbo.texture} />
      </mesh>
    </>
  );
}

/** a wave on water, three's GPUComputationRenderer stepping it: each frame its shader reads the last two heights
 *  (x now, y before) from one float target and writes the next into the other, and the plane shows the one written */
const WAVE = `void main() {
  vec2 uv = gl_FragCoord.xy / resolution.xy;
  vec2 c = 1.0 / resolution.xy;
  vec4 h = texture2D(heightmap, uv);
  float n = texture2D(heightmap, uv + vec2(0.0, c.y)).x + texture2D(heightmap, uv - vec2(0.0, c.y)).x
    + texture2D(heightmap, uv + vec2(c.x, 0.0)).x + texture2D(heightmap, uv - vec2(c.x, 0.0)).x;
  gl_FragColor = vec4((n * 0.5 - h.y) * 0.99, h.x, 0.0, 1.0);
}`;

const SHOW = {
  vertexShader: "varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }",
  fragmentShader: `uniform sampler2D heightmap; varying vec2 vUv;
void main() {
  float h = texture2D(heightmap, vUv).x;
  vec3 deep = vec3(0.04, 0.12, 0.3), crest = vec3(0.85, 0.95, 1.0), trough = vec3(0.0, 0.02, 0.08);
  gl_FragColor = vec4(h > 0.0 ? mix(deep, crest, clamp(h * 3.0, 0.0, 1.0)) : mix(deep, trough, clamp(-h * 3.0, 0.0, 1.0)), 1.0);
}`,
};

function PingPong() {
  const gl = useThree((s) => s.gl);
  const sim = useMemo(() => {
    const gpu = new GPUComputationRenderer(128, 128, gl);
    const start = gpu.createTexture();
    const d = start.image.data as Float32Array;
    // two drops, raised discs off the centre
    for (let y = 0; y < 128; y++) {
      for (let x = 0; x < 128; x++) {
        const h = Math.max(0, 1 - Math.hypot(x - 44, y - 64) / 9) + 0.8 * Math.max(0, 1 - Math.hypot(x - 88, y - 40) / 7);
        d.set([h, h, 0, 1], (y * 128 + x) * 4);
      }
    }
    const height = gpu.addVariable("heightmap", WAVE, start);
    gpu.setVariableDependencies(height, [height]);
    const err = gpu.init();
    if (err) throw new Error(err);
    return { gpu, height };
  }, [gl]);
  const show = useMemo(() => new THREE.ShaderMaterial({ uniforms: { heightmap: { value: null } }, ...SHOW }), []);
  useFrame(() => {
    sim.gpu.compute();
    show.uniforms.heightmap.value = sim.gpu.getCurrentRenderTarget(sim.height).texture;
  });
  return (
    <>
      <color attach="background" args={["#000000"]} />
      <mesh material={show} rotation={[-0.5, 0, 0]}>
        <planeGeometry args={[3, 3]} />
      </mesh>
    </>
  );
}

export const TARGET_INPUTS: Record<string, InputCase> = {
  fbo: {
    canvas: { camera: { position: [0, 0, 4], fov: 45 } },
    frame: 12,
    scene: () => <FboPlane />,
  },
  pingpong: {
    canvas: { camera: { position: [0, 0, 4], fov: 45 } },
    frame: 12,
    scene: () => <PingPong />,
  },
};
