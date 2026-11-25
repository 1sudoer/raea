// Render targets drawn on raea's GPU (docs/r3f.feature R3F23): a pass an app draws itself (gl.setRenderTarget(t);
// gl.render(scene, camera)) is its scene's parts, as they were when it was drawn, on a render layer of its own, and a
// camera part drawing that layer into the target's image; a material reading the target's texture names that image.
import { describe, expect, test } from "bun:test";
import { createPortal, useFrame } from "@react-three/fiber";
import { useMemo, type ReactNode } from "react";
import * as THREE from "three";
import { GPUComputationRenderer } from "three/examples/jsm/misc/GPUComputationRenderer.js";
import { MeshBasicNodeMaterial } from "three/webgpu";
import * as TSLTyped from "three/tsl";
// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
const { texture, uv } = TSLTyped as unknown as Record<string, any>;
import { convert } from "../src/input/scene";
import { end, stepTo } from "../src/input/session";
import type { Part } from "../src/parts";

const flat = (parts: Part[]): Part[] => parts.flatMap((p) => [p, ...flat(p.children ?? [])]);
let n = 0;
function partsOf(scene: () => ReactNode, frame = 0): Part[] {
  const key = `targets${n++}`;
  const s = stepTo(key, frame, 30, 400, 400, {}, scene);
  const st = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
  const out = flat(convert(st.scene, st.camera, st.gl, 400).parts);
  end(key);
  return out;
}

/** the audit's fbo probe: a knot drawn into a target (its own scene, through a portal) shown on a plane */
function Fbo() {
  const target = useMemo(() => new THREE.WebGLRenderTarget(128, 64), []);
  const scene = useMemo(() => Object.assign(new THREE.Scene(), { background: new THREE.Color("#ff8800") }), []);
  const cam = useMemo(() => {
    const c = new THREE.PerspectiveCamera(50, 2, 0.1, 100);
    c.position.set(0, 0, 4);
    return c;
  }, []);
  useFrame(({ gl }) => {
    gl.setRenderTarget(target);
    gl.render(scene, cam);
    gl.setRenderTarget(null);
  });
  return (
    <>
      {createPortal(<mesh name="knot"><torusKnotGeometry args={[0.6, 0.2, 32, 8]} /><meshBasicMaterial color="#2266ff" /></mesh>, scene)}
      <mesh name="screen"><planeGeometry args={[2, 1]} /><meshBasicMaterial map={target.texture} /></mesh>
    </>
  );
}

