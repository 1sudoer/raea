// postprocessing's effects raea has no effect of its own for, as raea <FramePass>es (docs/r3f.feature R3F25): each
// effect's mainImage or mainUv (its GLSL in pmndrs' postprocessing, or @react-three/postprocessing's own) ported to
// WGSL, read with its own uniforms as the effect set them (a scanline's count, a grid's scale, a dot screen's angle:
// what its setSize and setters worked out), then blended over the frame by its blendFunction and opacity, as its
// EffectPass blends it. A pass runs where the effect does in the composer: on the frame before tone mapping, or after
// it when it comes after a <ToneMapping>. The effects that read the frame elsewhere, a texture or the depth are in
// effects-sampled.ts.
import type * as THREE from "three";
import type { Part } from "./material";
import { SAMPLED } from "./effects-sampled";
import { TSL } from "./effects-tsl";

/** an effect of pmndrs' postprocessing, as post.ts reads it */
export interface PPEffect {
  name: string;
  blendMode?: { blendFunction?: number; opacity?: { value: number } };
  uniforms?: Map<string, { value: unknown }>;
  defines?: Map<string, string>;
  inputColorSpace?: string | null;
  [k: string]: unknown;
}

/** where an effect is drawn: the composer's clock (R3F's, in seconds), the frame's size, whether a <ToneMapping> comes
 *  before it, the composer's camera, and how to name what's drawn differently */
export interface Ctx {
  time: number;
  size: [number, number];
  after: boolean;
  camera?: THREE.Camera;
  warn: (k: string, why: string) => void;
}

/** what an effect adds to its pass beyond its own uniforms: values worked out here, textures (a picture each, as a
 *  <FramePass>'s `textures` takes it), the depth prepass */
export interface Extra {
  uniforms?: Record<string, number | number[]>;
  textures?: Record<string, unknown>;
  prepass?: boolean;
}

/** one effect in WGSL: `c` is the frame's colour, `p` the uv as GL has it (y up), `o` what it outputs; its uniforms
 *  (u_NAME()) are the effect's own, plus `time`, `resolution` and what `extra` works out. `uv`: a mainUv effect,
 *  whose output is the frame read at its moved uv, never blended. `srgb`: an effect that reads sRGB colours
 *  (inputColorSpace), as its EffectPass hands them over and takes them back */
export interface Spec {
  uniforms: string[];
  body: string;
  extra?: (e: PPEffect, ctx: Ctx) => Extra | undefined;
  uv?: boolean;
  srgb?: boolean;
}

/** three's `rand` (ShaderChunk common), its sRGB transfer, the frame read at a GL uv, and postprocessing's blend
 *  functions, by BlendFunction's number */
