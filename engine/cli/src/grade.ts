// A headless body's skin graded to the head that joins it (docs/character.feature CH20): the body's skin takes the
// head's neck colour, and just under the collar each vertex takes the colour the head's own skin has above it there
// (head.json's collar.ring_colours_srgb, one for each vertex of the ring, evened round the ring), easing into the
// neck colour over `fade` metres down. The colours are written into body.glb as vertex colours (COLOR_0, which
// multiplies the material's colour), so the body needs no picture and the join needs no weld. Measured on the first
// head (compositions/seam_check.tsx in the sheet3d project): the step across the collar at the front went from 8
// levels of 255 to under 1.
import { readFileSync, writeFileSync } from "node:fs";

type Vec3 = [number, number, number];
interface HeadJson { neck_colour_srgb?: number[]; collar_ring?: { positions?: Vec3[] }; collar?: { ring_colours_srgb?: number[][] } }
interface Gltf {
  accessors: { bufferView: number; byteOffset?: number; componentType: number; count: number; type: string }[];
  bufferViews: { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number; target?: number }[];
  buffers: { byteLength: number }[];
  materials?: { name?: string; pbrMetallicRoughness?: { baseColorFactor?: number[] } }[];
  meshes: { primitives: { attributes: Record<string, number>; material?: number }[] }[];
}

const linear = (c: number[]): Vec3 => [c[0] ** 2.2, c[1] ** 2.2, c[2] ** 2.2];
const near = (a: number, b: number) => Math.abs(((a - b + 3 * Math.PI) % (2 * Math.PI)) - Math.PI);

/** body.glb at `glb` graded to the head whose head.json is at `head`, in place; what was done, in a line */
export function gradeToHead(glb: string, head: string, fade = 0.08): string {
  const h = JSON.parse(readFileSync(head, "utf8")) as HeadJson;
  const ring = h.collar_ring?.positions, cols = h.collar?.ring_colours_srgb, tone = h.neck_colour_srgb;
  if (!ring?.length || !cols || cols.length !== ring.length || !tone) throw new Error(`${head}: no collar ring with its colours and no neck colour: the head was not joined to a body's collar`);
  // (bytes read through a DataView: the native raea's readFileSync gives a plain Uint8Array, not Node's Buffer)
  const b = new Uint8Array(readFileSync(glb)), dv = new DataView(b.buffer, b.byteOffset, b.byteLength), n = dv.getUint32(12, true);
  const g = JSON.parse(new TextDecoder().decode(b.subarray(20, 20 + n))) as Gltf;
  const at0 = 20 + n + 8, len0 = dv.getUint32(20 + n, true), f32 = (o: number) => dv.getFloat32(at0 + o, true);
  const mid = ring.reduce((s, p) => [s[0] + p[0] / ring.length, s[1] + p[1] / ring.length, s[2] + p[2] / ring.length], [0, 0, 0]);
  // the ring's colours by their angle round the neck, each the mean of itself and two neighbours on each side (one
  // texel's colour a vertex, carried down the neck, drew streaks)
  const lin = cols.map(linear), k = ring.length;
  const even = lin.map((_, i) => [0, 1, 2].map((c) => [-2, -1, 0, 1, 2].reduce((s, d) => s + lin[(i + d + k) % k][c], 0) / 5) as Vec3);
  const angle = ring.map((p) => Math.atan2(p[0] - mid[0], p[2] - mid[2]));
  const t = linear(tone);
  const added: Uint8Array[] = [];
  let at = len0 + ((4 - (len0 % 4)) % 4), graded = 0;
  for (const mesh of g.meshes) for (const p of mesh.primitives) {
    if (g.materials?.[p.material ?? -1]?.name !== "skin" || p.attributes.POSITION === undefined) continue;
    const a = g.accessors[p.attributes.POSITION], v = g.bufferViews[a.bufferView], from = (v.byteOffset ?? 0) + (a.byteOffset ?? 0), step = v.byteStride ?? 12;
    const out = new Float32Array(a.count * 4);
    for (let i = 0; i < a.count; i++) {
      const x = f32(from + i * step), y = f32(from + i * step + 4), z = f32(from + i * step + 8);
      const u = Math.min(1, Math.max(0, mid[1] - y) / fade), w = u * u * (3 - 2 * u), ang = Math.atan2(x - mid[0], z - mid[2]);
      let best = 0;
      for (let r = 1; r < k; r++) if (near(angle[r], ang) < near(angle[best], ang)) best = r;
      for (let c = 0; c < 3; c++) out[i * 4 + c] = (even[best][c] * (1 - w) + t[c] * w) / t[c];
      out[i * 4 + 3] = 1;
    }
    g.bufferViews.push({ buffer: 0, byteOffset: at, byteLength: out.byteLength, target: 34962 });
    g.accessors.push({ bufferView: g.bufferViews.length - 1, componentType: 5126, count: a.count, type: "VEC4" });
    p.attributes.COLOR_0 = g.accessors.length - 1;
    const m = g.materials![p.material!];
    m.pbrMetallicRoughness = { ...m.pbrMetallicRoughness, baseColorFactor: [...t, 1] };
    added.push(new Uint8Array(out.buffer));
    at += out.byteLength;
    graded += a.count;
  }
  if (!graded) throw new Error(`${glb}: no mesh with a material named "skin" to grade`);
  g.buffers[0].byteLength = at;
  const text = new TextEncoder().encode(JSON.stringify(g)), jlen = text.length + ((4 - (text.length % 4)) % 4);
  // the file again: its header, the JSON padded with spaces to four bytes, the buffer with the colours after it
  const file = new Uint8Array(12 + 8 + jlen + 8 + at), w = new DataView(file.buffer);
  const tag = (o: number, t4: string) => [...t4].forEach((ch, i) => (file[o + i] = ch.charCodeAt(0)));
  tag(0, "glTF"); w.setUint32(4, 2, true); w.setUint32(8, file.length, true);
  w.setUint32(12, jlen, true); tag(16, "JSON"); file.fill(0x20, 20, 20 + jlen); file.set(text, 20);
  w.setUint32(20 + jlen, at, true); tag(24 + jlen, "BIN\0");
  file.set(b.subarray(at0, at0 + len0), 28 + jlen);
  let o = 28 + jlen + len0 + ((4 - (len0 % 4)) % 4);
  for (const c of added) { file.set(c, o); o += c.length; }
  writeFileSync(glb, file);
  return `the skin graded to the head at the collar: ${graded} vertices, the neck colour ${tone.map((c) => c.toFixed(3)).join(", ")}, eased over ${Math.round(fade * 100)} cm`;
}
