// three/webgpu's PostProcessing (its RenderPipeline) as raea's frame effects (docs/r3f.feature R3F25). Its render()
// draws one full-screen quad (a QuadMesh) whose material's fragmentNode is the whole chain; the stand-in renderer hands
// that quad here. Its graph is read for the scene pass it starts from (a `pass(scene, camera)`), which raea draws as
// the frame, and for the nodes the frame goes through, in order, each becoming the raea effect that does the same,
// as a composer's effects do (post.ts): bloom, ambient occlusion, depth of field, anti-aliasing and pixelation as
// raea's own, the nodes reading only the frame as FramePasses (effects-tsl.ts), the output's tone mapping as raea's.
// The arithmetic joining them (`scenePassColor.add(bloomPass)`) is taken as the raea effect's own blend; a node raea
// has nothing for is named once and left out.
import * as THREE from "three";
import { TSL_POST } from "./kinds";

interface N {
  isNode?: boolean;
  value?: unknown;
  getChildren?: () => Iterable<N>;
  [k: string]: unknown;
}

const warned = new Set<string>();
function warnOnce(k: string, why: string) {
  if (warned.has(k)) return;
  warned.add(k);
  console.warn(`raea/r3f: ${k} ${why}`);
}

/** what a pipeline was last seen running a scene through: post.ts's effects, and the clock and size they're drawn at
 *  (a plain program's; R3F's own are read from its scene) */
export interface Pipeline {
  effects: unknown[];
  time?: number;
  size: [number, number];
  camera: THREE.Camera;
}

const pipelines = new WeakMap<object, Pipeline>();

/** the pipeline last drawn from `scene`'s pass, if one was */
export function pipelineOf(scene: object): Pipeline | undefined {
  return pipelines.get(scene);
}

// an Fn's call is a plain Node by its static type: its class's name
const typeOf = (n: N) => (n.isShaderCallNodeInternal === true ? "ShaderCallNodeInternal" : ((n.constructor as { type?: string }).type ?? n.constructor.name));
/** a uniform's or a constant's number (or a plain number) */
const num = (n: unknown, d: number) => {
  const v = (n as N | null)?.isNode ? (n as N).value : n;
  return typeof v === "number" && Number.isFinite(v) ? v : d;
};
const vec2 = (n: unknown, d: [number, number]): [number, number] => {
  const v = ((n as N | null)?.isNode ? (n as N).value : n) as { x?: number; y?: number } | null;
  return typeof v?.x === "number" && typeof v.y === "number" ? [v.x, v.y] : d;
};
/** an effect post.ts draws as a FramePass (effects-tsl.ts's spec by `name`), replacing the frame */
const effect = (name: string, uniforms: Record<string, unknown>) => ({ name, blendMode: { blendFunction: 23, opacity: { value: 1 } }, uniforms: new Map(Object.entries(uniforms).map(([k, value]) => [k, { value }])) });
/** one of raea's own effects, as it is */
const part = (type: string, props: Record<string, unknown>) => ({ name: "RaeaPart", part: { type, props } });

/** three's tone mappings as postprocessing's ToneMappingMode (post.ts's TONE): Linear and Custom draw untonemapped */
const MODES: Record<number, number> = { [THREE.ReinhardToneMapping]: 1, [THREE.CineonToneMapping]: 5, [THREE.ACESFilmicToneMapping]: 6, [THREE.AgXToneMapping]: 7, [THREE.NeutralToneMapping]: 8 };

/** the display nodes raea has nothing for yet (kinds.ts TSL_POST) */
const NONE = new Set(Object.entries(TSL_POST).filter(([, k]) => k.status === "none").map(([n]) => n));

/** What a QuadMesh drawn to the screen shows when it is a PostProcessing's: the scene and camera of the pass it starts
 *  from, its chain recorded for that scene (null for any other quad). `gl` is the renderer it's drawn with, `time` a
 *  plain program's clock in seconds (TSL's `time`). */
