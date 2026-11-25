// What three.js draws for room.tsx's scenes, worked out on the CPU so check.sh has three's pixels without a browser
// (#165): PMREMGenerator.fromScene(new RoomEnvironment()) and MeshStandardMaterial's shading as three 0.185 does them.
//   bun sdk/r3f/test/room/three.ts      prints each checked pixel's colour
// 1. The room as fromScene renders it into its cube (256 a face, texel centres): three's own RoomEnvironment object
//    (its boxes' matrices, its materials, its point light), each direction traced to the box it hits; a light box is
//    its emissive, a wall or box is lit by the point light (Lambert + BRDF_GGX_Multiscatter, three's attenuation).
//    fromScene's sigma blur (0.04 in the scenes) is left out: it softens only mirror reflections.
// 2. PMREM's GGX VNDF filter chain (_applyGGXFilter: 11 levels, 256 Hammersley samples each, the incremental
//    roughness times 1.25 t), each level read bilinearly within a face.
// 3. textureCubeUV's roughnessToMip, getIBLIrradiance and getIBLRadiance, RE_IndirectSpecular_Physical's
//    multiple scattering with the DFG LUT, RE_Direct_Physical for a directional light, no tone mapping (R3F's flat),
//    sRGB out.
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
// @ts-expect-error: three's DFG LUT (the one WebGLRenderer binds as dfgLUT) is internal, with no declaration file
import { getDFGLUT } from "three/src/renderers/shaders/DFGLUTData.js";

type V3 = THREE.Vector3;
const sat = (x: number) => Math.min(Math.max(x, 0), 1);

// the DFG LUT, sampled as texture2D(dfgLUT, vec2(roughness, dotNV)) with linear filtering
const LUT = (getDFGLUT() as THREE.DataTexture).image.data as Uint16Array;
function dfg(r: number, nv: number): [number, number] {
  const fx = Math.min(Math.max(r * 16 - 0.5, 0), 15), fy = Math.min(Math.max(nv * 16 - 0.5, 0), 15);
  const x0 = Math.floor(fx), y0 = Math.floor(fy), x1 = Math.min(x0 + 1, 15), y1 = Math.min(y0 + 1, 15);
  const tx = fx - x0, ty = fy - y0;
  const at = (x: number, y: number, c: number) => THREE.DataUtils.fromHalfFloat(LUT[(y * 16 + x) * 2 + c]);
  const o: [number, number] = [0, 0];
  for (let c = 0; c < 2; c++) o[c] = (at(x0, y0, c) * (1 - tx) + at(x1, y0, c) * tx) * (1 - ty) + (at(x0, y1, c) * (1 - tx) + at(x1, y1, c) * tx) * ty;
  return o;
}

/** BRDF_GGX_Multiscatter for a grey f0 (f90 1) */
function ggx(l: V3, v: V3, n: V3, f0: number, rough: number) {
  const a2 = rough ** 4;
  const h = l.clone().add(v).normalize();
  const nl = sat(n.dot(l)), nv = sat(n.dot(v)), nh = sat(n.dot(h)), vh = sat(v.dot(h));
  const fr = Math.pow(2, (-5.55473 * vh - 6.98316) * vh);
  const F = f0 * (1 - fr) + fr;
  const V = 0.5 / Math.max(nl * Math.sqrt(a2 + (1 - a2) * nv * nv) + nv * Math.sqrt(a2 + (1 - a2) * nl * nl), 1e-6);
  const D = a2 / (Math.PI * (nh * nh * (a2 - 1) + 1) ** 2);
  const [dv, dl] = [dfg(rough, nv), dfg(rough, nl)];
  const [ev, el] = [1 - dv[0] - dv[1], 1 - dl[0] - dl[1]];
  const favg = f0 + (1 - f0) / 21;
  const fms = ((f0 * dv[0] + dv[1]) * (f0 * dl[0] + dl[1]) * favg) / (1 - ev * el * favg + 1e-6);
  return F * V * D + fms * ev * el;
}

