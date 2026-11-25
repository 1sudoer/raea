// TSL compute (docs/r3f.feature R3F22): a ComputeNode (`Fn(...)().compute(count)`) an app runs with gl.compute or
// computeAsync is three's own WGSL, which its WGSLNodeBuilder writes against the stand-in renderer (tsl.ts), run by the
// engine as a <Compute> kernel (systems/compute three.rs). Its storage buffers are the engine's shared buffers by
// their attributes' keys (tsl-storage.ts), so the node materials reading them see what it wrote; its uniform groups
// (`object`, `render`) are packed by the engine from each dispatch's values, read as three's node updates leave them
// when it was dispatched (a uniform set in useFrame before gl.compute is that frame's).
//
// The renderer's dispatches are a list (Dispatches, on gl.__raea.computes) the scene's walk empties into one Compute
// part a frame: the kernels so far, their shared buffers, and that frame's runs in the order dispatched. The engine runs
// each frame's runs once, in order, before the cameras draw; a composition with a compute steps every frame from 0
// (runtime/plugins steps()), so a still at frame F has run every frame's. `epoch` is this list's session (a seek back
// makes a new stand-in renderer), `fresh` its first part: the engine resets the buffers to their first contents for it.
import type * as THREE from "three";
import { NodeFrame, type WebGPURenderer } from "three/webgpu";
import type { Part } from "./material";
import { standIn } from "./tsl";
import { isStorage, keyOf, sharedOf, type SharedBuffer, type StorageAttribute } from "./tsl-storage";

/** a ComputeNode as three makes one */
interface ComputeNodeLike {
  isComputeNode?: boolean;
  count: number | null;
  dispatchSize: number | number[] | null;
  workgroupSize: number[];
  version: number;
}

/** a kernel as the engine takes it: three's WGSL (its group 0 bindings as written) and each storage buffer's key */
export interface Kernel {
  wgsl: string;
  buffers: Record<string, string>;
}

/** one dispatch: its kernel, workgroups and uniform values ({group: {field: value}}) */
export type Run = [kernel: number, workgroups: [number, number, number], uniforms: Record<string, Record<string, number | number[]>>];

interface Built {
  version: number;
  id: number;
  kernel: Kernel;
  shared: Record<string, SharedBuffer>;
  updates: unknown[];
  frame: NodeFrame;
  /** each uniform group's fields and how their values are read */
  fields: { group: string; field: string; read: () => unknown }[];
}

type BuilderLike = {
  computeShader: string;
  updateNodes: unknown[];
  build(): void;
  getBindings(): { bindings: { name: string; isNodeUniformsGroup?: boolean; isStorageBuffer?: boolean; uniforms?: { name: string; nodeUniform?: { value: unknown } }[] }[] }[];
  uniforms?: Record<string, { name: string; type: string; node?: { value?: unknown } }[]>;
};

const built = new WeakMap<object, Built>();
let kernels = 0;

/** the workgroups a dispatch runs, as three's WebGPU backend works them out: a count over the workgroup size, past
 *  65,535 groups split into rows */
export function workgroupsOf(node: ComputeNodeLike, size: number | number[] | null | undefined): [number, number, number] {
  const d = size ?? node.dispatchSize ?? node.count;
  if (Array.isArray(d)) return [d[0] ?? 1, d[1] ?? 1, d[2] ?? 1];
  if (typeof d !== "number" || !(d >= 0)) throw new Error("a dispatch with no count or dispatch size (an indirect dispatch raea doesn't run)");
  const per = node.workgroupSize.reduce((a, b) => a * b, 1);
  const n = Math.ceil(d / per);
  return n > 65535 ? [65535, Math.ceil(n / 65535), 1] : [Math.max(1, n), 1, 1];
}

/** three's `@binding( N ) @group( 0 )` as the engine reads a declaration (`@group(0) @binding(N)`) */
const bindings = (src: string) =>
  src.replace(/@binding\(\s*(\d+)\s*\)\s*@group\(\s*(\d+)\s*\)/g, (_all, b: string, g: string) => {
    if (g !== "0") throw new Error(`three's compute binds group ${g} (raea's computes bind group 0)`);
    return `@group(0) @binding(${b})`;
  });

