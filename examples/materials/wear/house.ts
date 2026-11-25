// The corner house raea/wear's demo weathers (docs/wear.feature WR7), made in code: a four-storey block on a street
// corner, its south front on x = 0..12 facing +z, its east front on z = -10..0 facing +x, the corner at (12, 0).
// A sandstone plinth, a rusticated ground floor, plastered storeys, sandstone sills and a string course, a cornice
// on consoles under a slate roof, a door in a sandstone surround up granite steps, an iron balcony, a zinc gutter and
// downpipe, an enamel street plate on each face of the corner, a cellar hatch in the walk, a brick firewall on the
// west, and the walk all round. The openings go through the walls (glass 15 cm in, frames in the reveals), so cracks
// can start at their corners. One glTF material per wear class, named for it. y up, metres.
//   houseGlb()  the .glb's bytes (make-house.ts writes assets/house.glb)

type V3 = [number, number, number];
/** a hexahedron: its eight corners, corner (i, j, k) at index i + 2j + 4k */
type Solid = V3[];

export const CLASSES = ["plaster", "brick", "sandstone", "granite", "wood", "iron", "zinc", "copper", "slate", "felt", "glass", "enamel", "brass", "ground"] as const;
export type HouseClass = (typeof CLASSES)[number];

const LOOK: Record<HouseClass, { color: V3; rough: number; metal: number }> = {
  plaster: { color: [0.8, 0.72, 0.56], rough: 0.85, metal: 0 },
  brick: { color: [0.45, 0.2, 0.13], rough: 0.9, metal: 0 },
  sandstone: { color: [0.68, 0.57, 0.4], rough: 0.85, metal: 0 },
  granite: { color: [0.46, 0.46, 0.47], rough: 0.7, metal: 0 },
  wood: { color: [0.17, 0.28, 0.21], rough: 0.6, metal: 0 },
  iron: { color: [0.06, 0.07, 0.065], rough: 0.5, metal: 0.6 },
  zinc: { color: [0.55, 0.58, 0.6], rough: 0.4, metal: 0.9 },
  copper: { color: [0.62, 0.33, 0.18], rough: 0.35, metal: 1 },
  slate: { color: [0.2, 0.21, 0.24], rough: 0.7, metal: 0 },
  felt: { color: [0.08, 0.08, 0.08], rough: 0.9, metal: 0 },
  glass: { color: [0.07, 0.09, 0.11], rough: 0.05, metal: 0.2 },
  enamel: { color: [0.08, 0.16, 0.5], rough: 0.25, metal: 0 },
  brass: { color: [0.75, 0.58, 0.25], rough: 0.3, metal: 1 },
  ground: { color: [0.33, 0.32, 0.3], rough: 0.9, metal: 0 },
};

export const SOUTH = { x0: 0, x1: 12 };
export const EAST = { z0: -10, z1: 0 };
export const TOP = 14.2;
/** the storeys' floor heights and their windows' sill and head */
export const STOREYS = [
  { floor: 0.6, sill: 1.5, head: 3.4 },
  { floor: 4.2, sill: 5.1, head: 7.2 },
  { floor: 7.6, sill: 8.5, head: 10.5 },
  { floor: 10.9, sill: 11.7, head: 13.5 },
];
/** the windows' middles along each front (metres from its west or south end), 1.2 m wide; the door's bay is 6 on the south */
export const BAYS = { south: [1.5, 4.0, 6.0, 8.5, 10.6], east: [1.6, 4.4, 7.2, 9.0] };
export const DOOR = { at: 6.0, w: 1.4, h: 2.9 };
const WIN = 1.2;
const T = 0.5;

const parts = new Map<HouseClass, Solid[]>(CLASSES.map((c) => [c, []]));
/** a parallelepiped from a corner and its three edges */
const add = (c: HouseClass, [o, u, v, w]: [V3, V3, V3, V3]) => {
  const s: Solid = [];
  for (let n = 0; n < 8; n++) {
    const [i, j, k] = [n & 1, (n >> 1) & 1, (n >> 2) & 1];
    s.push([o[0] + u[0] * i + v[0] * j + w[0] * k, o[1] + u[1] * i + v[1] * j + w[1] * k, o[2] + u[2] * i + v[2] * j + w[2] * k]);
  }
  parts.get(c)!.push(s);
};
/** a slab `t` thick upward from a planar quad (its corners in order round it) */
const slab = (c: HouseClass, q: [V3, V3, V3, V3], t: number) => {
  const base = [q[0], q[1], q[3], q[2]];
  parts.get(c)!.push([...base, ...base.map((p): V3 => [p[0], p[1] + t, p[2]])]);
};
/** an axis-aligned box from its two corners */
const box = (c: HouseClass, a: V3, b: V3) => add(c, [a, [b[0] - a[0], 0, 0], [0, b[1] - a[1], 0], [0, 0, b[2] - a[2]]]);

