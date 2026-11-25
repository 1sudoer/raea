// A headless body's skin graded to its head at the collar (engine/cli/src/grade.ts, docs/character.feature CH20b): a
// vertex at the collar takes the ring's colour above it, one a fade's length down keeps the neck colour.
import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gradeToHead } from "../src/grade";

test("a body's skin takes the head's colours at the collar and its neck colour below", () => {
  const dir = mkdtempSync(join(tmpdir(), "grade-"));
  // three vertices: at the collar in front, at the collar behind, and 10 cm under the collar in front
  const pos = Buffer.alloc(36);
  [[0, 1.5, 0.05], [0, 1.5, -0.05], [0, 1.4, 0.05]].flat().forEach((v, i) => pos.writeFloatLE(v, i * 4));
  const gltf = { asset: { version: "2.0" }, buffers: [{ byteLength: 36 }], bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: 36 }],
    accessors: [{ bufferView: 0, componentType: 5126, count: 3, type: "VEC3" }], materials: [{ name: "skin", pbrMetallicRoughness: { baseColorFactor: [1, 1, 1, 1] } }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, material: 0 }] }] };
  let json = Buffer.from(JSON.stringify(gltf));
  json = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 0x20)]);
  const h = Buffer.alloc(20), bh = Buffer.alloc(8);
  h.write("glTF", 0, "latin1"); h.writeUInt32LE(2, 4); h.writeUInt32LE(28 + json.length + 36, 8); h.writeUInt32LE(json.length, 12); h.write("JSON", 16, "latin1");
  bh.writeUInt32LE(36, 0); bh.write("BIN\0", 4, "latin1");
  writeFileSync(join(dir, "body.glb"), Buffer.concat([h, json, bh, pos]));
  // a ring of eight: the front half one colour, the back half another; the neck colour between
  const ring = Array.from({ length: 8 }, (_, k) => [0.05 * Math.sin((k / 8) * 2 * Math.PI), 1.5, 0.05 * Math.cos((k / 8) * 2 * Math.PI)]);
  const front = (k: number) => Math.cos((k / 8) * 2 * Math.PI) > 0.5, back = (k: number) => Math.cos((k / 8) * 2 * Math.PI) < -0.5;
  writeFileSync(join(dir, "head.json"), JSON.stringify({ neck_colour_srgb: [0.5, 0.5, 0.5], collar_ring: { positions: ring },
    collar: { ring_colours_srgb: ring.map((_, k) => (front(k) ? [0.6, 0.5, 0.5] : back(k) ? [0.4, 0.5, 0.5] : [0.5, 0.5, 0.5])) } }));
  expect(gradeToHead(join(dir, "body.glb"), join(dir, "head.json"))).toContain("3 vertices");
  const b = readFileSync(join(dir, "body.glb")), n = b.readUInt32LE(12), g = JSON.parse(b.subarray(20, 20 + n).toString("utf8"));
  const bin = b.subarray(28 + n), a = g.accessors[g.meshes[0].primitives[0].attributes.COLOR_0], at = g.bufferViews[a.bufferView].byteOffset;
  const red = (i: number) => bin.readFloatLE(at + i * 16);
  expect(a.type).toBe("VEC4");
  expect(red(0)).toBeGreaterThan(1.1);          // the front of the collar: redder than the neck colour
  expect(red(1)).toBeLessThan(0.9);             // the back: less red
  expect(red(2)).toBeCloseTo(1, 5);             // a fade's length down: the neck colour itself
  expect(g.materials[0].pbrMetallicRoughness.baseColorFactor[0]).toBeCloseTo(0.5 ** 2.2, 5);
  expect(() => gradeToHead(join(dir, "body.glb"), join(dir, "body.glb"))).toThrow();
});
