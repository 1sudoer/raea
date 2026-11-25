// The renderer React Three Fiber configures and a plain three.js program makes (session.ts, program.ts), which never
// draws: raea does. What's set on it (shadows, tone mapping, clear colour) is read back as the scene is converted.
// Its own module, so a composition's `import "three"` (vanilla.ts) needs nothing of React Three Fiber's.
import * as THREE from "three";
import { renderInto } from "./targets";
import { Dispatches } from "./tsl-compute";
import { recordPipeline } from "./tsl-post";

const warned = new Set<string>();
export function warnOnce(k: string, why: string) {
  if (warned.has(k)) return;
  warned.add(k);
  console.warn(`raea/r3f: ${k} ${why}`);
}

/** the renderer R3F configures and never draws with: what it sets on it (shadows, tone mapping) is read back */
export function standInRenderer(): THREE.WebGLRenderer {
  const noop = () => {};
  const size = { width: 1, height: 1 };
  // a WebGL context that answers what a composer asks as it is made (@react-three/postprocessing, three's own), and
  // takes every other call as it renders (its depth blit: bindFramebuffer, blitFramebuffer) doing nothing, its
  // constants (READ_FRAMEBUFFER, DEPTH_BUFFER_BIT) 0: raea draws the effect (post.ts)
  const gl = {
    getContextAttributes: () => ({ alpha: true, depth: true, stencil: false, antialias: false, premultipliedAlpha: true }),
    getExtension: () => null, getParameter: () => 0, getSupportedExtensions: () => [], isContextLost: () => false,
  } as Record<string | symbol, unknown>;
  const context = new Proxy(gl, { get: (t, k) => (k in t ? t[k] : typeof k !== "string" || k === "then" ? undefined : /^[A-Z0-9_]+$/.test(k) ? 0 : /^[a-z]/.test(k) ? noop : undefined) });
  // a pass an app draws into a render target itself (drei's <ContactShadows>) is done on the CPU (targets.ts)
  let target: THREE.RenderTarget | null = null;
  const clear: [number, number, number, number] = [0, 0, 0, 0];
  // WebGLRenderer's GL state, which a pass an app draws itself sets as it goes (drei's <MeshReflectorMaterial>:
  // state.buffers.depth.setMask): every call does nothing, every flag reads as off (docs/r3f.feature R3F16)
  const inert = (): unknown => new Proxy(noop, { get: (_, k) => (k === "then" || typeof k !== "string" ? undefined : inert()), apply: () => undefined });
  const state = { buffers: { color: inert(), depth: inert(), stencil: inert() }, enable: noop, disable: noop, reset: noop, bindTexture: noop, unbindTexture: noop, activeTexture: noop, setBlending: noop, setFlipSided: noop, setCullFace: noop, viewport: noop, scissor: noop, setScissorTest: noop, useProgram: () => false, bindFramebuffer: () => false, drawBuffers: noop, setPolygonOffset: noop, setLineWidth: noop, setMaterial: noop };
  // what an app dispatches with gl.compute (TSL's compute nodes), in order: each frame's runs as a <Compute> kernel
  // each, its storage shared with the materials reading it (tsl-compute.ts, docs/r3f.feature R3F22)
  const dispatched = new Dispatches(null);
  const own = {
    compute: (nodes: unknown, size?: number | number[] | null) => dispatched.compute(nodes, size),
    computeAsync: async (nodes: unknown, size?: number | number[] | null) => dispatched.compute(nodes, size),
    __raea: { computes: dispatched },
    // three's WebGL clip space (a CubeCamera asks, drei's <Environment> with children): raea draws in Bevy's anyway
    coordinateSystem: THREE.WebGLCoordinateSystem,
    state,
    extensions: { has: () => false, get: () => null, init: noop },
    render: (scene: THREE.Object3D, camera: THREE.Camera) => {
      if (target) renderInto(target, scene, camera, clear);
    },
    setRenderTarget: (t: THREE.RenderTarget | null) => void (target = t),
    getRenderTarget: () => target,
    setClearColor: (c: THREE.ColorRepresentation, a?: number) => {
      const col = new THREE.Color(c);
      clear[0] = col.r, clear[1] = col.g, clear[2] = col.b;
      if (a !== undefined) clear[3] = a;
    },
    setClearAlpha: (a: number) => void (clear[3] = a),
    getClearAlpha: () => clear[3], setSize: (w: number, h: number) => void Object.assign(size, { width: w, height: h }), setPixelRatio: noop, getPixelRatio: () => 1,
    getSize: (v: THREE.Vector2) => v.set(size.width, size.height), getDrawingBufferSize: (v: THREE.Vector2) => v.set(size.width, size.height),
    getContext: () => context, getClearColor: (c: THREE.Color) => c.setRGB(clear[0], clear[1], clear[2]),
    dispose: noop, compile: noop, autoClear: true, autoClearColor: true, autoClearDepth: true, autoClearStencil: true,
    domElement: { style: {}, addEventListener: noop, removeEventListener: noop },
    xr: { enabled: false, isPresenting: false, addEventListener: noop, removeEventListener: noop, setAnimationLoop: noop },
    shadowMap: { enabled: false, type: THREE.PCFShadowMap },
    outputColorSpace: THREE.SRGBColorSpace, toneMapping: THREE.NoToneMapping, toneMappingExposure: 1,
    capabilities: { isWebGL2: true, maxTextureSize: 16384, maxSamples: 4, logarithmicDepthBuffer: false }, info: { autoReset: true, reset: noop, render: { calls: 0, triangles: 0, points: 0, lines: 0, frame: 0 }, memory: { geometries: 0, textures: 0 }, programs: [] }, properties: { get: () => ({}) },
  } as Record<string | symbol, unknown>;
  // a pass the CPU doesn't do is drawn by raea's engine as a camera into the target's image, its scene's parts taken as
  // they are when it's drawn (targets.ts recordPass, passes.ts: R3F23)
  own.render = (scene: THREE.Object3D, camera: THREE.Camera) => void (target && renderInto(target, scene, camera, clear, own as unknown as THREE.WebGLRenderer));
  // three/webgpu's PostProcessing draws its quad to the screen: its chain is recorded for the scene it starts from,
  // which R3F's frame is (tsl-post.ts, docs/r3f.feature R3F25)
  const into = own.render as (scene: THREE.Object3D, camera: THREE.Camera) => void;
  own.render = (scene: THREE.Object3D, camera: THREE.Camera) => (target ? into(scene, camera) : void recordPipeline(scene, own as unknown as THREE.WebGLRenderer));
  // any other of WebGLRenderer's methods (setRenderTarget, clear, readRenderTargetPixels ...) does nothing: raea draws
  // (verbs only: a catch-all would answer `then`, and the renderer would read as a promise)
  const verb = /^(set|get|clear|read|copy|init|reset|render|dispose|compile|update|bind)[A-Z]?/;
  return new Proxy(own, { get: (t, k) => (k in t ? t[k] : typeof k === "string" && verb.test(k) ? noop : undefined) }) as unknown as THREE.WebGLRenderer;
}
