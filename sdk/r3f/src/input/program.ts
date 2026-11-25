// A plain three.js program drawn by raea (docs/r3f.feature R3F19): the function a single-file demo's script is, run by
// <Three program={fn}> (Three.tsx) with three's renderers swapped by raea's bundler for stand-ins (the composition's own
// `import "three"` and `"three/webgpu"` are vanilla.ts and vanilla-webgpu.ts, three with its renderer replaced): they
// draw nothing and record what they're asked to render. Each frame the scene last rendered to the screen is drawn by
// raea's engine as <R3F>'s is (scene.ts). The program's animation loop (setAnimationLoop, requestAnimationFrame) runs
// once a frame at that frame's time, i * 1000 / fps ms, with performance.now() reading the same time and Math.random
// seeded, so frame f is the same played to or rendered alone (raea's first rule); going backwards runs the program
// again from the start.
import * as THREE from "three";
import { standInRenderer, warnOnce } from "./renderer";
import { nextRandom, seedOf } from "./seed";
import { recordPipeline } from "./tsl-post";

type Rec = Record<string | symbol, any>;
type Loop = (time: number) => void;

/** a plain three.js page's script as a function, given the canvas to draw on (or it makes its own) */
export type Program = (canvas: HTMLCanvasElement) => unknown;

/** what a renderer was asked to render: the scene, its camera, and the renderer (its tone mapping, its shadows) */
export interface Drawn {
  scene: THREE.Object3D;
  camera: THREE.Camera;
  gl: THREE.WebGLRenderer;
}

export interface Run {
  /** the program's text: a program written inline is the same one each render */
  source: string;
  /** the last frame stepped (-1: none yet) */
  frame: number;
  /** its Math.random's state */
  rand: number;
  /** the time the step being run reads, ms */
  now: number;
  /** the renderers it made: each one's passes into targets are emptied as a frame steps (R3F23) */
  renderers: Set<object>;
  /** its renderers' animation loops */
  loops: Map<object, Loop>;
  /** the frame callbacks it asked for, run at the next step */
  rafs: Map<number, FrameRequestCallback>;
  raf: number;
  /** the last render to the screen, and the last render of a Scene (into anything) */
  drawn: Drawn | null;
  scene: Drawn | null;
  canvas: HTMLCanvasElement;
  /** what the program returned to undo itself, run when it ends */
  stop?: () => void;
  /** an async program's promise, until it settles; then it steps from frame 0 */
  waiting?: Promise<unknown>;
  failed?: { error: unknown };
  /** how many times its globals are in place (a step, the program running, an async program's first moments) */
  depth: number;
}

// ---- the globals a program reads, its own while it runs ----

const live: Run[] = [];
let saved: { raf: unknown; caf: unknown; random: () => number; now: unknown; own: boolean } | null = null;
const top = () => live[live.length - 1];

function enter(run: Run) {
  if (run.depth++ > 0) return;
  live.push(run);
  if (live.length > 1) return;
  const g = globalThis as Rec;
  const perf = g.performance as Rec | undefined;
  saved = { raf: g.requestAnimationFrame, caf: g.cancelAnimationFrame, random: Math.random, now: perf?.now, own: !!perf && Object.prototype.hasOwnProperty.call(perf, "now") };
  g.requestAnimationFrame = (cb: FrameRequestCallback) => {
    const r = top();
    r.rafs.set(++r.raf, cb);
    return r.raf;
  };
  g.cancelAnimationFrame = (id: number) => void top()?.rafs.delete(id);
  Math.random = () => nextRandom(top());
  try {
    if (perf) Object.defineProperty(perf, "now", { value: () => top().now, configurable: true, writable: true });
  } catch {
    // a performance whose now can't be replaced: three's Clock reads the wall clock
  }
}

function leave(run: Run) {
  if (run.depth === 0 || --run.depth > 0) return;
  live.splice(live.indexOf(run), 1);
  if (live.length || !saved) return;
  const g = globalThis as Rec;
  const perf = g.performance as Rec | undefined;
  g.requestAnimationFrame = saved.raf;
  g.cancelAnimationFrame = saved.caf;
  Math.random = saved.random;
  try {
    if (perf && saved.own) perf.now = saved.now;
    else if (perf) delete perf.now;
  } catch {
    // as entered
  }
  saved = null;
}

function within<T>(run: Run, fn: () => T): T {
  enter(run);
  try {
    return fn();
  } finally {
    leave(run);
  }
}

// ---- the stand-ins ----

/** What a program draws on: a raea element where the composition has a DOM, so `document.body.appendChild
 *  (renderer.domElement)` takes it (createElement("canvas") is a 2D OffscreenCanvas here, which isn't a node), with
 *  what three and its controls ask of a canvas; a plain stand-in where there's no DOM. */
export function programCanvas(width: number, height: number): HTMLCanvasElement {
  const noop = () => {};
  const doc = (globalThis as Rec).document as Rec | undefined;
  let el: Rec | undefined;
  try {
    el = doc?.createElement?.("div");
  } catch {
    el = undefined;
  }
  el ??= { style: {} };
  const asks: Rec = {
    width, height, addEventListener: noop, removeEventListener: noop, dispatchEvent: () => true, setPointerCapture: noop, releasePointerCapture: noop,
    hasPointerCapture: () => false, focus: noop, getContext: () => null, getRootNode: () => doc ?? el, ownerDocument: doc ?? null,
    getBoundingClientRect: () => ({ left: 0, top: 0, width, height, x: 0, y: 0, right: width, bottom: height }),
  };
  for (const [k, v] of Object.entries(asks)) {
    if (k in el) continue;
    try {
      Object.defineProperty(el, k, { value: v, writable: true, configurable: true });
    } catch {
      // a sealed element: what it has
    }
  }
  return el as unknown as HTMLCanvasElement;
}