/** A front: `along` and `out` its axes, `at` where it starts; a wall of `c` from y0 to y1 with rectangular openings
 *  (u0, u1, v0, v1 along it and up) cut through, as boxes round them. */
function wall(c: HouseClass, at: V3, along: V3, out: V3, len: number, y0: number, y1: number, holes: [number, number, number, number][], depth = T, proud = 0) {
  const cuts = [...new Set([0, len, ...holes.flatMap((h) => [h[0], h[1]])])].sort((a, b) => a - b);
  for (let i = 0; i + 1 < cuts.length; i++) {
    const [u0, u1] = [cuts[i], cuts[i + 1]];
    const mid = (u0 + u1) / 2;
    // the openings in this column, clipped to the wall's own height
    const cut = holes.filter((h) => h[0] <= mid && mid <= h[1] && h[3] > y0 && h[2] < y1).flatMap((h) => [Math.max(h[2], y0), Math.min(h[3], y1)]);
    const ys = [y0, y1, ...cut].sort((a, b) => a - b);
    for (let j = 0; j + 1 < ys.length; j += 2) {
      if (ys[j + 1] - ys[j] < 1e-4) continue;
      const o: V3 = [at[0] + along[0] * u0 - out[0] * (depth - proud), ys[j], at[2] + along[2] * u0 - out[2] * (depth - proud)];
      add(c, [o, [along[0] * (u1 - u0), 0, along[2] * (u1 - u0)], [0, ys[j + 1] - ys[j], 0], [out[0] * depth, 0, out[2] * depth]]);
    }
  }
}

/** A window in the opening (u0..u1, v0..v1) of the front at `at`: glass 15 cm in, a frame round it in the reveal and a
 *  transom, a sandstone sill below standing out 8 cm. */
function window(at: V3, along: V3, out: V3, u0: number, u1: number, v0: number, v1: number, sill = true) {
  const p = (u: number, v: number, o: number): V3 => [at[0] + along[0] * u + out[0] * o, v, at[2] + along[2] * u + out[2] * o];
  const piece = (cl: HouseClass, ua: number, ub: number, va: number, vb: number, oa: number, ob: number) => {
    const o = p(ua, va, oa);
    add(cl, [o, [along[0] * (ub - ua), 0, along[2] * (ub - ua)], [0, vb - va, 0], [out[0] * (ob - oa), 0, out[2] * (ob - oa)]]);
  };
  piece("glass", u0 + 0.06, u1 - 0.06, v0 + 0.06, v1 - 0.06, -0.17, -0.15);
  piece("wood", u0, u0 + 0.06, v0, v1, -0.2, -0.1);
  piece("wood", u1 - 0.06, u1, v0, v1, -0.2, -0.1);
  piece("wood", u0, u1, v0, v0 + 0.06, -0.2, -0.1);
  piece("wood", u0, u1, v1 - 0.06, v1, -0.2, -0.1);
  const tr = v0 + (v1 - v0) * 0.72;
  piece("wood", u0, u1, tr - 0.03, tr + 0.03, -0.2, -0.12);
  piece("wood", (u0 + u1) / 2 - 0.03, (u0 + u1) / 2 + 0.03, v0, tr, -0.2, -0.12);
  if (sill) piece("sandstone", u0 - 0.08, u1 + 0.08, v0 - 0.08, v0, -0.12, 0.08);
}