export function recordPipeline(quad: unknown, gl: THREE.WebGLRenderer, time?: number): { scene: THREE.Object3D; camera: THREE.Camera } | null {
  const out = (quad as { isQuadMesh?: boolean; material?: { fragmentNode?: N } }).material?.fragmentNode;
  if (!(quad as { isQuadMesh?: boolean }).isQuadMesh || !out?.isNode) return null;
  const effects: unknown[] = [];
  const seen = new Set<N>();
  let pass: { scene: THREE.Object3D; camera: THREE.Camera } | null = null;
  let pipeline: { _toneMapping?: number } | null = null;
  const read = (n: N) => {
    const type = typeOf(n);
    if (n.isPassNode === true) {
      const p = { scene: n.scene as THREE.Object3D, camera: n.camera as THREE.Camera };
      if (!pass) pass = p;
      else if (p.scene !== pass.scene) warnOnce("a second pass(scene, camera)", "in one PostProcessing: raea draws the first one's scene, and the others' aren't drawn");
      if (type === "PixelationPassNode") {
        effects.push(part("Look", { mode: "pixel", pixelSize: Math.max(1, Math.round(num(n.pixelSize, 6))) }));
        warnOnce("pixelationPass()'s edges", "aren't drawn: its cells are");
      } else if (type === "SSAAPassNode") effects.push(part("AntiAliasing", { mode: "msaa" }));
      return;
    }
    switch (type) {
      case "BloomNode":
        // three's UnrealBloom adds strength times its blurred mips (their radius-weighted factors sum to about 3): raea's
        // Bloom, added on top too, at 4 times it (in-tslpost, SSIM 0.97)
        effects.push(part("Bloom", { intensity: num(n.strength, 1) * 4, threshold: num(n.threshold, 0), additive: true }));
        return;
      case "GTAONode":
        effects.push(part("AmbientOcclusion", { quality: "medium" }));
        return;
      case "DepthOfFieldNode":
        effects.push(part("DepthOfField", { mode: "bokeh", focus: num(n.focusDistanceNode, 1), maxBlur: Math.max(1, Math.round(num(n.bokehScaleNode, 1) * 16)) }));
        return;
      case "FXAANode":
        effects.push(part("AntiAliasing", { mode: "fxaa" }));
        return;
      case "SMAANode":
      case "TRAANode":
      case "TAAUNode":
        // temporal ones too: raea draws a frame from that frame alone, so a render equals its play
        effects.push(part("AntiAliasing", { mode: "smaa" }));
        return;
      case "FilmNode":
        effects.push(effect(type, { intensity: n.intensityNode ? num(n.intensityNode, 1) : -1 }));
        return;
      case "DotScreenNode":
        effects.push(effect(type, { angle: num(n.angle, 1.57), scale: num(n.scale, 1) }));
        return;
      case "RGBShiftNode":
        effects.push(effect(type, { amount: num(n.amount, 0.005), angle: num(n.angle, 0) }));
        return;
      case "ChromaticAberrationNode":
        effects.push(effect(type, { strength: num(n.strengthNode, 1), center: vec2(n.centerNode, [0.5, 0.5]), scale: num(n.scaleNode, 1.1) }));
        return;
      case "Lut3DNode":
        effects.push(effect(type, { lut: (n.lutNode as N | undefined)?.value ?? null, intensity: num(n.intensityNode, 1) }));
        return;
      case "RenderOutputNode": {
        // the output transform: the pipeline's tone mapping (the renderer's), with three's exposure
        const tm = (n._toneMapping as number | null) ?? pipeline?._toneMapping ?? gl.toneMapping;
        const mode = MODES[tm];
        if (mode !== undefined) effects.push({ name: "ToneMappingEffect", blendMode: { blendFunction: 23 }, mode, exposure: gl.toneMappingExposure ?? 1 });
        return;
      }
      case "ShaderCallNodeInternal": {
        // an Fn's call: sepia() and bleach() are known by what their source holds
        const src = String((n.shaderNode as { jsFunc?: unknown } | undefined)?.jsFunc ?? "");
        const args = (n.rawInputs as unknown[] | undefined) ?? [];
        if (/\.393\b/.test(src) && /\.769\b/.test(src)) effects.push(effect("sepia", {}));
        else if (/\.45\b/.test(src) && /luminance/.test(src)) effects.push(effect("bleach", { amount: num(args[1], 1) }));
        else warnOnce("an Fn of the app's own", "in a PostProcessing isn't run: raea draws the chain's known nodes and the frame without it");
        return;
      }
      default:
        if (NONE.has(type)) warnOnce(`three's ${type.replace(/Node$/, "")} node`, "in a PostProcessing has no raea counterpart yet: left out");
    }
  };
  const visit = (n: N) => {
    if (!n?.isNode || seen.has(n)) return;
    seen.add(n);
    const ctx = n.isContextNode === true ? (n.value as { renderPipeline?: { _toneMapping?: number } } | undefined) : undefined;
    if (ctx?.renderPipeline) pipeline = ctx.renderPipeline;
    for (const c of n.getChildren?.() ?? []) visit(c);
    // after what it reads: the order the frame goes through them
    read(n);
  };
  visit(out);
  if (!pass) return null;
  const { scene, camera } = pass as { scene: THREE.Object3D; camera: THREE.Camera };
  const size = gl.getSize?.(new THREE.Vector2()) ?? new THREE.Vector2(1, 1);
  pipelines.set(scene, { effects, time, size: [size.x, size.y], camera });
  return { scene, camera };
}
