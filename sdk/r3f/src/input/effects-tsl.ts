// three/webgpu's post nodes (three/addons/tsl/display) that read only the frame, as raea FramePasses doing what each
// node's setup() builds (docs/r3f.feature R3F25). tsl-post.ts finds them in a PostProcessing's graph and hands them to
// post.ts as effects named by the node, their values as uniforms. A node's uv is WebGPU's, y down as raea's frame()
// reads it: `t` below. Each replaces the frame (NORMAL at full opacity), as the node's output does.
import type { Spec } from "./effects";
import { SAMPLED } from "./effects-sampled";

const T = "let t = vec2<f32>(p.x, 1.0 - p.y);";

export const TSL: Record<string, Spec> = {
  // film(): the colour plus itself times clamp(rand(fract(uv + time)) + 0.1), mixed in by its intensity (-1: none given)
  FilmNode: {
    uniforms: ["intensity"],
    body: `${T}
  let n = clamp(pp_rand(fract(t + u_time())) + 0.1, 0.0, 1.0);
  let film = c.rgb + c.rgb * n;
  o = vec4<f32>(select(mix(c.rgb, film, u_intensity()), film, u_intensity() < 0.0), c.a);`,
  },
  // dotScreen(): the grey's contrast raised against a pattern of dots turned by angle, in pixels times scale
  DotScreenNode: {
    uniforms: ["angle", "scale"],
    body: `${T}
  let q = t * u_resolution();
  let sc = vec2<f32>(sin(u_angle()), cos(u_angle()));
  let pt = vec2<f32>(sc.y * q.x - sc.x * q.y, sc.x * q.x + sc.y * q.y) * u_scale();
  o = vec4<f32>(vec3<f32>((c.r + c.g + c.b) / 3.0 * 10.0 - 5.0 + sin(pt.x) * sin(pt.y) * 4.0), c.a);`,
  },
  // rgbShift(): red read amount along angle, blue as far the other way
  RGBShiftNode: {
    uv: true,
    uniforms: ["amount", "angle"],
    body: `${T}
  let d = vec2<f32>(cos(u_angle()), sin(u_angle())) * u_amount();
  let ga = frame(t);
  o = vec4<f32>(frame(t + d).r, ga.g, frame(t - d).b, ga.a);`,
  },
  // chromaticAberration(): red scaled out from the centre and blue in, each further the farther from it
  ChromaticAberrationNode: {
    uv: true,
    uniforms: ["strength", "center", "scale"],
    body: `${T}
  let off = t - u_center();
  let k = u_scale() * 0.02 * u_strength();
  let ab = u_strength() * length(off) * 0.01;
  let rr = frame(u_center() + off * (1.0 + k) + off * ab).r;
  let bb = frame(u_center() + off * (1.0 - k) - off * ab).b;
  o = vec4<f32>(rr, frame(t).g, bb, frame(t).a);`,
  },
  // sepia() (Sepia.js): glfx's sepia weights
  sepia: {
    uniforms: [],
    body: "o = vec4<f32>(dot(c.rgb, vec3<f32>(0.393, 0.769, 0.189)), dot(c.rgb, vec3<f32>(0.349, 0.686, 0.168)), dot(c.rgb, vec3<f32>(0.272, 0.534, 0.131)), c.a);",
  },
  // bleach() (BleachBypass.js): a multiply below mid-grey and a screen above it, by the luminance, mixed by its amount
  bleach: {
    uniforms: ["amount"],
    body: `let lum = dot(c.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
  let l = clamp(10.0 * (lum - 0.45), 0.0, 1.0);
  let nc = mix(2.0 * lum * c.rgb, 1.0 - 2.0 * (1.0 - lum) * (1.0 - c.rgb), l);
  let a2 = c.a * u_amount();
  o = vec4<f32>(a2 * nc + c.rgb * (1.0 - a2), c.a);`,
  },
  // lut3D(): the colour looked up in its 3D table (as postprocessing's LUT3DEffect, without its sRGB round trip: the
  // node reads the colour as the graph has it), mixed in by its intensity
  Lut3DNode: {
    ...SAMPLED.LUT3DEffect,
    srgb: false,
    uniforms: ["intensity"],
    body: `${SAMPLED.LUT3DEffect.body}
  o = mix(c, o, u_intensity());`,
  },
};