/** The renderer a program's `new THREE.WebGLRenderer()` (or WebGPURenderer) makes: R3F's stand-in (session.ts: what's
 *  set on it is read back, a render into a target is done where raea can), which records a render to the screen and
 *  its animation loop into the program running it. */
export function standIn(params: { canvas?: unknown } | undefined, webgpu: boolean): THREE.WebGLRenderer {
  const run = live.length ? top() : null;
  const kind = webgpu ? "WebGPURenderer" : "WebGLRenderer";
  if (!run) warnOnce(`new ${kind}()`, "outside <Three program>: raea draws a plain three.js program from inside <Three program={fn}>, its script a function it can run from the start; this one draws nothing");
  const base = standInRenderer() as unknown as Rec;
  let self: Rec = {};
  const render = (scene: THREE.Object3D, camera: THREE.Camera) => {
    if (!run || !scene || !camera) return;
    const d = { scene, camera, gl: self as unknown as THREE.WebGLRenderer };
    if ((scene as Rec).isScene) run.scene = d;
    if (base.getRenderTarget()) return base.render(scene, camera);
    // three/webgpu's PostProcessing: the scene its chain starts from, the chain recorded at the run's clock (tsl-post.ts)
    const p = recordPipeline(scene, self as unknown as THREE.WebGLRenderer, run.now / 1000);
    run.drawn = p ? { ...p, gl: d.gl } : d;
  };
  const own: Rec = {
    domElement: params?.canvas ?? run?.canvas ?? programCanvas(300, 150),
    render,
    setAnimationLoop: (cb: Loop | null) => void (run && (cb ? run.loops.set(self, cb) : run.loops.delete(self))),
    dispose: () => void run?.loops.delete(self),
  };
  // three/webgpu's: ready at once (`await renderer.init()`), its async calls the sync ones
  if (webgpu) Object.assign(own, { isWebGPURenderer: true, init: async () => self, hasInitialized: () => true, renderAsync: async (s: THREE.Object3D, c: THREE.Camera) => render(s, c), hasFeature: () => false, hasFeatureAsync: async () => false });
  self = new Proxy(own, {
    get: (t, k) => (k in t ? t[k] : base[k]),
    set: (t, k, v) => {
      if (k in t) t[k] = v;
      else base[k] = v;
      return true;
    },
    has: (t, k) => k in t || k in base,
  });
  run?.renderers.add(self);
  return self as unknown as THREE.WebGLRenderer;
}

// ---- a run ----

/** `program` run for the <Three> keyed `key`, on `canvas`: what it made recorded, no frame stepped yet */
export function startRun(key: string, program: Program, canvas: HTMLCanvasElement): Run {
  const run: Run = { source: String(program), frame: -1, rand: seedOf(key), now: 0, renderers: new Set(), loops: new Map(), rafs: new Map(), raf: 0, drawn: null, scene: null, canvas, depth: 0 };
  enter(run);
  let out: unknown;
  try {
    out = program(canvas);
  } catch (e) {
    leave(run);
    throw e;
  }
  if (typeof out === "function") run.stop = out as () => void;
  if (!out || typeof (out as Promise<unknown>).then !== "function") {
    leave(run);
    return run;
  }
  // an async program (`await renderer.init()`, then its scene): its globals stay its own until it settles or the
  // moment ends, whichever is first (what it does after a real load reads the page's), and it steps once settled
  const end = () => leave(run);
  setTimeout(end, 0);
  run.waiting = (out as Promise<unknown>)
    .then(
      (r) => void (typeof r === "function" && (run.stop = r as () => void)),
      (error) => void (run.failed = { error }),
    )
    .finally(() => {
      end();
      run.waiting = undefined;
      run.frame = -1;
    });
  return run;
}

/** `run` stepped to frame `frame` (fps a second): each step runs its animation loops and the frame callbacks it asked
 *  for, at that step's time */
export function stepRun(run: Run, frame: number, fps: number) {
  for (let i = run.frame + 1; i <= frame; i++) {
    run.now = (i * 1000) / (fps > 0 ? fps : 30);
    within(run, () => {
      // what its renderers drew into targets is this step's (R3F23): the last step's passes go
      for (const r of run.renderers) (r as { __raea?: { step?: () => void } }).__raea?.step?.();
      const due = [...run.rafs.values()];
      run.rafs.clear();
      for (const cb of [...run.loops.values()]) cb(run.now);
      for (const cb of due) cb(run.now);
    });
    run.frame = i;
  }
}

/** `run` ended: what it returned to undo itself run, its loops dropped */
export function endRun(run: Run) {
  const stop = run.stop;
  run.stop = undefined;
  run.loops.clear();
  run.rafs.clear();
  if (stop) within(run, stop);
}

/** what `run` shows: its last render to the screen. A full-screen pass there (three's EffectComposer, a
 *  FullScreenQuad: a mesh rendered alone) is drawn as the scene it rendered last, without the pass */
export function drawnOf(run: Run): Drawn | null {
  const d = run.drawn;
  if (d && !(d.scene as Rec).isScene && run.scene) {
    warnOnce("a full-screen pass", "renders to the screen (three's EffectComposer, a FullScreenQuad): raea draws the scene it rendered last, without the pass");
    return run.scene;
  }
  return d ?? run.scene;
}
