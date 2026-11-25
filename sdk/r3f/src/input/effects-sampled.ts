// postprocessing's effects that read more than the frame's own pixel, as raea <FramePass>es (docs/r3f.feature R3F25,
// effects.ts): the frame elsewhere (a glitch's or a shock wave's moved uv, a tilt shift's blur, an ASCII cell), a
// texture (a glitch's perturbation map, the ASCII characters, a LUT), the depth (god rays). Where three draws a pass of
// its own first (a Kawase blur, the light source's mask), the FramePass does its work in one go, close to it, and says
// so once.
import * as THREE from "three";
import type { Ctx, Extra, PPEffect, Spec } from "./effects";
import { flipsY, pngOf, srcOf } from "./texture";

type U = { value: unknown };
const u = (e: PPEffect, k: string) => (e.uniforms?.get(k) as U | undefined)?.value;
const num = (v: unknown, d: number) => (typeof v === "number" && Number.isFinite(v) ? v : d);

/** a texture as a FramePass takes it, and whether its rows count from the bottom; undefined when raea can't load it */
function picture(t: unknown, sampler: Record<string, unknown>): { pic: unknown; flip: number } | undefined {
  const tex = t as THREE.Texture | null;
  const src = tex && srcOf(tex);
  return src ? { pic: { src, sampler }, flip: flipsY(tex) ? 1 : 0 } : undefined;
}

/** postprocessing's KawaseBlurPass kernels, by KernelSize */
const KERNELS = [[0, 0], [0, 1, 1], [0, 1, 1, 2], [0, 1, 2, 2, 3], [0, 1, 2, 3, 4, 4, 5], [0, 1, 2, 3, 4, 5, 7, 8, 9, 10]];

/** a Kawase blur's spread as a Gaussian's sigma in pixels: each pass averages four taps (k + 0.5) texels off on both
 *  axes, at the pass's resolution scale */
function kawaseSigma(kernelSize: number, resolutionScale: number, scale: number): number {
  const k = KERNELS[Math.max(0, Math.min(KERNELS.length - 1, Math.round(kernelSize)))];
  const texel = 1 / Math.max(resolutionScale, 1e-3);
  return Math.sqrt(k.reduce((s, x) => s + ((x + 0.5) * texel * scale) ** 2, 0));
}

/** a 3D LUT (three's Data3DTexture, n x n x n) as a horizontal strip picture, n slices of blue side by side, red
 *  across each and green down, as postprocessing's 2D LUTs are laid out */
const strips = new WeakMap<object, string>();
function stripOf(t: THREE.Data3DTexture): string | undefined {
  const done = strips.get(t);
  if (done) return done;
  const { data, width: n, height, depth } = t.image as { data: ArrayLike<number>; width: number; height: number; depth: number };
  if (!data || n !== height || n !== depth) return undefined;
  const ch = Math.round(data.length / (n * n * n));
  if (ch < 3) return undefined;
  const float = !(data instanceof Uint8Array || data instanceof Uint8ClampedArray);
  const rgba = new Uint8Array(n * n * n * 4);
  for (let b = 0; b < n; b++)
    for (let g = 0; g < n; g++)
      for (let r = 0; r < n; r++) {
        const from = ((b * n + g) * n + r) * ch;
        const to = (g * n * n + b * n + r) * 4;
        for (let i = 0; i < 3; i++) rgba[to + i] = float ? Math.round(Math.min(1, Math.max(0, data[from + i])) * 255) : data[from + i];
        rgba[to + 3] = 255;
      }
  const s = pngOf(rgba, n * n, n);
  strips.set(t, s);
  return s;
}

const v3 = new THREE.Vector3();
const sphere = new THREE.Sphere();