// 1. the room
type Box = { inv: THREE.Matrix4; m: THREE.Matrix4; back: boolean; emissive?: number };
const boxes: Box[] = [];
let light = new THREE.Vector3(), lightK = 0, reach = 0;
{
  const room = new RoomEnvironment();
  room.updateMatrixWorld(true);
  room.traverse((o) => {
    if (o instanceof THREE.PointLight) [light, lightK, reach] = [o.getWorldPosition(new THREE.Vector3()), o.intensity, o.distance];
    if (!(o instanceof THREE.Mesh)) return;
    const mat = o.material as THREE.MeshStandardMaterial | THREE.MeshLambertMaterial;
    const add = (m: THREE.Matrix4) =>
      boxes.push({ m, inv: m.clone().invert(), back: mat.side === THREE.BackSide, emissive: mat instanceof THREE.MeshLambertMaterial ? mat.emissive.r * mat.emissiveIntensity : undefined });
    if (o instanceof THREE.InstancedMesh) {
      for (let i = 0; i < o.count; i++) {
        const at = new THREE.Matrix4();
        o.getMatrixAt(i, at);
        add(o.matrixWorld.clone().multiply(at));
      }
    } else add(o.matrixWorld.clone());
  });
}
function radiance(d: V3): number {
  let best = Infinity, hit: Box | undefined, nl = new THREE.Vector3();
  for (const b of boxes) {
    const lo = new THREE.Vector3().applyMatrix4(b.inv);
    const ld = d.clone().applyMatrix3(new THREE.Matrix3().setFromMatrix4(b.inv));
    let t0 = -Infinity, t1 = Infinity, a0 = -1, a1 = -1;
    for (let k = 0; k < 3; k++) {
      const o = lo.getComponent(k), dd = ld.getComponent(k);
      if (Math.abs(dd) < 1e-12) {
        if (Math.abs(o) > 0.5) t0 = Infinity;
        continue;
      }
      let ta = (-0.5 - o) / dd, tb = (0.5 - o) / dd;
      if (ta > tb) [ta, tb] = [tb, ta];
      if (ta > t0) [t0, a0] = [ta, k];
      if (tb < t1) [t1, a1] = [tb, k];
    }
    if (t0 > t1) continue;
    const [t, axis] = b.back ? [t1, a1] : [t0, a0];
    if (t < 0.1 || t >= best || axis < 0) continue;
    [best, hit] = [t, b];
    nl = new THREE.Vector3();
    nl.setComponent(axis, Math.sign(lo.getComponent(axis) + ld.getComponent(axis) * t) * (b.back ? -1 : 1));
  }
  if (!hit) return 0;
  if (hit.emissive !== undefined) return hit.emissive;
  const p = d.clone().multiplyScalar(best);
  const n = nl.applyMatrix3(new THREE.Matrix3().getNormalMatrix(hit.m)).normalize();
  const to = light.clone().sub(p);
  const dist = to.length();
  const l = to.divideScalar(dist);
  const irr = sat(n.dot(l)) * lightK * (1 / Math.max(dist * dist, 0.01)) * sat(1 - (dist / reach) ** 4) ** 2;
  return irr / Math.PI + irr * ggx(l, d.clone().negate(), n, 0.04, 1);
}

// 2. PMREM: cube levels of 256, 128, 64, 32, then 16 (seven of them), PMREM's face layout (getUV)
type Level = { n: number; px: Float32Array };
function face(d: V3) {
  const [ax, ay, az] = [Math.abs(d.x), Math.abs(d.y), Math.abs(d.z)];
  const f = ax > az ? (ax > ay ? (d.x > 0 ? 0 : 3) : d.y > 0 ? 1 : 4) : az > ay ? (d.z > 0 ? 2 : 5) : d.y > 0 ? 1 : 4;
  const uv = [[d.z / ax, d.y / ax], [-d.x / ay, -d.z / ay], [-d.x / az, d.y / az], [-d.z / ax, d.y / ax], [-d.x / ay, d.z / ay], [d.x / az, d.y / az]][f];
  return { f, u: 0.5 * (uv[0] + 1), v: 0.5 * (uv[1] + 1) };
}
function dir(f: number, u: number, v: number): V3 {
  const [a, b] = [2 * u - 1, 2 * v - 1];
  return new THREE.Vector3(...([[1, b, a], [-a, 1, -b], [-a, b, 1], [-1, b, -a], [-a, -1, b], [a, b, -1]][f] as [number, number, number])).normalize();
}
function sample(l: Level, d: V3) {
  const { f, u, v } = face(d), n = l.n, o = f * n * n;
  const fx = Math.min(Math.max(u * n - 0.5, 0), n - 1), fy = Math.min(Math.max(v * n - 0.5, 0), n - 1);
  const x0 = Math.floor(fx), y0 = Math.floor(fy), x1 = Math.min(x0 + 1, n - 1), y1 = Math.min(y0 + 1, n - 1);
  const [tx, ty] = [fx - x0, fy - y0];
  const p = (x: number, y: number) => l.px[o + y * n + x];
  return (p(x0, y0) * (1 - tx) + p(x1, y0) * tx) * (1 - ty) + (p(x0, y1) * (1 - tx) + p(x1, y1) * tx) * ty;
}
function make(n: number, at: (d: V3) => number): Level {
  const px = new Float32Array(6 * n * n);
  for (let f = 0; f < 6; f++) for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) px[f * n * n + y * n + x] = at(dir(f, (x + 0.5) / n, (y + 0.5) / n));
  return { n, px };
}
const radical = (i: number) => {
  let b = i >>> 0;
  b = ((b << 16) | (b >>> 16)) >>> 0;
  b = (((b & 0x55555555) << 1) | ((b & 0xaaaaaaaa) >>> 1)) >>> 0;
  b = (((b & 0x33333333) << 2) | ((b & 0xcccccccc) >>> 2)) >>> 0;
  b = (((b & 0x0f0f0f0f) << 4) | ((b & 0xf0f0f0f0) >>> 4)) >>> 0;
  b = (((b & 0x00ff00ff) << 8) | ((b & 0xff00ff00) >>> 8)) >>> 0;
  return b * 2.3283064365386963e-10;
};
/** _getGGXShader's pass: V = N, 256 VNDF samples of the level before, weighted by N.L */
function filter(src: Level, n: number, rough: number): Level {
  const alpha = rough * rough;
  const hs = Array.from({ length: 256 }, (_, i) => {
    const r = Math.sqrt(i / 256), phi = 2 * Math.PI * radical(i);
    const [t1, t2] = [r * Math.cos(phi), r * Math.sin(phi)];
    return new THREE.Vector3(alpha * t1, alpha * t2, Math.sqrt(Math.max(0, 1 - t1 * t1 - t2 * t2))).normalize();
  });
  return make(n, (N) => {
    const T = new THREE.Vector3().crossVectors(Math.abs(N.z) < 0.999 ? new THREE.Vector3(0, 0, 1) : new THREE.Vector3(1, 0, 0), N).normalize();
    const B = new THREE.Vector3().crossVectors(N, T);
    let [acc, w] = [0, 0];
    for (const h of hs) {
      const H = T.clone().multiplyScalar(h.x).addScaledVector(B, h.y).addScaledVector(N, h.z).normalize();
      const L = H.multiplyScalar(2 * N.dot(H)).sub(N).normalize();
      const nl = Math.max(N.dot(L), 0);
      if (nl > 0) [acc, w] = [acc + sample(src, L) * nl, w + nl];
    }
    return w > 0 ? acc / w : 0;
  });
}
const SIZES = [256, 128, 64, 32, 16, 16, 16, 16, 16, 16, 16];
const levels: Level[] = [make(256, radiance)];
for (let i = 1; i < SIZES.length; i++) {
  const [t, s] = [i / 10, (i - 1) / 10];
  levels.push(filter(levels[i - 1], SIZES[i], Math.sqrt(t * t - s * s) * t * 1.25));
}