const PRELUDE = /* wgsl */ `
fn pp_rand(uv: vec2<f32>) -> f32 {
  let dt = dot(uv, vec2<f32>(12.9898, 78.233));
  let sn = dt - 3.141592653589793 * floor(dt / 3.141592653589793);
  return fract(sin(sn) * 43758.5453);
}
fn pp_srgb(c: vec3<f32>) -> vec3<f32> {
  return select(pow(max(c, vec3<f32>(0.0)), vec3<f32>(0.41666)) * 1.055 - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}
fn pp_linear(c: vec3<f32>) -> vec3<f32> {
  return select(pow(max(c, vec3<f32>(0.0)) * 0.9478672986 + 0.0521327014, vec3<f32>(2.4)), c * 0.0773993808, c <= vec3<f32>(0.04045));
}
fn pp_frame(q: vec2<f32>) -> vec4<f32> {
  return frame(vec2<f32>(q.x, 1.0 - q.y));
}
fn pp_blend(dst: vec4<f32>, src: vec4<f32>, opacity: f32, mode: i32) -> vec4<f32> {
  let a = dst.rgb;
  let b = src.rgb;
  var c = b;
  switch mode {
    case 0: { c = a + b; }
    case 1: { return mix(dst, src, src.a * opacity); }
    case 2: { c = (a + b) * 0.5; }
    case 4: { c = mix(step(vec3<f32>(0.0), b) * (1.0 - min(vec3<f32>(1.0), (1.0 - a) / max(b, vec3<f32>(1e-9)))), vec3<f32>(1.0), step(vec3<f32>(1.0), a)); }
    case 5: { c = step(vec3<f32>(0.0), a) * mix(min(vec3<f32>(1.0), a / max(1.0 - b, vec3<f32>(1e-9))), vec3<f32>(1.0), step(vec3<f32>(1.0), b)); }
    case 6: { c = min(a, b); }
    case 7: { c = abs(a - b); }
    case 8: { c = a / max(b, vec3<f32>(1e-9)); }
    case 9: { return dst; }
    case 10: { c = a + b - 2.0 * a * b; }
    case 11: { let x = min(a, vec3<f32>(1.0)); let y = min(b, vec3<f32>(1.0)); c = mix(2.0 * x * y, 1.0 - 2.0 * (1.0 - x) * (1.0 - y), step(vec3<f32>(0.5), y)); }
    case 16: { c = max(a, b); }
    case 17: { c = clamp(b + a - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)); }
    case 18: { c = min(a + b, vec3<f32>(1.0)); }
    case 21: { c = a * b; }
    case 22: { c = max(1.0 - abs(1.0 - a - b), vec3<f32>(0.0)); }
    case 23: { return mix(dst, src, opacity); }
    case 24: { c = mix(2.0 * b * a, 1.0 - 2.0 * (1.0 - b) * (1.0 - a), step(vec3<f32>(0.5), a)); }
    case 26: { c = mix(min(a * a / max(1.0 - b, vec3<f32>(1e-9)), vec3<f32>(1.0)), b, step(vec3<f32>(1.0), b)); }
    case 28: { c = a + b - min(a * b, vec3<f32>(1.0)); }
    case 30: { return src; }
    case 31: { c = max(a - b, vec3<f32>(0.0)); }
    default: { return mix(dst, src, opacity); }
  }
  return mix(dst, vec4<f32>(c, max(dst.a, src.a)), opacity);
}
`;

/** the blend functions PRELUDE has (the rest are drawn as NORMAL, named once) */
const BLENDS = new Set([0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 16, 17, 18, 21, 22, 23, 24, 26, 28, 30, 31]);

/** the effects that read only the frame's own pixel */
const BODIES: Record<string, Spec> = {
  NoiseEffect: {
    uniforms: [],
    extra: (e) => ({ uniforms: { premultiply: e.defines?.has("PREMULTIPLY") ? 1 : 0 } }),
    body: "let n = vec3<f32>(pp_rand(p * (1.0 + u_time())));\n  o = select(vec4<f32>(n, c.a), vec4<f32>(min(c.rgb * n, vec3<f32>(1.0)), c.a), u_premultiply() > 0.5);",
  },
  ScanlineEffect: {
    uniforms: ["count", "scrollSpeed"],
    body: "let y = p.y + u_time() * u_scrollSpeed();\n  let sl = vec2<f32>(sin(y * u_count()), cos(y * u_count()));\n  o = vec4<f32>(sl.x, sl.y, sl.x, c.a);",
  },
  SepiaEffect: {
    uniforms: ["weightsR", "weightsG", "weightsB"],
    body: "o = vec4<f32>(dot(c.rgb, u_weightsR()), dot(c.rgb, u_weightsG()), dot(c.rgb, u_weightsB()), c.a);",
  },
  DotScreenEffect: {
    uniforms: ["angle", "scale"],
    body: "let q = p * u_resolution();\n  let pt = u_scale() * vec2<f32>(dot(u_angle().yx, vec2<f32>(q.x, -q.y)), dot(u_angle(), q));\n  o = vec4<f32>(c.rgb * 10.0 - 5.0 + sin(pt.x) * sin(pt.y) * 4.0, c.a);",
  },
  ColorAverageEffect: {
    uniforms: [],
    body: "o = vec4<f32>(vec3<f32>(dot(c.rgb, vec3<f32>(0.3333333))), c.a);",
  },
  ColorDepthEffect: {
    uniforms: ["factor"],
    body: "o = vec4<f32>(floor(c.rgb * u_factor() + 0.5) / u_factor(), c.a);",
  },
  GridEffect: {
    uniforms: ["scale", "lineWidth"],
    body: "let g = 0.5 - max(abs(fract(p.x * u_scale().x) - 0.5), abs(fract(p.y * u_scale().y) - 0.5));\n  o = vec4<f32>(vec3<f32>(smoothstep(0.0, max(u_lineWidth(), 1e-6), g)), c.a);",
  },
};