export const SAMPLED: Record<string, Spec> = {
  // the frame read at a uv a random column or row band flips, nudged by the perturbation map (its update() picks the
  // band and seeds each frame, by R3F's clock and Math.random, seeded in raea)
  GlitchEffect: {
    uv: true,
    uniforms: ["active", "columns", "random", "seeds", "distortion"],
    extra: (e) => {
      const t = picture(u(e, "perturbationMap"), { wrap: "repeat" });
      return t && { textures: { perturb: t.pic }, uniforms: { flip: t.flip } };
    },
    body: `var q = p;
  if (u_active() > 0.5) {
    let cols = u_columns();
    let rnd = u_random();
    let seeds = u_seeds();
    let dist = u_distortion();
    if (q.y < dist.x + cols && q.y > dist.x - cols * rnd) {
      let sx = clamp(ceil(seeds.x), 0.0, 1.0);
      q.y = sx * (1.0 - (q.y + dist.y)) + (1.0 - sx) * dist.y;
    }
    if (q.x < dist.y + cols && q.x > dist.y - cols * rnd) {
      let sy = clamp(ceil(seeds.y), 0.0, 1.0);
      q.x = sy * dist.x + (1.0 - sy) * (1.0 - (q.x + dist.x));
    }
    let tq = q * rnd * rnd;
    let n = textureSampleLevel(tex_perturb, samp_perturb, select(tq, vec2<f32>(tq.x, 1.0 - tq.y), u_flip() > 0.5), 0.0).rg;
    q += n * seeds * (rnd * 0.2);
  }
  o = pp_frame(q);`,
  },
  // the frame pushed out in a ring around the epicentre (its update() projects it and grows the ring)
  ShockWaveEffect: {
    uv: true,
    uniforms: ["active", "center", "cameraDistance", "size", "radius", "maxRadius", "waveSize", "amplitude"],
    body: `var q = p;
  if (u_active() > 0.5) {
    let ac = vec2<f32>(u_resolution().x / u_resolution().y, 1.0);
    let d = q * ac - u_center() * ac;
    let dist = sqrt(dot(d, d)) * (0.1 * u_cameraDistance() / u_size());
    if (dist > u_radius() && dist < u_radius() + u_waveSize()) {
      let angle = (dist - u_radius()) * 6.283185307179586 / u_waveSize();
      let cs = (1.0 - cos(angle)) * 0.5;
      let extent = u_maxRadius() + u_waveSize();
      let decay = max(extent - dist * dist, 0.0) / extent;
      q -= ((cs * u_amplitude() * d) / dist) * decay;
    }
  }
  o = pp_frame(q);`,
  },
  // postprocessing's: the frame blurred outside a band (its Kawase blur passes, here one Gaussian of their spread),
  // the band's own pixels kept
  TiltShiftEffect: {
    uniforms: ["rotation", "maskParams"],
    extra: (e, ctx) => {
      const bp = e.blurPass as { kernelSize?: number; scale?: number; resolution?: { scale?: number }; blurMaterial?: { uniforms?: { maskParams?: U } } } | undefined;
      const mask = bp?.blurMaterial?.uniforms?.maskParams?.value as THREE.Vector4 | undefined;
      if (!mask) return undefined;
      ctx.warn("<TiltShift>", "is blurred in one pass, a Gaussian as wide as its Kawase blur: close to three's, not the same");
      return { uniforms: { blurMask: [mask.x, mask.y, mask.z, mask.w], sigma: kawaseSigma(num(bp?.kernelSize, 2), num(bp?.resolution?.scale, 0.5), num(bp?.scale, 1)) } };
    },
    body: `let res = u_resolution();
  var v2 = (p - 0.5) * 2.0 * vec2<f32>(res.x / res.y, 1.0);
  let rot = u_rotation();
  v2 = vec2<f32>(dot(rot, v2), dot(rot, vec2<f32>(v2.y, -v2.x)));
  let bm = u_blurMask();
  let sg = (1.0 - (smoothstep(bm.x, bm.y, v2.y) - smoothstep(bm.w, bm.z, v2.y))) * u_sigma();
  var acc = vec4<f32>(0.0);
  var wsum = 0.0;
  for (var i = -3; i <= 3; i++) {
    for (var j = -3; j <= 3; j++) {
      let d = vec2<f32>(f32(i), f32(j)) * 0.7;
      let w = exp(-0.5 * dot(d, d));
      acc += pp_frame(p + d * sg / res) * w;
      wsum += w;
    }
  }
  let mp = u_maskParams();
  o = mix(acc / wsum, c, step(mp.x, v2.y) - step(mp.y, v2.y));`,
  },
  // @react-three/postprocessing's <TiltShift2> (Evan Wallace's): the frame blurred along `direction`, more the further
  // from the line from `start` to `end`
  TiltShift2Effect: {
    uniforms: ["blur", "taper", "start", "end", "direction", "samples"],
    body: `let res = u_resolution();
  let sp = u_start() * res;
  let ep = u_end() * res;
  let fs = u_samples();
  let hs = fs / 2.0;
  let maxd = length(res);
  let fc = vec3<f32>(in.position.x, res.y - in.position.y, 0.5);
  let off = fract(sin(dot(fc, vec3<f32>(12.9898, 78.233, 151.7182))) * 43758.5453);
  let nrm = normalize(vec2<f32>(sp.y - ep.y, ep.x - sp.x));
  let radius = smoothstep(0.0, 1.0, abs(dot(p * res - sp, nrm)) / (u_taper() * maxd)) * u_blur() * (maxd / 16.0);
  var acc = vec4<f32>(0.0);
  var total = 0.0;
  for (var i = 0; i <= 100; i++) {
    if (f32(i) >= fs) { break; }
    let pc = (-hs + f32(i) + off - 0.5) / hs;
    let w = 1.0 - abs(pc);
    let s = pp_frame(p + normalize(u_direction()) / res * pc * radius);
    acc += vec4<f32>(s.rgb * s.a, s.a) * w;
    total += w;
  }
  o = acc / total;
  o = vec4<f32>(o.rgb / (o.a + 0.00001), o.a);`,
  },
  // @react-three/postprocessing's <ASCII>: each cell's grey picks a character from the canvas it drew them on
  ASCIIEffect: {
    uniforms: ["uCellSize", "uCharactersCount", "uInvert", "uColor"],
    extra: (e) => {
      const t = picture(u(e, "uCharacters"), { wrap: "repeat", filter: "nearest" });
      return t && { textures: { chars: t.pic }, uniforms: { flip: t.flip } };
    },
    body: `let cell = u_resolution() / u_uCellSize();
  let grid = 1.0 / cell;
  let px = pp_frame(grid * (0.5 + floor(p / grid)));
  var g = dot(px.rgb, vec3<f32>(0.299, 0.587, 0.114));
  if (u_uInvert() > 0.5) { g = 1.0 - g; }
  let ci = floor((u_uCharactersCount() - 1.0) * g);
  let offs = vec2<f32>(ci - 16.0 * floor(ci / 16.0), -floor(ci / 16.0)) / 16.0;
  let m = p * (cell / 16.0);
  let cu = (m - floor(m * 16.0) / 16.0) - vec2<f32>(0.0, 1.0 / 16.0) + offs;
  let ch = textureSampleLevel(tex_chars, samp_chars, select(cu, vec2<f32>(cu.x, 1.0 - cu.y), u_flip() > 0.5), 0.0);
  o = vec4<f32>(u_uColor() * ch.r, px.a);`,
  },
  // postprocessing's LUT3DEffect (<LUT>): the colour looked up in its table, in sRGB as the effect reads it; a 3D table
  // as a strip of slices (a 2D one as it's drawn), between two slices
  LUT3DEffect: {
    srgb: true,
    uniforms: [],
    extra: (e, ctx) => {
      const t = u(e, "lut") as (THREE.Texture & { isData3DTexture?: boolean }) | null;
      if (!t?.image) return undefined;
      const { width, height } = t.image as { width: number; height: number };
      let lut: Extra["textures"];
      let horizontal = width > height ? 1 : 0;
      let size = Math.min(width, height);
      let [tw, th] = [1 / width, 1 / height];
      let flip = 0;
      if (t.isData3DTexture) {
        const s = stripOf(t as THREE.Data3DTexture);
        if (!s) return undefined;
        lut = { lut: { src: s } };
        [horizontal, size, tw, th] = [1, width, 1 / (width * width), 1 / width];
      } else {
        const p = picture(t, {});
        if (!p) return undefined;
        lut = { lut: p.pic };
        flip = p.flip;
      }
      if (e.defines?.has("CUSTOM_INPUT_DOMAIN")) ctx.warn("<LUT> input domain", "isn't read: the table spans 0 to 1");
      return { textures: lut, uniforms: { lut: [size, tw, th, horizontal], flip } };
    },
    body: `let l = u_lut();
  let rgb = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * ((l.x - 1.0) / l.x) + 0.5 / l.x;
  let slice = rgb.b * l.x;
  let s0 = floor(slice);
  let ci = slice - s0 - 0.5;
  let s1 = s0 + sign(ci);
  var uv0 = vec2<f32>(0.0);
  var uv1 = vec2<f32>(0.0);
  if (l.w > 0.5) {
    let xo = clamp(rgb.r * l.z, l.y * 0.5, l.z - l.y * 0.5);
    uv0 = vec2<f32>(s0 * l.z + xo, rgb.g);
    uv1 = vec2<f32>(s1 * l.z + xo, rgb.g);
  } else {
    let yo = clamp(rgb.g * l.y, l.z * 0.5, l.y - l.z * 0.5);
    uv0 = vec2<f32>(rgb.r, s0 * l.y + yo);
    uv1 = vec2<f32>(rgb.r, s1 * l.y + yo);
  }
  if (u_flip() > 0.5) {
    uv0.y = 1.0 - uv0.y;
    uv1.y = 1.0 - uv1.y;
  }
  let a0 = textureSampleLevel(tex_lut, samp_lut, uv0, 0.0);
  let a1 = textureSampleLevel(tex_lut, samp_lut, uv1, 0.0);
  o = vec4<f32>(mix(a0, a1, abs(ci)).rgb, c.a);`,
  },
  // god rays: light streaked from the sun toward each pixel, decaying, where the sun is seen. three draws the light
  // source into a mask of its own (the scene's depth in front of it) and blurs the rays; here the mask is the sun's
  // disc on screen where the depth prepass has nothing nearer, the rays unblurred
  GodRaysEffect: {
    uniforms: [],
    extra: (e, ctx) => {
      const sun = (e.lightSource ?? e._lightSource) as THREE.Mesh | undefined;
      const cam = (e.camera as THREE.Camera | undefined) ?? ctx.camera;
      const m = e.godRaysMaterial as { uniforms?: Record<string, U>; defines?: Record<string, string> } | undefined;
      if (!sun?.isMesh || !cam || !m?.uniforms) return undefined;
      sun.updateWorldMatrix(true, false);
      cam.updateWorldMatrix(true, false);
      const g = sun.geometry;
      if (!g.boundingSphere) g.computeBoundingSphere();
      sphere.copy(g.boundingSphere ?? new THREE.Sphere(new THREE.Vector3(), 1)).applyMatrix4(sun.matrixWorld);
      const z = -v3.copy(sphere.center).applyMatrix4(cam.matrixWorldInverse).z;
      if (z <= 0) return undefined;
      const ndc = v3.copy(sphere.center).project(cam);
      const persp = cam as THREE.PerspectiveCamera;
      const fy = persp.isPerspectiveCamera ? 1 / Math.tan(THREE.MathUtils.degToRad(persp.fov) / 2) : 1;
      const aspect = ctx.size[0] / ctx.size[1];
      const ry = persp.isPerspectiveCamera ? (sphere.radius / z) * fy * 0.5 : 0.05;
      const mat = (Array.isArray(sun.material) ? sun.material[0] : sun.material) as THREE.MeshBasicMaterial;
      const col = mat?.color ?? new THREE.Color(1, 1, 1);
      const a = mat?.transparent ? num(mat.opacity, 1) : 1;
      const un = (k: string, d: number) => num(m.uniforms?.[k]?.value, d);
      ctx.warn("<GodRays>", "masks the light by its disc on screen and the depth, its rays unblurred: close to three's, not the same");
      return {
        prepass: true,
        uniforms: {
          sun: [(ndc.x + 1) / 2, (ndc.y + 1) / 2, ry / aspect, ry],
          rays: [un("density", 0.96), un("decay", 0.9), un("weight", 0.4), un("exposure", 0.6)],
          more: [un("clampMax", 1), Number(m.defines?.SAMPLES_INT ?? 60), z, sphere.radius],
          color: [col.r * a, col.g * a, col.b * a],
        },
      };
    },
    body: `let sun = u_sun();
  let rays = u_rays();
  let more = u_more();
  let n = max(more.y, 1.0);
  var coord = p;
  let delta = (sun.xy - p) * (1.0 / n) * rays.x;
  var decay = 1.0;
  var acc = vec3<f32>(0.0);
  for (var i = 0; i < 256; i++) {
    if (f32(i) >= n) { break; }
    coord += delta;
    let d = (coord - sun.xy) / sun.zw;
    if (dot(d, d) <= 1.0 && depth(vec2<f32>(coord.x, 1.0 - coord.y)) >= more.z - more.w * 1.05) {
      acc += u_color() * decay * rays.z;
    }
    decay *= rays.y;
  }
  o = vec4<f32>(clamp(acc * rays.w, vec3<f32>(0.0), vec3<f32>(more.x)), c.a);`,
  },
};