// 3. the material's lookups and shading
function mip(r: number) {
  if (r >= 0.8) return (1 - r) * 5 - 2;
  if (r >= 0.4) return ((0.8 - r) * 3) / 0.4 - 1;
  if (r >= 0.305) return (0.4 - r) / 0.095 + 2;
  if (r >= 0.21) return (0.305 - r) / 0.095 + 3;
  return -2 * Math.log2(1.16 * r);
}
/** textureCubeUV: mip m is level 8 - m */
function cubeUV(d: V3, rough: number) {
  const m = Math.min(Math.max(mip(rough), -2), 8), mi = Math.floor(m), mf = m - mi;
  const c0 = sample(levels[8 - mi], d);
  return mf === 0 ? c0 : c0 * (1 - mf) + sample(levels[8 - mi - 1], d) * mf;
}
const srgb = (c: number) => Math.round(255 * sat(c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055));

/** a MeshStandardMaterial pixel (metalness 0) lit by the room at `envK` and a directional light */
export function shade(hex: string, rough: number, n: V3, v: V3, envK: number, sun?: { dir: V3; k: number }): number[] {
  const c = new THREE.Color(hex);
  const fab = dfg(rough, sat(n.dot(v)));
  const single = 0.04 * fab[0] + fab[1], ems = 1 - fab[0] - fab[1], favg = 0.04 + 0.96 / 21;
  const multi = ((single * favg) / (1 - ems * favg)) * ems;
  const E = cubeUV(n, 1) * envK;
  const r = v.clone().negate().reflect(n).lerp(n, rough ** 4).normalize();
  const rad = cubeUV(r, rough) * envK;
  const irr = sun ? sat(n.dot(sun.dir)) * sun.k : 0;
  const spec = sun ? irr * ggx(sun.dir, v, n, 0.04, rough) : 0;
  return [c.r, c.g, c.b].map((a) => srgb(irr * a / Math.PI + spec + rad * single + multi * E + a * (1 - single - multi) * E));
}

if (import.meta.main) {
  const front = new THREE.Vector3(0, 0, 1);
  // room.tsx "repro": #131's box, camera (0, 0, 5), a directional light from (2, 4, 4) at 1.5, the frame's middle
  console.log("repro 200,200", shade("#D97757", 0.5, front, front, 1, { dir: new THREE.Vector3(2, 4, 4).normalize(), k: 1.5 }).join(","));
  // room.tsx "grey": two grey boxes at x -0.8 and 0.8 (front faces at z 0.6), camera (0, 0, 6), the room at 0.5
  for (const [x, r, px] of [[-0.8, 0.5, 97], [0.8, 1, 303]] as const) console.log(`grey ${px},200`, shade("#808080", r, front, new THREE.Vector3(-x, 0, 5.4).normalize(), 0.5).join(","));
}