function build() {
  for (const v of parts.values()) v.length = 0;
  const S: V3 = [SOUTH.x0, 0, 0];
  const E: V3 = [SOUTH.x1, 0, EAST.z1];
  const X: V3 = [1, 0, 0];
  const Z: V3 = [0, 0, 1];
  const NZ: V3 = [0, 0, -1];
  const fronts = [
    { at: S, along: X, out: Z, len: SOUTH.x1 - SOUTH.x0, bays: BAYS.south, door: true },
    { at: E, along: NZ, out: X, len: EAST.z1 - EAST.z0, bays: BAYS.east, door: false },
  ];
  for (const f of fronts) {
    const holes: [number, number, number, number][] = [];
    for (const [k, st] of STOREYS.entries()) {
      for (const b of f.bays) {
        if (f.door && k === 0 && b === DOOR.at) continue;
        holes.push([b - WIN / 2, b + WIN / 2, st.sill, st.head]);
      }
    }
    if (f.door) holes.push([DOOR.at - DOOR.w / 2, DOOR.at + DOOR.w / 2, 0.6, 0.6 + DOOR.h]);
    // the ground floor rendered in rusticated bands (a 2 cm groove every 0.4 m), the storeys above plain
    const ground = holes.filter((h) => h[2] < 4.2);
    for (let y = 0.6; y < 4.2 - 1e-6; y += 0.4) {
      const top = Math.min(y + 0.38, 4.2);
      wall("plaster", f.at, f.along, f.out, f.len, y, top, ground);
      wall("plaster", f.at, f.along, f.out, f.len, top, Math.min(y + 0.4, 4.2), ground, T, -0.02);
    }
    wall("plaster", f.at, f.along, f.out, f.len, 4.2, TOP, holes.filter((h) => h[2] >= 4.2));
    // the plinth, standing 8 cm out, the string course over the ground floor and the cornice's frieze
    wall("sandstone", f.at, f.along, f.out, f.len, 0, 0.6, f.door ? [[DOOR.at - DOOR.w / 2, DOOR.at + DOOR.w / 2, 0, 0.6]] : [], T, 0.08);
    wall("sandstone", f.at, f.along, f.out, f.len, 4.05, 4.25, [], T, 0.14);
    for (const h of holes) {
      if (f.door && h[0] === DOOR.at - DOOR.w / 2) continue;
      window(f.at, f.along, f.out, h[0], h[1], h[2], h[3]);
    }
    // the cornice on its consoles, every 0.9 m
    wall("sandstone", f.at, f.along, f.out, f.len + 0.6, TOP, TOP + 0.4, [], T, 0.6);
    for (let u = 0.45; u < f.len; u += 0.9) {
      const p: V3 = [f.at[0] + f.along[0] * u, TOP - 0.45, f.at[2] + f.along[2] * u];
      add("sandstone", [[p[0] - f.along[0] * 0.09, p[1], p[2] - f.along[2] * 0.09], [f.along[0] * 0.18, 0, f.along[2] * 0.18], [0, 0.45, 0], [f.out[0] * 0.45, 0, f.out[2] * 0.45]]);
    }
    // the zinc gutter along the cornice's lip
    add("zinc", [[f.at[0] + f.out[0] * 0.6 - f.along[0] * 0.0, TOP + 0.4, f.at[2] + f.out[2] * 0.6], [f.along[0] * (f.len + 0.6), 0, f.along[2] * (f.len + 0.6)], [0, 0.14, 0], [f.out[0] * 0.16, 0, f.out[2] * 0.16]]);
  }
  // the door: wood in a sandstone surround, granite steps up to it, a brass handle and bell plate, a copper canopy
  const d0 = DOOR.at - DOOR.w / 2, d1 = DOOR.at + DOOR.w / 2;
  box("wood", [d0, 0.6, -0.2], [d1, 0.6 + DOOR.h, -0.12]);
  box("sandstone", [d0 - 0.25, 0.6, 0], [d0, 0.6 + DOOR.h + 0.3, 0.1]);
  box("sandstone", [d1, 0.6, 0], [d1 + 0.25, 0.6 + DOOR.h + 0.3, 0.1]);
  box("sandstone", [d0, 0.6 + DOOR.h, 0], [d1, 0.6 + DOOR.h + 0.3, 0.1]);
  for (const [k, w] of [0.2, 0.4, 0.6].entries()) box("granite", [d0 - 0.4 + k * 0.05, 0, 0.1], [d1 + 0.4 - k * 0.05, w, 0.1 + 0.9 - k * 0.3]);
  box("brass", [d1 - 0.22, 1.55, -0.12], [d1 - 0.12, 1.62, -0.07]);
  box("brass", [d1 + 0.05, 1.75, 0.1], [d1 + 0.2, 1.95, 0.115]);
  add("copper", [[d0 - 0.4, 0.6 + DOOR.h + 0.4, 0], [DOOR.w + 0.8, 0, 0], [0, -0.25, 0.8], [0, 0.04, 0]]);
  // the iron balcony at the first floor's corner bay on the south front: a stone slab, balusters and a rail
  const bx = BAYS.south[4];
  box("sandstone", [bx - 1.1, 4.25, 0], [bx + 1.1, 4.4, 0.95]);
  for (let i = 0; i <= 14; i++) box("iron", [bx - 1.05 + i * 0.15, 4.4, 0.86], [bx - 1.03 + i * 0.15, 5.35, 0.88]);
  box("iron", [bx - 1.08, 5.35, 0.84], [bx + 1.08, 5.4, 0.9]);
  // the downpipe down the south front's west end, from the gutter to the walk
  box("zinc", [0.25, 0.05, 0.62], [0.35, TOP + 0.45, 0.72]);
  box("zinc", [0.25, 0.05, 0.0], [0.35, 0.15, 0.72]);
  // an enamel street plate on each face of the corner, 2.6-2.9 m up
  box("enamel", [SOUTH.x1 - 0.9, 2.6, 0], [SOUTH.x1 - 0.2, 2.9, 0.012]);
  box("enamel", [SOUTH.x1, 2.6, -0.9], [SOUTH.x1 + 0.012, 2.9, -0.2]);
  // the cellar hatch: iron plates in the walk by the east front
  box("iron", [SOUTH.x1 + 0.3, -0.01, -6.5], [SOUTH.x1 + 1.1, 0.03, -5.3]);
  // the slate roof: a slope up from each front's cornice to a ridge 3 m higher, meeting on a hip over the corner,
  // and a felt flat behind them
  const [r, d, e, ex, ez] = [3.0, 3.2, TOP + 0.5, SOUTH.x1 + 0.1, EAST.z1 + 0.1];
  slab("slate", [[SOUTH.x0, e, ez], [ex, e, ez], [ex - d, e + r, ez - d], [SOUTH.x0, e + r, ez - d]], 0.06);
  slab("slate", [[ex, e, EAST.z0], [ex - d, e + r, EAST.z0], [ex - d, e + r, ez - d], [ex, e, ez]], 0.06);
  box("felt", [SOUTH.x0, e + r, EAST.z0], [ex - d, e + r + 0.05, ez - d]);
  // the house's own back and its neighbour: a brick firewall on the west, standing above the roof
  box("brick", [SOUTH.x0 - 0.4, 0, EAST.z0], [SOUTH.x0, TOP + 4.2, -T]);
  box("plaster", [SOUTH.x0, 0, EAST.z0 - 0.1], [SOUTH.x1, TOP + 0.4, EAST.z0]);
  box("plaster", [SOUTH.x0, 0, EAST.z0], [SOUTH.x1 - T, TOP + 0.4, -T]);
  // the walk round the corner, a granite kerb and the road 15 cm below it
  box("ground", [-6, -0.2, -16], [SOUTH.x1 + 3.5, 0, 3.5]);
  box("granite", [-6, -0.2, 3.5], [SOUTH.x1 + 3.8, 0, 3.8]);
  box("granite", [SOUTH.x1 + 3.5, -0.2, -16], [SOUTH.x1 + 3.8, 0, 3.5]);
  box("ground", [-6, -0.5, 3.8], [SOUTH.x1 + 12, -0.15, 14]);
  box("ground", [SOUTH.x1 + 3.8, -0.5, -16], [SOUTH.x1 + 12, -0.15, 3.8]);
}