function build(node: ComputeNodeLike): Built {
  const had = built.get(node);
  if (had && had.version === node.version) return had;
  const r = standIn(true);
  const b = (r as unknown as { backend: { createNodeBuilder(n: unknown, r: WebGPURenderer): BuilderLike } }).backend.createNodeBuilder(node, r);
  const errors: string[] = [];
  const err = console.error;
  console.error = (...a: unknown[]) => errors.push(a.map(String).join(" "));
  try {
    b.build();
  } finally {
    console.error = err;
  }
  if (errors.length) throw new Error(`three couldn't build it: ${errors[0]}`);
  const buffers: Record<string, string> = {};
  const shared: Record<string, SharedBuffer> = {};
  for (const u of b.uniforms?.compute ?? []) {
    if (u.type !== "storageBuffer") continue;
    if (!isStorage(u.node?.value)) throw new Error(`storage buffer ${u.name} has no storage attribute raea can share`);
    const a = u.node!.value as StorageAttribute;
    buffers[u.name] = keyOf(a);
    shared[keyOf(a)] = sharedOf(a);
  }
  const fields: Built["fields"] = [];
  for (const g of b.getBindings()) {
    for (const x of g.bindings) {
      if (x.isNodeUniformsGroup) for (const u of x.uniforms ?? []) fields.push({ group: x.name, field: u.name, read: () => u.nodeUniform?.value });
      else if (!x.isStorageBuffer) throw new Error(`binding ${x.name} is a ${(x as object).constructor.name}, which raea's computes don't bind (a texture, a storage texture)`);
    }
  }
  const out: Built = { version: node.version, id: had?.id ?? kernels++, kernel: { wgsl: bindings(b.computeShader), buffers }, shared, updates: b.updateNodes, frame: new NodeFrame(), fields };
  built.set(node, out);
  return out;
}

/** a uniform's value as the engine packs it: a number or a list of numbers */
function plain(v: unknown): number | number[] {
  if (typeof v === "number") return v;
  if (typeof v === "boolean") return v ? 1 : 0;
  const o = v as { isColor?: boolean; r: number; g: number; b: number; isMatrix3?: boolean; isMatrix4?: boolean; toArray?: () => number[] };
  if (o?.isColor) return [o.r, o.g, o.b];
  if (o?.isMatrix3 || o?.isMatrix4) throw new Error("a matrix uniform (raea's computes pack numbers and vectors)");
  if (typeof o?.toArray === "function") return o.toArray();
  throw new Error(`a uniform's value ${String(v)} isn't a number or vector`);
}

/** its uniforms' values now, three's node updates run first (the count, a uniform's onObjectUpdate) */
function valuesOf(k: Built, renderer: unknown): Run[2] {
  const f = k.frame as unknown as Record<string, unknown> & { updateNode(n: unknown): void; frameId: number; renderId: number };
  f.renderer = renderer;
  f.time = performance.now() / 1000;
  f.frameId++;
  f.renderId++;
  for (const n of k.updates) f.updateNode(n);
  const out: Run[2] = {};
  for (const { group, field, read } of k.fields) (out[group] ??= {})[field] = plain(read());
  return out;
}

const warned = new Set<string>();
let epochs = 0;

/** A renderer's dispatches (gl.__raea.computes): compute and computeAsync record each, the walk empties them into one
 *  Compute part a frame (part). */
export class Dispatches {
  readonly epoch = epochs++;
  /** the runs since the last part, in the order dispatched */
  readonly runs: Run[] = [];
  private kernels = new Map<number, Kernel>();
  private shared = new Map<string, SharedBuffer>();
  private parts = 0;
  private last?: { time: number; part: Part };

  constructor(private readonly renderer: unknown) {}

  get length(): number {
    return this.runs.length;
  }

  /** gl.compute(node | nodes, dispatchSize?): each node's kernel and its values now; one raea can't run is named once */
  compute(nodes: unknown, size?: number | number[] | null): void {
    for (const node of Array.isArray(nodes) ? nodes : [nodes]) {
      try {
        const n = node as ComputeNodeLike;
        if (!n?.isComputeNode || typeof (n as { build?: unknown }).build !== "function") throw new Error("it isn't a ComputeNode");
        const k = build(n);
        this.kernels.set(k.id, k.kernel);
        for (const [key, b] of Object.entries(k.shared)) if (!this.shared.has(key)) this.shared.set(key, b);
        this.runs.push([k.id, workgroupsOf(n, size), valuesOf(k, this.renderer)]);
      } catch (e) {
        if (!(e instanceof Error)) throw e;
        if (!warned.has(e.message)) {
          warned.add(e.message);
          console.warn(`raea/r3f: gl.compute dispatches a compute raea can't run (${e.message}): the scene is drawn without it`);
        }
      }
    }
  }

  /** this frame's Compute part (none before the first dispatch): the runs since the last, emptied. Asked again at the
   *  same time (a frame converted twice), the same part. */
  part(time: number): Part | undefined {
    if (this.last && this.last.time === time) return this.last.part;
    if (!this.kernels.size) return undefined;
    const runs = this.runs.splice(0, this.runs.length);
    const part: Part = {
      type: "Compute",
      props: {
        kernels: Object.fromEntries(this.kernels),
        shared: Object.fromEntries(this.shared),
        runs: { epoch: this.epoch, seq: this.parts, fresh: this.parts === 0, runs },
      },
    };
    this.parts++;
    this.last = { time, part };
    return part;
  }
}

/** the dispatches of a renderer that has them (raea's stand-ins: renderer.ts) */
export function dispatchesOf(gl: THREE.WebGLRenderer | undefined): Dispatches | undefined {
  const d = (gl as unknown as { __raea?: { computes?: unknown } } | undefined)?.__raea?.computes;
  return d instanceof Dispatches ? d : undefined;
}