const SPECS: Record<string, Spec> = { ...BODIES, ...SAMPLED, ...TSL };

/** the effects this module draws */
export const FRAME_EFFECTS = new Set(Object.keys(SPECS));

/** a uniform's value as a FramePass takes it: a number, or a vector's components */
export function valueOf(v: unknown): number | number[] | undefined {
  if (typeof v === "number") return v;
  if (typeof v === "boolean") return v ? 1 : 0;
  if (Array.isArray(v) && v.length >= 2 && v.length <= 4 && v.every((x) => typeof x === "number")) return v as number[];
  const o = v as { x?: number; y?: number; z?: number; w?: number; r?: number; g?: number; b?: number } | null;
  if (!o || typeof o !== "object") return undefined;
  if (typeof o.r === "number") return [o.r, o.g ?? 0, o.b ?? 0];
  if (typeof o.x === "number" && typeof o.y === "number") return typeof o.w === "number" ? [o.x, o.y, o.z ?? 0, o.w] : typeof o.z === "number" ? [o.x, o.y, o.z] : [o.x, o.y];
  return undefined;
}

/** `e` as a FramePass part, or undefined when it isn't one of these (or lacks what it needs: a texture, its uniforms) */
export function framePass(e: PPEffect, ctx: Ctx): Part | undefined {
  // @react-three/postprocessing's <TiltShift2> names its effect TiltShiftEffect too: it's the one with a taper
  const spec = SPECS[e.name === "TiltShiftEffect" && e.uniforms?.has("taper") ? "TiltShift2Effect" : e.name];
  if (!spec) return undefined;
  let mode = typeof e.blendMode?.blendFunction === "number" ? e.blendMode.blendFunction : 23;
  if (spec.uv) mode = 30;
  else if (!BLENDS.has(mode)) {
    ctx.warn(`<${e.name.replace(/Effect$/, "")}> blend ${mode}`, "isn't one raea blends with: drawn as NORMAL");
    mode = 23;
  }
  const uniforms: Record<string, number | number[]> = { time: ctx.time, resolution: ctx.size, opacity: e.blendMode?.opacity?.value ?? 1, blend: mode };
  for (const k of spec.uniforms) {
    const v = valueOf(e.uniforms?.get(k)?.value);
    if (v === undefined) return undefined;
    uniforms[k] = v;
  }
  const extra = spec.extra ? spec.extra(e, ctx) : {};
  if (!extra) return undefined;
  Object.assign(uniforms, extra.uniforms);
  const [into, back] = spec.srgb ? ["pp_srgb(", ")"] : ["", ""];
  const wgsl = `${PRELUDE}
fn shade(in: FullscreenVertexOutput) -> vec4<f32> {
  let f = frame(in.uv);
  let c = vec4<f32>(${into}f.rgb${back}, f.a);
  let p = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
  var o = c;
  ${spec.body}
  let b = pp_blend(c, o, u_opacity(), i32(u_blend()));
  return vec4<f32>(${spec.srgb ? "pp_linear(b.rgb)" : "b.rgb"}, b.a);
}
`;
  const props: Record<string, unknown> = { wgsl, uniforms, at: ctx.after ? "after-tonemap" : "before-tonemap" };
  if (extra.textures) props.textures = extra.textures;
  if (extra.prepass) props.prepass = true;
  return { type: "FramePass", props };
}
