// TSL compute and storage buffers (docs/r3f.feature R3F22): gl.compute's dispatches as one Compute part a frame (three's
// own WGSL a kernel, its storage buffer shared by key, the runs in order with their uniforms then), and a node
// material reading the buffer on an InstancedMesh as one mesh of copies (src/input/tsl-compute.ts, tsl-storage.ts,
// tsl-instances.ts, tsl.ts).
import { describe, expect, test } from "bun:test";
import { useFrame } from "@react-three/fiber";
import { useRef, type ReactNode } from "react";
import * as THREE from "three";
import { MeshBasicNodeMaterial } from "three/webgpu";
import * as TSLTyped from "three/tsl";
import { convert } from "../src/input/scene";
import { end, stepTo } from "../src/input/session";
import { workgroupsOf } from "../src/input/tsl-compute";
import type { Part } from "../src/parts";
import { TSL_INPUTS } from "./parity/tsl";

// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
const { Fn, color, float, instanceIndex, instancedArray, positionLocal, uniform, vec3 } = TSLTyped as unknown as Record<string, any>;

const flat = (parts: Part[]): Part[] => parts.flatMap((p) => [p, ...flat(p.children ?? [])]);
const COUNT = 4096;

/** a buffer of positions, set by one compute on the first frame and moved by another every frame */
function particles() {
  const pos = instancedArray(COUNT, "vec3");
  const speed = uniform(0.5);
  const init = Fn(() => {
    pos.element(instanceIndex).assign(vec3(float(instanceIndex).mul(0.001), 0, 0));
  })().compute(COUNT);
  const update = Fn(() => {
    const p = pos.element(instanceIndex);
    p.assign(p.add(vec3(0, speed, 0)));
  })().compute(COUNT);
  return { pos, speed, init, update };
}

function Dispatch({ p, children }: { p: ReturnType<typeof particles>; children: ReactNode }) {
  const started = useRef(false);
  useFrame(({ gl }) => {
    const g = gl as unknown as { compute(n: unknown): void };
    if (!started.current) g.compute(p.init);
    started.current = true;
    g.compute(p.update);
  });
  return <>{children}</>;
}

let n = 0;
function frames(scene: () => ReactNode, last: number) {
  const key = `c${n++}`;
  const out: Part[][] = [];
  for (let f = 0; f <= last; f++) {
    const s = stepTo(key, f, 24, 320, 180, {}, scene);
    const st = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
    out.push(flat(convert(st.scene, st.camera, st.gl, 180, f / 24, 1 / 24).parts));
  }
  end(key);
  return out;
}

type ComputeProps = { kernels: Record<string, { wgsl: string; buffers: Record<string, string> }>; shared: Record<string, { size: number }>; runs: { epoch: number; seq: number; fresh: boolean; runs: [number, number[], Record<string, Record<string, number | number[]>>][] } };

