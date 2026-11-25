// A node material on an InstancedMesh (docs/r3f.feature R3F22): one mesh of the copies, which the engine repeats from
// the geometry (systems/three-shader storage.rs: a BufferMesh's `copies`), each copy's number its own attribute
// `tsl_copy` (three's instanceIndex, tsl.ts), its matrix baked into its vertices, and what the material reads a copy at
// a time (instanceColor, a range) on each of the copy's vertices. So 65,536 particles a compute places are one mesh
// and one draw, as three draws them. The copies are made once for each change of the geometry, count, matrices and
// colours; past what one mesh should hold, or for what this doesn't take, scene.ts draws a mesh a copy.
import * as THREE from "three";
import { arraysOf } from "./geometry";
import type { Part, Props } from "./material";
import { ownAttributes } from "./shader";

/** the vertices (triangle corners) one mesh of copies may hold */
export const MAX_CORNERS = 4_000_000;

interface Made {
  key: string;
  mesh: Props;
}
const made = new WeakMap<THREE.InstancedMesh, Made>();
const warned = new Set<string>();

/** whether every copy's matrix is the identity (a fresh InstancedMesh: a compute or positionNode places the copies) */
function identity(a: ArrayLike<number>, n: number): boolean {
  for (let i = 0; i < n; i++) for (let k = 0; k < 16; k++) if (a[i * 16 + k] !== (k % 5 === 0 ? 1 : 0)) return false;
  return true;
}

/** the BufferMesh of the copies: the geometry once, and what the engine repeats it with */
function copiesMesh(m: THREE.InstancedMesh, surface: Part, count: number): Props | undefined {
  const arrays = arraysOf(m.geometry, { flipV: false, colors: false });
  if (!arrays) return undefined;
  const corners = (arrays.indices?.length ?? arrays.positions.length / 3) * count;
  if (corners > MAX_CORNERS) {
    const k = `${m.uuid}`;
    if (!warned.has(k)) {
      warned.add(k);
      console.warn(`raea/r3f: an InstancedMesh of ${count} copies of ${arrays.positions.length / 3} vertices is more than one mesh holds (${MAX_CORNERS} corners): drawn a mesh a copy`);
    }
    return undefined;
  }
  const attributes: Record<string, { size: number; values: number[] }> = {};
  for (const { name, attribute } of surface.copies ?? []) {
    const values: number[] = new Array(count * attribute.itemSize);
    for (let i = 0; i < count; i++) for (let c = 0; c < attribute.itemSize; c++) values[i * attribute.itemSize + c] = attribute.array[i * attribute.itemSize + c];
    attributes[name] = { size: attribute.itemSize, values };
  }
  const copies: Props = { count, ...(Object.keys(attributes).length ? { attributes } : {}) };
  const mats = m.instanceMatrix.array;
  if (!identity(mats, count)) copies.matrices = Array.from(mats.subarray(0, count * 16));
  // the geometry's own attributes a three shader reads (aRandom, aOffset...), each vertex's, repeated with the copies
  const own = surface.props.wgsl && (surface.props.wgsl as { attributes?: string[] }).attributes?.some((a) => m.geometry.getAttribute(a)) ? ownAttributes(m.geometry) : undefined;
  return { ...arrays, ...(own ? { attributes: own } : {}), copies };
}

/** An InstancedMesh whose node material is compiled for copies (tsl.ts) as one mesh of its copies; undefined for
 *  anything else (several materials, a GLSL shader, more corners than a mesh holds), which scene.ts draws a mesh a copy. */
export function nodeInstancedParts(m: THREE.InstancedMesh, name: string, surface: Part, w: { shadows: boolean }, at: Props): Part[] | undefined {
  if (!surface.copies || Array.isArray(m.material)) return undefined;
  const count = Math.min(m.count, m.instanceMatrix.count);
  if (count < 1) return [];
  const g = m.geometry;
  const versions = Object.values(g.attributes).map((a) => (a as THREE.BufferAttribute).version).join(",");
  const key = [g.uuid, versions, g.index?.version ?? -1, count, m.instanceMatrix.version, m.instanceColor?.version ?? -1, ...surface.copies.map((c) => `${c.name}:${c.attribute.version}`)].join("|");
  let hit = made.get(m);
  if (hit?.key !== key) {
    const mesh = copiesMesh(m, surface, count);
    if (!mesh) return undefined;
    made.set(m, (hit = { key, mesh }));
  }
  // a copy moved on the GPU (a positionNode, a compute's positions) casts no shadow of where its vertices were
  const moved = (m.material as { positionNode?: unknown }).positionNode != null;
  const { copies: _per, ...own } = surface;
  return [{ type: "Mesh", props: { name, ...at, castShadow: w.shadows && m.castShadow && !moved, receiveShadow: m.receiveShadow }, children: [{ type: "BufferMesh", props: hit.mesh }, own] }];
}