/** Each class's solids (after `houseGlb` or `solids` builds them). */
export function solids(): Map<HouseClass, Solid[]> {
  build();
  return parts;
}

/** The corner house as a .glb: one primitive and one material per class, each solid's six faces with flat normals. */
export function houseGlb(): Uint8Array {
  build();
  const pos: number[] = [], nrm: number[] = [], idx: number[] = [];
  const prims: { first: number; count: number; mat: number }[] = [];
  const mats: unknown[] = [];
  const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const norm = (a: V3): V3 => { const l = Math.hypot(...a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
  for (const c of CLASSES) {
    const first = idx.length;
    for (const s of parts.get(c)!) {
      const at = (i: number, j: number, k: number): V3 => s[i + 2 * j + 4 * k];
      const mid: V3 = [0, 1, 2].map((a) => s.reduce((t, p) => t + p[a], 0) / 8) as V3;
      const quads: V3[][] = [
        [at(0, 0, 0), at(0, 1, 0), at(0, 1, 1), at(0, 0, 1)], [at(1, 0, 0), at(1, 0, 1), at(1, 1, 1), at(1, 1, 0)],
        [at(0, 0, 0), at(0, 0, 1), at(1, 0, 1), at(1, 0, 0)], [at(0, 1, 0), at(1, 1, 0), at(1, 1, 1), at(0, 1, 1)],
        [at(0, 0, 0), at(1, 0, 0), at(1, 1, 0), at(0, 1, 0)], [at(0, 0, 1), at(0, 1, 1), at(1, 1, 1), at(1, 0, 1)],
      ];
      for (let q of quads) {
        let n = norm(cross(sub(q[1], q[0]), sub(q[2], q[0])));
        const centre: V3 = [(q[0][0] + q[2][0]) / 2, (q[0][1] + q[2][1]) / 2, (q[0][2] + q[2][2]) / 2];
        const away = sub(centre, mid);
        if (n[0] * away[0] + n[1] * away[1] + n[2] * away[2] < 0) { q = [q[0], q[3], q[2], q[1]]; n = [-n[0], -n[1], -n[2]]; }
        const b = pos.length / 3;
        for (const p of q) { pos.push(...p); nrm.push(...n); }
        idx.push(b, b + 1, b + 2, b, b + 2, b + 3);
      }
    }
    if (idx.length > first) {
      prims.push({ first, count: idx.length - first, mat: mats.length });
      const l = LOOK[c];
      mats.push({ name: c, pbrMetallicRoughness: { baseColorFactor: [...l.color, 1], roughnessFactor: l.rough, metallicFactor: l.metal } });
    }
  }
  const f32 = new Float32Array([...pos, ...nrm]);
  const u32 = new Uint32Array(idx);
  const bin = new Uint8Array(f32.byteLength + u32.byteLength);
  bin.set(new Uint8Array(f32.buffer), 0);
  bin.set(new Uint8Array(u32.buffer), f32.byteLength);
  const n = pos.length / 3;
  const lo = [0, 1, 2].map((k) => Math.min(...pos.filter((_, i) => i % 3 === k)));
  const hi = [0, 1, 2].map((k) => Math.max(...pos.filter((_, i) => i % 3 === k)));
  const json = {
    asset: { version: "2.0", generator: "raea examples/materials/wear/house.ts" }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0, name: "corner-house" }],
    meshes: [{ primitives: prims.map((p, i) => ({ attributes: { POSITION: 0, NORMAL: 1 }, indices: 2 + i, material: p.mat })) }],
    materials: mats,
    buffers: [{ byteLength: bin.byteLength }],
    bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: n * 12 }, { buffer: 0, byteOffset: n * 12, byteLength: n * 12 }, { buffer: 0, byteOffset: n * 24, byteLength: u32.byteLength }],
    accessors: [
      { bufferView: 0, componentType: 5126, count: n, type: "VEC3", min: lo, max: hi },
      { bufferView: 1, componentType: 5126, count: n, type: "VEC3" },
      ...prims.map((p) => ({ bufferView: 2, byteOffset: p.first * 4, componentType: 5125, count: p.count, type: "SCALAR" })),
    ],
  };
  const pad = (b: Uint8Array, fill: number) => { const out = new Uint8Array(Math.ceil(b.length / 4) * 4).fill(fill); out.set(b); return out; };
  const j = pad(new TextEncoder().encode(JSON.stringify(json)), 0x20);
  const bb = pad(bin, 0);
  const glb = new Uint8Array(12 + 8 + j.length + 8 + bb.length);
  const dv = new DataView(glb.buffer);
  dv.setUint32(0, 0x46546c67, true); dv.setUint32(4, 2, true); dv.setUint32(8, glb.length, true);
  dv.setUint32(12, j.length, true); dv.setUint32(16, 0x4e4f534a, true); glb.set(j, 20);
  dv.setUint32(20 + j.length, bb.length, true); dv.setUint32(24 + j.length, 0x004e4942, true); glb.set(bb, 28 + j.length);
  return glb;
}