describe("TSL compute", () => {
  test("each dispatch is three's WGSL as a kernel, its buffer shared by key, run in order with its uniforms then", () => {
    const p = particles();
    const [f0, f1] = frames(() => <Dispatch p={p}><group /></Dispatch>, 1);
    const c0 = f0.find((x) => x.type === "Compute")?.props as ComputeProps;
    expect(c0).toBeDefined();
    expect(Object.keys(c0.kernels)).toHaveLength(2);
    const [init, update] = c0.runs.runs;
    expect(c0.runs.fresh).toBe(true);
    expect([init[1], update[1]]).toEqual([[64, 1, 1], [64, 1, 1]]);
    const k = c0.kernels[String(update[0])];
    expect(k.wgsl).toMatch(/@group\(0\) @binding\(0\)\s*var<storage, read_write> NodeBuffer_\d+/);
    expect(k.wgsl).toContain("@compute");
    // both kernels write the one buffer: 4,096 vec3s, 16 bytes each in storage
    const keys = Object.values(c0.kernels).flatMap((x) => Object.values(x.buffers));
    expect(new Set(keys).size).toBe(1);
    expect(c0.shared[keys[0]].size).toBe(COUNT * 16);
    // its uniforms as three's node updates left them: the speed and the count
    expect(Object.values(update[2].object)).toEqual(expect.arrayContaining([0.5, COUNT]));
    // the next frame: only its own dispatch, the same session, not fresh
    const c1 = f1.find((x) => x.type === "Compute")?.props as ComputeProps;
    expect(c1.runs.runs.map((r) => r[0])).toEqual([update[0]]);
    expect([c1.runs.epoch, c1.runs.seq, c1.runs.fresh]).toEqual([c0.runs.epoch, 1, false]);
  });

  test("three's dispatch size: a count over the workgroup, past 65,535 groups in rows; a size given as it is", () => {
    const node = { count: 1000, dispatchSize: null, workgroupSize: [64], version: 1 };
    expect(workgroupsOf(node, undefined)).toEqual([16, 1, 1]);
    expect(workgroupsOf({ ...node, count: 64 * 65535 * 2 }, undefined)).toEqual([65535, 2, 1]);
    expect(workgroupsOf(node, [4, 4])).toEqual([4, 4, 1]);
  });

  test("a node material reading the buffer (.toAttribute()) on an InstancedMesh is one mesh of copies, its storage the compute's", () => {
    const p = particles();
    const m = new MeshBasicNodeMaterial();
    m.positionNode = positionLocal.add(p.pos.toAttribute());
    m.colorNode = color(1, 0.5, 0.2);
    const geo = new THREE.PlaneGeometry(0.05, 0.05);
    const [f0] = frames(() => <Dispatch p={p}><instancedMesh args={[geo, m, COUNT]} /></Dispatch>, 0);
    const c0 = f0.find((x) => x.type === "Compute")!.props as ComputeProps;
    const key = Object.values(Object.values(c0.kernels)[0].buffers)[0];
    const mesh = f0.find((x) => x.type === "Mesh" && x.children?.[1]?.type === "ThreeShaderMaterial")!;
    expect(mesh).toBeDefined();
    const buf = mesh.children![0].props as { positions: number[]; copies: { count: number; matrices?: number[] } };
    // the geometry once (a quad), repeated by the engine; its copies all where the mesh is
    expect(buf.positions).toHaveLength(12);
    expect(buf.copies.count).toBe(COUNT);
    expect(buf.copies.matrices).toBeUndefined();
    const wgsl = mesh.children![1].props.wgsl as { vertex: string; attributes: string[]; storage: { name: string; key: string; size: number }[] };
    expect(wgsl.storage).toEqual([{ name: "tsl_s0", key, size: COUNT * 16 }]);
    expect(wgsl.attributes[0]).toBe("tsl_copy");
    expect(wgsl.vertex).toContain("let tsl_copy = u32(v.a0.x);");
    expect(wgsl.vertex).toMatch(/var<storage, read> tsl_s0 : tsl_s0Struct;/);
    expect(wgsl.vertex).toMatch(/= tsl_s0\.value\[tsl_copy\];/);
    // the copy's matrix is the identity: the engine bakes the copies' matrices into their vertices
    expect(wgsl.vertex).toContain("vec4<f32>(1.0, 0.0, 0.0, 0.0)");
    // moved on the GPU: no shadow of where its vertices were
    expect(mesh.props.castShadow).toBe(false);
  });

  test(".element(instanceIndex) in both stages reads the shared buffer read-only; instanceColor and the matrices go with the copies", () => {
    const p = particles();
    const m = new MeshBasicNodeMaterial();
    m.positionNode = positionLocal.mul(0.05).add(p.pos.element(instanceIndex));
    m.colorNode = color(1, 0.6, 0.2).mul(p.pos.element(instanceIndex).y.mul(0.5).add(0.5));
    const geo = new THREE.PlaneGeometry(1, 1);
    const im = new THREE.InstancedMesh(geo, m, 8);
    for (let i = 0; i < 8; i++) {
      im.setMatrixAt(i, new THREE.Matrix4().makeTranslation(i, 0, 0));
      im.setColorAt(i, new THREE.Color(i / 8, 0, 0));
    }
    const [f0] = frames(() => <Dispatch p={p}><primitive object={im} /></Dispatch>, 0);
    const mesh = f0.find((x) => x.type === "Mesh" && x.children?.[1]?.type === "ThreeShaderMaterial")!;
    const wgsl = mesh.children![1].props.wgsl as { vertex: string; fragment: string; attributes: string[]; storage: { name: string }[] };
    expect(wgsl.storage.map((s) => s.name)).toEqual(["tsl_s0"]);
    expect(wgsl.vertex).toMatch(/var<storage, read> tsl_s0 : NodeBuffer_\d+Struct;/);
    expect(wgsl.fragment).toMatch(/var<storage, read> tsl_s0 : NodeBuffer_\d+Struct;/);
    expect(wgsl.vertex).not.toMatch(/NodeBuffer_\d+\.value/);
    expect(wgsl.attributes).toEqual(["tsl_copy", "instanceColor"]);
    const copies = (mesh.children![0].props as { copies: { count: number; attributes: Record<string, { size: number; values: number[] }>; matrices: number[] } }).copies;
    expect(copies.attributes.instanceColor.size).toBe(3);
    expect(copies.attributes.instanceColor.values[3 * 4]).toBeCloseTo(0.5, 6);
    expect(copies.matrices).toHaveLength(8 * 16);
    expect(copies.matrices[16 * 3 + 12]).toBe(3);
  });

  test("the parity swirl: two buffers both computes write and the copies' material reads, a frame's update once", () => {
    const [f0, f1, f2] = frames(TSL_INPUTS["tsl-particles"].scene, 2);
    const c0 = f0.find((x) => x.type === "Compute")!.props as ComputeProps;
    expect(c0.runs.runs.map((r) => r[1])).toEqual([[1024, 1, 1], [1024, 1, 1]]);
    expect(Object.keys(c0.shared)).toHaveLength(2);
    const mesh = f0.find((x) => x.type === "Mesh" && x.children?.[1]?.type === "ThreeShaderMaterial")!;
    const m = mesh.children![1].props as { wgsl: { storage: { key: string }[]; vertex: string }; blending: string };
    expect(m.wgsl.storage.map((s) => s.key).sort()).toEqual(Object.keys(c0.shared).sort());
    expect((mesh.children![0].props as { copies: { count: number } }).copies.count).toBe(65536);
    for (const f of [f1, f2]) expect((f.find((x) => x.type === "Compute")!.props as ComputeProps).runs.runs).toHaveLength(1);
  });

  test("a mesh drawn once reads instance 0 (Bevy's instance index is its place in a batch)", () => {
    const p = particles();
    const m = new MeshBasicNodeMaterial();
    m.positionNode = positionLocal.add(p.pos.element(instanceIndex));
    const [f0] = frames(() => <Dispatch p={p}><mesh material={m}><planeGeometry /></mesh></Dispatch>, 0);
    const wgsl = f0.find((x) => x.type === "ThreeShaderMaterial")!.props.wgsl as { vertex: string };
    expect(wgsl.vertex).toMatch(/let instanceIndex = 0u;/);
  });
});
