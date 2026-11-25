// Worn paint (docs/examples.feature EX9): the standard material with use written over it, as a surface shader on
// raea's public hook (raea-sys-shader), where it was the engine's own MaterialExtension (raea-sys-wear) until 11-19.
// MeshStandardMaterial's `wear` and raea/pbr's <WearExtension> draw it. Paint mottled by noise, its edges chipped back
// to `chipColor` (the metal under the paint, shinier), grime gathered low and on undersides, and rust run down in
// streaks. The noise is taken in the mesh's own space, not the world's, so a walking robot's wear stays where it is on
// each part instead of swimming over it; parts built in one space (a rigged build's pieces, in the rest pose) share
// it, so their wear lines up across a seam. Edges are found from how fast the surface's normal turns on screen
// against how fast its position does (its curvature, 1 / radius), so a rounded edge of a few millimetres chips and a
// broad face doesn't. raea/wear's wearShader is the other wear: the weather's, baked from a building's own shape.
import type { WearOverlay } from "./overlays";

/** The surface hook. Its uniforms: `wear` (amount, a blotch's size in metres, chips, grime), `wear2` (rust, seed),
 *  and the chips', grime's and rust's colours. */
export const WORN_WESL = /* wgsl */ `
fn worn_hash(p: vec3<f32>) -> f32 {
  let q = fract(p * 0.3183099 + vec3(0.1, 0.2, 0.3));
  let r = q * 17.0;
  return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

fn worn_vnoise(p: vec3<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = mix(mix(worn_hash(i), worn_hash(i + vec3(1.0, 0.0, 0.0)), u.x), mix(worn_hash(i + vec3(0.0, 1.0, 0.0)), worn_hash(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
  let b = mix(mix(worn_hash(i + vec3(0.0, 0.0, 1.0)), worn_hash(i + vec3(1.0, 0.0, 1.0)), u.x), mix(worn_hash(i + vec3(0.0, 1.0, 1.0)), worn_hash(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
  return mix(a, b, u.z);
}

// four octaves, 0..1
fn worn_fbm(p: vec3<f32>) -> f32 {
  var s = 0.0;
  var w = 0.5;
  var q = p;
  for (var k = 0; k < 4; k++) {
    s += w * worn_vnoise(q);
    q = q * 2.03 + vec3(1.7, 9.2, 3.1);
    w *= 0.5;
  }
  return s / 0.9375;
}

fn surface(in: VertexOutput, pbr: ptr<function, PbrInput>) {
  let amount = u_wear().x;
  let scale = u_wear().y;
  let chips = u_wear().z;
  let grimes = u_wear().w;
  let rusts = u_wear2().x;
  let seed = u_wear2().y;

  var p = in.world_position.xyz;
  var n = normalize(in.world_normal);
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
  // the mesh's own space, so the wear rides with the part
  let m = bevy_pbr::render::mesh_functions::get_local_from_world(in.instance_index);
  p = (m * vec4(in.world_position.xyz, 1.0)).xyz;
  n = normalize((m * vec4(n, 0.0)).xyz);
}
  let q = p / scale + vec3(seed * 13.1);

  // curvature (1 / metres): how fast the normal turns against how fast the position moves, on screen
  let curve = length(fwidth(in.world_normal)) / max(length(fwidth(in.world_position.xyz)), 1e-5);
  let edge = smoothstep(25.0, 90.0, curve) + 0.5 * smoothstep(0.12, 0.35, 1.0 - max(abs(n.x), max(abs(n.y), abs(n.z))));
  let a = amount;

  var c = (*pbr).material.base_color.rgb;
  var rough = (*pbr).material.perceptual_roughness;
  var metal = (*pbr).material.metallic;

  // paint: mottled a little, and duller where it's thin
  let mottle = worn_fbm(q * 0.6);
  c *= 1.0 + (mottle - 0.5) * 0.22 * a;
  rough = clamp(rough + (worn_fbm(q * 1.7 + 5.0) - 0.5) * 0.3 * a, 0.04, 1.0);

  // grime: low, in hollows and undersides, broken up
  let low = clamp(-n.y, 0.0, 1.0) * 0.6 + smoothstep(0.55, 0.85, worn_fbm(q * 0.35 + 20.0));
  let g = clamp(low * grimes * a, 0.0, 0.85) * smoothstep(0.3, 0.7, worn_fbm(q * 2.3 + 40.0) + 0.25);
  c = mix(c, u_grime_color().rgb, g);
  rough = mix(rough, 0.9, g);

  // rust: streaks run down from blotches (noise squashed along local y)
  let streak = worn_fbm(vec3(q.x * 3.0, q.y * 0.25, q.z * 3.0) + 60.0);
  let blotch = smoothstep(0.6, 0.8, worn_fbm(q * 0.5 + 80.0));
  let r = clamp((blotch + smoothstep(0.62, 0.8, streak) * 0.6) * rusts * a, 0.0, 0.9);
  c = mix(c, u_rust_color().rgb * (0.7 + 0.6 * worn_vnoise(q * 6.0)), r);
  rough = mix(rough, 0.95, r);
  metal = mix(metal, 0.0, r);

  // chips: the paint gone at edges and a few scattered spots, the metal under it showing, shinier; an edge lowers
  // the bar a long way, so at full wear most edges chip and only the odd spot on a broad face does
  let s = chips * a;
  let field = worn_fbm(q * 2.5 + 100.0) + edge * 0.5 - 0.2;
  let chip = smoothstep(1.0 - s * 0.55, 1.04 - s * 0.55, field);
  c = mix(c, u_chip_color().rgb, chip);
  rough = mix(rough, 0.32, chip);
  metal = mix(metal, 0.85, chip);

  (*pbr).material.base_color = vec4(c, (*pbr).material.base_color.a);
  (*pbr).material.perceptual_roughness = rough;
  (*pbr).material.metallic = metal;
}
`;

const share = (v: number | undefined, d: number) => Math.min(1, Math.max(0, v ?? d));

/** A material's `wear` as the surface shader that draws it: `{ wesl, uniforms }` for its `shader`. */
export function wornShader(w: WearOverlay): { wesl: string; uniforms: Record<string, number[] | string> } {
  const amount = Math.min(1, w.amount);
  return {
    wesl: WORN_WESL,
    uniforms: {
      wear: [amount, w.scale !== undefined && w.scale > 0 ? w.scale : 0.08, share(w.chips, 0.6), share(w.grime, 0.5)],
      wear2: [share(w.rust, 0.25), w.seed ?? 0, 0, 0],
      chip_color: w.chipColor ?? "#3a3c40",
      grime_color: w.grimeColor ?? "#3b3226",
      rust_color: w.rustColor ?? "#7a3f1c",
    },
  };
}