describe("a render into a target, drawn by raea's engine", () => {
  test("is a camera into the target's image, before the screen's, drawing its scene's parts on its own layer", () => {
    const out = partsOf(() => <Fbo />);
    const cam = out.find((p) => typeof p.props.target === "string")!;
    expect(cam.type).toBe("PerspectiveCamera");
    expect(cam.props.target).toMatch(/^render:[^:]+:128x64$/);
    expect(cam.props.order as number).toBeLessThan(0);
    // cleared to the pass scene's background, opaque
    expect(cam.props.bg).toBe("#ff8800ff");
    const layer = (cam.props.layers as number[])[0];
    expect(layer).toBeGreaterThan(31);
    const knot = out.find((p) => p.type === "Mesh" && String(p.props.name).includes("knot"))!;
    expect(knot.props.layers).toEqual([layer]);
    expect(String(knot.props.name).startsWith("pass/")).toBe(true);
    // the screen's plane is on no pass layer and reads the image the camera draws, by the same name
    const screen = out.find((p) => p.type === "Mesh" && String(p.props.name).includes("screen"))!;
    expect(screen.props.layers).toBeUndefined();
    const map = flat(screen.children ?? []).map((c) => c.props.map).find((m) => typeof m === "string");
    expect(map).toBe(cam.props.target as string);
  });

  test("is the frame's own: a render of frame 3 has the one pass frame 3 drew, not one for each frame stepped", () => {
    const out = partsOf(() => <Fbo />, 3);
    expect(out.filter((p) => typeof p.props.target === "string").length).toBe(1);
    expect(out.filter((p) => p.type === "Mesh" && String(p.props.name).includes("knot")).length).toBe(1);
  });

  test("takes its scene as it was when drawn: a quad drawn twice with another uniform is two passes, each its own", () => {
    function Twice() {
      const a = useMemo(() => new THREE.WebGLRenderTarget(32, 32, { type: THREE.HalfFloatType }), []);
      const b = useMemo(() => new THREE.WebGLRenderTarget(32, 32, { type: THREE.HalfFloatType }), []);
      const mat = useMemo(() => new THREE.ShaderMaterial({ uniforms: { k: { value: 0 } }, vertexShader: "void main() { gl_Position = vec4(position, 1.0); }", fragmentShader: "uniform float k; void main() { gl_FragColor = vec4(k); }" }), []);
      const quad = useMemo(() => new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat), [mat]);
      const cam = useMemo(() => new THREE.Camera(), []);
      useFrame(({ gl }) => {
        mat.uniforms.k.value = 0.25;
        gl.setRenderTarget(a);
        gl.render(quad, cam);
        mat.uniforms.k.value = 0.75;
        gl.setRenderTarget(b);
        gl.render(quad, cam);
        gl.setRenderTarget(null);
      });
      return null;
    }
    const out = partsOf(() => <Twice />);
    const cams = out.filter((p) => typeof p.props.target === "string");
    expect(cams.length).toBe(2);
    // a float target's image is a float one, and a bare THREE.Camera's pass is drawn by an orthographic one over the quad
    for (const c of cams) {
      expect(c.props.target).toMatch(/:32x32:float$/);
      expect(c.type).toBe("OrthographicCamera");
    }
    expect(cams[0].props.order as number).toBeLessThan(cams[1].props.order as number);
    const k = (layer: number) => {
      const quad = out.find((p) => p.type === "Mesh" && (p.props.layers as number[] | undefined)?.[0] === layer)!;
      return flat(quad.children ?? []).map((c) => (c.props.uniforms as { k?: number } | undefined)?.k).find((v) => v !== undefined);
    };
    expect(k((cams[0].props.layers as number[])[0])).toBe(0.25);
    expect(k((cams[1].props.layers as number[])[0])).toBe(0.75);
    // a three shader drawn into a target writes its colour as it is, never through the screen's tone mapping
    const outputs = out.filter((p) => ((p.props.layers as number[] | undefined)?.[0] ?? 0) >= 64).flatMap((p) => flat(p.children ?? [])).filter((c) => c.type === "ThreeShaderMaterial").map((c) => c.props.output);
    expect(outputs).toEqual(["target", "target"]);
  });

  test("is the same thing's frame to frame: one quad, its material swapped, keeps a pass for each (GPUComputationRenderer)", () => {
    // its init draws the quad with its pass-through shader into the target once, then each frame with the variable's:
    // a later frame's compute pass is frame 0's compute pass again (its rows, its shader), not the init's
    function Swapped() {
      const t = useMemo(() => new THREE.WebGLRenderTarget(16, 16, { type: THREE.FloatType }), []);
      const shader = (k: number) => new THREE.ShaderMaterial({ uniforms: { k: { value: k } }, vertexShader: "void main() { gl_Position = vec4(position, 1.0); }", fragmentShader: "uniform float k; void main() { gl_FragColor = vec4(k); }" });
      const [through, compute] = useMemo(() => [shader(0.1), shader(0.9)], []);
      const quad = useMemo(() => new THREE.Mesh(new THREE.PlaneGeometry(2, 2), through), [through]);
      const cam = useMemo(() => new THREE.Camera(), []);
      const init = useMemo(() => ({ done: false }), []);
      useFrame(({ gl }) => {
        gl.setRenderTarget(t);
        if (!init.done) {
          quad.material = through;
          gl.render(quad, cam);
          init.done = true;
        }
        quad.material = compute;
        gl.render(quad, cam);
        gl.setRenderTarget(null);
      });
      return null;
    }
    const key = `targets${n++}`;
    const camsAt = (frame: number) => {
      const st = stepTo(key, frame, 30, 400, 400, {}, () => <Swapped />).state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
      return flat(convert(st.scene, st.camera, st.gl, 400).parts).filter((p) => typeof p.props.target === "string");
    };
    const first = camsAt(0);
    const later = camsAt(2);
    end(key);
    expect(first.length).toBe(2);
    expect(later.length).toBe(1);
    expect(later[0].props.name).toBe(first[1].props.name);
    expect(later[0].props.layers).toEqual(first[1].props.layers);
    expect(later[0].props.name).not.toBe(first[0].props.name);
  });

  test("is read by a TSL node material as its camera draws it: the image itself, v turned over, as sRGB", () => {
    function Read() {
      const t = useMemo(() => new THREE.WebGLRenderTarget(32, 32), []);
      const scene = useMemo(() => Object.assign(new THREE.Scene(), { background: new THREE.Color("#336699") }), []);
      const cam = useMemo(() => new THREE.PerspectiveCamera(), []);
      const mat = useMemo(() => Object.assign(new MeshBasicNodeMaterial(), { colorNode: texture(t.texture, uv()) }), [t]);
      useFrame(({ gl }) => {
        gl.setRenderTarget(t);
        gl.render(scene, cam);
        gl.setRenderTarget(null);
      });
      return <mesh name="screen" material={mat}><planeGeometry args={[2, 2]} /></mesh>;
    }
    const out = partsOf(() => <Read />);
    const target = out.find((p) => typeof p.props.target === "string")!.props.target as string;
    const shader = flat(out.find((p) => p.type === "Mesh" && String(p.props.name).includes("screen"))!.children ?? []).find((c) => c.props.wgsl)!;
    const textures = Object.values(shader.props.textures as Record<string, { src: string; colorSpace: string }>);
    expect(textures).toEqual([{ src: target, colorSpace: "srgb" }]);
    expect((shader.props.wgsl as { fragment: string }).fragment).toMatch(/textureSample\( tex_\w+, samp_\w+, tsl_flip_v\(/);
  });

  test("is read by a ShaderMaterial as its camera draws it: the image itself, v turned over, never a raw copy", () => {
    // a raw copy ({colorSpace: "linear"}: three-shader's linear::raw) is taken once, before any pass has drawn
    function Read() {
      const t = useMemo(() => new THREE.WebGLRenderTarget(32, 32), []);
      const scene = useMemo(() => Object.assign(new THREE.Scene(), { background: new THREE.Color("#336699") }), []);
      const cam = useMemo(() => new THREE.PerspectiveCamera(), []);
      const mat = useMemo(() => new THREE.ShaderMaterial({ uniforms: { tex: { value: t.texture } }, vertexShader: "varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }", fragmentShader: "uniform sampler2D tex; varying vec2 vUv; void main() { gl_FragColor = texture2D(tex, vUv); }" }), [t]);
      useFrame(({ gl }) => {
        gl.setRenderTarget(t);
        gl.render(scene, cam);
        gl.setRenderTarget(null);
      });
      return <mesh name="screen" material={mat}><planeGeometry args={[2, 2]} /></mesh>;
    }
    const out = partsOf(() => <Read />);
    const target = out.find((p) => typeof p.props.target === "string")!.props.target as string;
    const shader = flat(out.find((p) => p.type === "Mesh" && String(p.props.name).includes("screen"))!.children ?? []).find((c) => c.props.textures)!;
    expect((shader.props.textures as Record<string, unknown>).tex).toBe(target);
    expect(shader.props.flipY).toContain("tex");
  });

  test("ping-pongs as three's GPUComputationRenderer does: its two targets' passes on their frames, the display reading the one drawn", () => {
    function Wave() {
      const sim = useMemo(() => ({ gpu: undefined as GPUComputationRenderer | undefined, v: undefined as unknown }), []);
      const show = useMemo(() => new THREE.ShaderMaterial({ uniforms: { heightmap: { value: null } }, vertexShader: "varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }", fragmentShader: "uniform sampler2D heightmap; varying vec2 vUv; void main() { gl_FragColor = texture2D(heightmap, vUv); }" }), []);
      useFrame(({ gl }) => {
        if (!sim.gpu) {
          sim.gpu = new GPUComputationRenderer(16, 16, gl);
          sim.v = sim.gpu.addVariable("heightmap", "void main() { gl_FragColor = texture2D(heightmap, gl_FragCoord.xy / resolution.xy) * 0.9; }", sim.gpu.createTexture());
          sim.gpu.setVariableDependencies(sim.v as never, [sim.v as never]);
          expect(sim.gpu.init()).toBeNull();
        }
        sim.gpu.compute();
        show.uniforms.heightmap.value = sim.gpu.getCurrentRenderTarget(sim.v as never).texture;
      });
      return <mesh name="screen" material={show}><planeGeometry args={[2, 2]} /></mesh>;
    }
    const key = `targets${n++}`;
    const at = (frame: number) => {
      const st = stepTo(key, frame, 30, 400, 400, {}, () => <Wave />).state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
      const parts = flat(convert(st.scene, st.camera, st.gl, 400).parts);
      const cams = parts.filter((p) => typeof p.props.target === "string");
      const shader = flat(parts.find((p) => p.type === "Mesh" && String(p.props.name).includes("screen"))!.children ?? []).find((c) => c.props.textures)!;
      return { cams, read: (shader.props.textures as Record<string, string>).heightmap };
    };
    const [f0, f1, f2] = [at(0), at(1), at(2)];
    end(key);
    // frame 0: the start drawn into both targets, then the first step into the second; each later frame one step
    expect(f0.cams.length).toBe(3);
    expect(f1.cams.length).toBe(1);
    expect(f2.cams.length).toBe(1);
    const [a, b] = [f0.cams[0].props.target, f0.cams[1].props.target];
    expect(a).not.toBe(b);
    expect([f0.cams[2].props.target, f1.cams[0].props.target, f2.cams[0].props.target]).toEqual([b, a, b]);
    // the display reads the target each frame's step wrote, and a step's pass keeps its name from frame to frame
    expect([f0.read, f1.read, f2.read]).toEqual([b, a, b] as string[]);
    expect(f2.cams[0].props.name).toBe(f0.cams[2].props.name);
  });
});
