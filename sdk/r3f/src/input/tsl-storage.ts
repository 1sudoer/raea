// TSL storage buffers (docs/r3f.feature R3F22): an `instancedArray`, `attributeArray` or `storage` attribute is one
// buffer the engine keeps from frame to frame, shared by key between the computes that write it (tsl-compute.ts) and
// the node materials that read it (tsl.ts): `.element(i)` in a material is a storage binding, `.toAttribute()` a
// vertex attribute three makes of it, which raea reads from the same buffer at the copy being drawn. The engine binds
// a material's buffers as `tsl_s0` and `tsl_s1` (systems/three-shader storage.rs) and a compute's under the names
// three gave them (systems/compute three.rs), each by its key.
import type * as THREE from "three";

/** a storage attribute as three makes one (StorageBufferAttribute, StorageInstancedBufferAttribute) */
export interface StorageAttribute {
  array: Float32Array | Uint32Array | Int32Array;
  itemSize: number;
  count: number;
  isStorageBufferAttribute?: boolean;
  isStorageInstancedBufferAttribute?: boolean;
}

/** a shared buffer as the engine makes it: its size in bytes and first contents, in WGSL's storage layout */
export interface SharedBuffer {
  size: number;
  f32?: number[];
  u32?: number[];
  i32?: number[];
}

/** the storage buffers a material reads (the engine's two) */
export const MATERIAL_STORAGE = 2;

const keys = new WeakMap<object, string>();
let next = 0;

/** a storage attribute's key: one buffer however many graphs read or write it */
export function keyOf(a: object): string {
  let k = keys.get(a);
  if (!k) keys.set(a, (k = `tsl:${next++}`));
  return k;
}

export const isStorage = (a: unknown): a is StorageAttribute => !!a && ((a as StorageAttribute).isStorageBufferAttribute === true || (a as StorageAttribute).isStorageInstancedBufferAttribute === true);

const scalar = (a: StorageAttribute) => (a.array instanceof Uint32Array ? "u32" : a.array instanceof Int32Array ? "i32" : "f32");

/** an element's WGSL type: f32, u32 or i32, or a vec2-4 of one */
export function elementType(a: StorageAttribute): string {
  const s = scalar(a);
  if (a.itemSize === 1) return s;
  if (a.itemSize >= 2 && a.itemSize <= 4) return `vec${a.itemSize}<${s}>`;
  throw new Error(`a storage buffer of ${a.itemSize} numbers an element (raea's take 1 to 4)`);
}

/** an element's words in storage: a vec3 takes four, as WGSL's `array<vec3<f32>>` strides 16 bytes */
const strideOf = (itemSize: number) => (itemSize === 3 ? 4 : itemSize);

/** the buffer the engine makes for a storage attribute: its contents only when any is set (three's are zeros at first) */
export function sharedOf(a: StorageAttribute): SharedBuffer {
  elementType(a);
  const stride = strideOf(a.itemSize);
  const size = Math.max(1, a.count) * stride * 4;
  if (!a.array.some((x) => x !== 0)) return { size };
  const words: number[] = new Array(Math.max(1, a.count) * stride).fill(0);
  for (let i = 0; i < a.count; i++) for (let c = 0; c < a.itemSize; c++) words[i * stride + c] = a.array[i * a.itemSize + c];
  return { size, [scalar(a)]: words };
}

/** the storage buffers a node material reads, in the order the engine binds them (tsl_s0, tsl_s1) */
export class MaterialStorage {
  readonly list: { key: string; attribute: StorageAttribute }[] = [];

  /** the WGSL name of a storage attribute's buffer, added if it's new */
  name(a: StorageAttribute): string {
    const key = keyOf(a);
    let i = this.list.findIndex((s) => s.key === key);
    if (i < 0) {
      if (this.list.length === MATERIAL_STORAGE) throw new Error(`it reads more than ${MATERIAL_STORAGE} storage buffers`);
      i = this.list.push({ key, attribute: a }) - 1;
    }
    return `tsl_s${i}`;
  }

  /** the spec's `storage`: each buffer's name, key, size and first contents */
  spec(): ({ name: string; key: string } & SharedBuffer)[] {
    return this.list.map((s, i) => ({ name: `tsl_s${i}`, key: s.key, ...sharedOf(s.attribute) }));
  }
}

/** a stage's own declaration of a storage buffer it reads but three didn't declare in it (a `.toAttribute()` read) */
export function declaration(name: string, a: StorageAttribute): string {
  return `struct ${name}Struct {\n\tvalue : array< ${elementType(a)} >\n};\nvar<storage, read> ${name} : ${name}Struct;\n`;
}

/** an attribute a copy at a time that three made of a storage attribute (`.toAttribute()`): its storage attribute */
export function storageOfAttribute(node: { value?: unknown } | undefined): StorageAttribute | undefined {
  const v = node?.value as (THREE.BufferAttribute & StorageAttribute) | undefined;
  return isStorage(v) ? v : undefined;
}
