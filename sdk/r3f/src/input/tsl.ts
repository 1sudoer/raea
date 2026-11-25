// A TSL node material (three/webgpu: MeshBasicNodeMaterial with a colorNode, lettra's MSDF text, any library built on
// TSL) as raea's ThreeShaderMaterial: three compiles the node graph to WGSL with its own WGSLNodeBuilder, run headless
// against a stand-in WebGPURenderer (it is asked for features and a node library, never to draw), and the WGSL is
// rewritten to the engine's contract (systems/three-shader):
//   the vertex stage   fn three_vertex(v: ThreeVertex) -> ThreeVaryings
//   the fragment stage fn three_fragment(in: ThreeVaryings, front: bool) -> vec4<f32>   (linear: `output: "linear"`)
//   both declare       struct ThreeVaryings { @builtin(position) clip: vec4<f32>, @location(0) ... }
// three's uniform groups and textures become the prelude's u_NAME() accessors (254 vec4 slots of the 256: three-shader module.rs) and tex_NAME/samp_NAME;
// the camera's projection and view and the model's world matrix become three_projection(), three_view() and
// three_model(instance), so Bevy's own camera and transform place it (Bevy's projection is reverse-Z). Every other
// uniform takes the value three's own node updates give it this frame (time from R3F's clock, a light's colour), so a
// uniform the scene animates in useFrame moves with it. A uniformArray is packed element by element and read as
// u_NAME(i); a vertex attribute of the mesh's own is one of the contract's a0..a3. A storage buffer it reads
// (`.element(i)`, `.toAttribute()`) is the engine's shared buffer by its attribute's key (tsl-storage.ts, R3F22). On an
// InstancedMesh it is compiled for copies (R3F22): three built with no uniform room, so the copies' matrices and
// colours are attributes; the engine draws one mesh of the copies, instance_index the copy's number (`tsl_copy`, its
// own attribute a0), the matrices identity (the engine bakes them into the copies) and any other attribute a copy at a
// time the copy's own. A graph this can't carry (more than 4 textures or 4 attributes of the mesh's own, more than 2
// storage buffers, a render target) is refused with the reason, and the material is drawn flat as before.
import * as THREE from "three";
import { ModelNode, NodeFrame, WebGPURenderer } from "three/webgpu";
import type { Part, Props } from "./material";
import { onGpu } from "./targets";
import { flipsY, srcOf } from "./texture";
import { isLit, litBuilder, litStages } from "./tsl-lit";
import { MaterialStorage, declaration, elementType, isStorage, storageOfAttribute, type SharedBuffer, type StorageAttribute } from "./tsl-storage";
import { ATTRIBUTES, ATTRIBUTES_OWN, ELEMENT, FLIP, STORED, Unsupported, fit, flipSamples, indexed, params, slotsOf, structs, word } from "./tsl-wgsl";

/** what a node material is drawn for this frame: the object it's on, its scene and camera, R3F's clock */
export interface NodeAt {
  object: THREE.Object3D;
  scene: THREE.Scene;
  camera: THREE.Camera;
  /** R3F's clock, seconds (TSL's `time`) */
  time: number;
  /** seconds since the last frame (TSL's `deltaTime`) */
  delta?: number;
}

export interface ShaderUniform {
  name: string;
  type: string;
  value: number[];
  array?: number;
}
export interface ShaderTexture {
  name: string;
  src: string;
  colorSpace: "srgb" | "linear";
}
export interface ShaderSpec {
  vertex: string;
  fragment: string;
  uniforms: ShaderUniform[];
  textures: ShaderTexture[];
}

/** the engine's limits (systems/three-shader) */
export const SLOTS = 254;
export const TEXTURES = 4;

type AnyNode = { isNode?: boolean; name?: string; value?: unknown; uniformNode?: AnyNode; scope?: string };
interface NodeUniformLike { name: string; nodeUniform?: { node: AnyNode; value: unknown } }
interface BindingLike { name: string; isNodeUniformsGroup?: boolean; isSampledTexture?: boolean; isSampler?: boolean; isStorageBuffer?: boolean; isBuffer?: boolean; uniforms?: NodeUniformLike[]; texture?: THREE.Texture; isSampledCubeTexture?: boolean; isSampledTexture3D?: boolean }
interface Builder {
  scene: THREE.Scene;
  camera: THREE.Camera;
  material: THREE.Material;
  context: { material?: THREE.Material };
  lightsNode?: unknown;
  vertexShader: string;
  fragmentShader: string;
  updateNodes: AnyNode[];
  updateBeforeNodes: AnyNode[];
  updateAfterNodes: AnyNode[];
  build(): void;
  getBindings(): { name: string; bindings: BindingLike[] }[];
  /** each stage's uniforms, a storage buffer's among them (its WGSL name, type "storageBuffer", its node's attribute) */
  uniforms?: Record<string, { name: string; type: string; node?: { value?: unknown } }[]>;
  /** the attributes three made itself (an InstancedMesh's matrices and colours, a `.toAttribute()`), by WGSL name */
  bufferAttributes?: { name: string; type: string; node?: { value?: unknown; bufferOffset?: number } }[];
}

// ---- the stand-in renderer ----

let renderer: WebGPURenderer | undefined;

/** three's WebGPURenderer, never initialised: WGSLNodeBuilder reads its node library and asks it what the GPU can do */
export function standIn(fresh = false): WebGPURenderer {
  if (renderer && !fresh) return renderer;
  const noop = () => {};
  const canvas = { style: {}, width: 1, height: 1, addEventListener: noop, removeEventListener: noop, getContext: () => null };
  const r = new WebGPURenderer({ canvas: canvas as unknown as HTMLCanvasElement });
  const own = r as unknown as Record<string, unknown>;
  // a builder asks before it is built; nothing here runs on a GPU, and Bevy has what these name
  own._initialized = true;
  own.hasFeature = () => false;
  own.hasCompatibility = () => true;
  // what three's WebGPU backend reads from its device while it builds: the uniform buffer size that decides whether an
  // InstancedMesh's matrices are a uniform array or attributes (wgpu's defaults), and compute's workgroup limits
  (own.backend as { device?: unknown }).device = { limits: { maxUniformBufferBindingSize: 65536, maxStorageBufferBindingSize: 134217728, maxComputeWorkgroupsPerDimension: 65535, maxComputeInvocationsPerWorkgroup: 256, maxComputeWorkgroupSizeX: 256, maxComputeWorkgroupSizeY: 256, maxComputeWorkgroupSizeZ: 64, maxBindGroups: 4, maxSampledTexturesPerShaderStage: 16, maxStorageBuffersPerShaderStage: 8 }, features: new Set<string>() };
  // no shadow maps: a shadow in three's graph would be a depth texture raea doesn't fill (raea's own lights cast)
  r.shadowMap.enabled = false;
  if (fresh) return r;
  return (renderer = r);
}

// ---- compiling a graph ----

interface Compiled {
  key: string;
  builder: Builder;
  vertex: string;
  fragment: string;
  /** each packed uniform: its accessor name, type, slot count and how its value is read */
  uniforms: { name: string; type: string; array?: number; read: () => unknown; stored: "f32" | "u32" | "bool" }[];
  textures: { name: string; texture: THREE.Texture }[];
  /** the mesh's own attributes it reads, in order (the contract's a0..a3) */
  attributes: string[];
  /** the storage buffers it reads (tsl_s0, tsl_s1), and the spec's list of them, made once */
  storage: ({ name: string; key: string } & SharedBuffer)[];
  /** compiled for copies (an InstancedMesh): the attributes it reads a copy at a time, each one of its own */
  copies?: { name: string; attribute: THREE.BufferAttribute }[];
  lit: boolean; // a surface Bevy lights (tsl-lit.ts), not a colour
  frame: NodeFrame;
}

const compiled = new WeakMap<THREE.Material, Compiled>();

/** what a compile depends on beyond the material: the geometry's attributes (a graph reads the ones it has) */
function keyOf(m: THREE.Material, o: THREE.Object3D): string {
  const g = (o as THREE.Mesh).geometry as THREE.BufferGeometry | undefined;
  return `${m.version}|${g ? Object.keys(g.attributes).sort().join(",") : ""}|${copiesOf(o) ? "c" : ""}|${(o as THREE.InstancedMesh).instanceColor ? "k" : ""}`;
}

/** whether an object draws its material as copies (R3F22): an InstancedMesh */
export const copiesOf = (o: THREE.Object3D) => (o as THREE.InstancedMesh).isInstancedMesh === true;

function compile(m: THREE.Material, at: NodeAt): Compiled {
  const key = keyOf(m, at.object);
  const had = compiled.get(m);
  if (had && had.key === key) return had;
  // a renderer of its own: three caches a shared group's binding (render, frame) per renderer, with the uniforms of
  // whichever material built it first
  const r = standIn(true);
  // copies: no uniform room, so three reads an InstancedMesh's matrices and colours as attributes a copy at a time
  const copies = copiesOf(at.object);
  if (copies) ((r as unknown as { backend: { device: { limits: Record<string, number> } } }).backend.device.limits.maxUniformBufferBindingSize = 0);
  const b = (r as unknown as { backend: { createNodeBuilder(o: THREE.Object3D, r: WebGPURenderer): Builder } }).backend.createNodeBuilder(at.object, r);
  b.scene = at.scene;
  b.camera = at.camera;
  b.material = m;
  b.context.material = m;
  // a lit node material's surface is lit by Bevy's lights (tsl-lit.ts); a basic one's light map by three's light nodes
  const lit = isLit(m);
  if (lit) litBuilder(b, m);
  else if ((m as unknown as { lights?: boolean }).lights) b.lightsNode = (r as unknown as { lighting: { getNode(s: THREE.Scene, c: THREE.Camera): unknown } }).lighting.getNode(at.scene, at.camera);
  const errors: string[] = [];
  const err = console.error;
  console.error = (...a: unknown[]) => errors.push(a.map(String).join(" "));
  try {
    b.build();
  } finally {
    console.error = err;
  }
  if (errors.length) throw new Unsupported(`three couldn't build its graph: ${errors[0]}`);
  if (b.updateBeforeNodes.length || b.updateAfterNodes.length) throw new Unsupported("it renders the scene again first (a reflector, a render target, a pass): raea draws one scene");
  const c = rewrite(b, copies ? (at.object as THREE.InstancedMesh) : undefined);
  const out: Compiled = { key, builder: b, ...c, ...(lit ? litStages(c.vertex, c.fragment) : {}), lit, frame: new NodeFrame() };
  compiled.set(m, out);
  return out;
}

// ---- the WGSL, rewritten to the contract ----

function rewrite(b: Builder, copies?: THREE.InstancedMesh): Omit<Compiled, "key" | "builder" | "frame" | "lit"> {
  let vertex = b.vertexShader;
  let fragment = b.fragmentShader;
  const groups = b.getBindings();
  // the model's world matrix: the uniform behind ModelNode's world-matrix scope
  const modelWorld = new Set<AnyNode>();
  for (const n of b.updateNodes) if (n instanceof ModelNode && (n as AnyNode).scope === "worldMatrix" && (n as AnyNode).uniformNode) modelWorld.add((n as AnyNode).uniformNode!);
  // each group's members: where the engine's own value stands in, or the packed uniform's accessor
  const types = new Map([...structs(vertex), ...structs(fragment)]);
  const members = new Map<string, Map<string, { vertex: string; fragment: string }>>();
  const uniforms: Compiled["uniforms"] = [];
  const textures: Compiled["textures"] = [];
  const flipped = new Set<string>();
  /** a uniformArray's buffer: three's WGSL name for it, and the packed uniform it reads as */
  const buffers: { wgsl: string; name: string }[] = [];
  /** the storage buffers it reads, by the engine's names for them (tsl_s0, tsl_s1) */
  const storage = new MaterialStorage();
  let slots = 0;
  for (const g of groups) {
    for (const x of g.bindings) {
      if (x.isNodeUniformsGroup && x.uniforms) {
        const decl = [...vertex.matchAll(new RegExp(`var<uniform>\\s+${x.name}\\s*:\\s*(\\w+)`, "g")), ...fragment.matchAll(new RegExp(`var<uniform>\\s+${x.name}\\s*:\\s*(\\w+)`, "g"))][0];
        const fields = decl ? types.get(decl[1]) : undefined;
        const map = new Map<string, { vertex: string; fragment: string }>();
        for (const u of x.uniforms) {
          const node = u.nodeUniform?.node;
          const t = fields?.get(u.name);
          // a shared group (render, frame) carries every uniform built against this renderer: one this shader
          // neither declares nor reads is another material's
          if (!t && !new RegExp(`(?<![A-Za-z0-9_])${x.name}\\.${u.name}(?![A-Za-z0-9_])`).test(vertex + fragment)) continue;
          if (!node || !t) throw new Unsupported(`uniform ${x.name}.${u.name} has no declaration three wrote`);
          const named = node.name;
          if (named === "cameraProjectionMatrix") map.set(u.name, { vertex: "three_projection()", fragment: "three_projection()" });
          else if (named === "cameraViewMatrix") map.set(u.name, { vertex: "three_view()", fragment: "three_view()" });
          else if (named === "cameraPosition") map.set(u.name, { vertex: "three_camera_position()", fragment: "three_camera_position()" });
          else {
            if (/^array</.test(t)) throw new Unsupported(`uniform ${u.name} is an array in three's uniform group (${t})`);
            const type = t;
            const stored = STORED[type];
            if (!stored) throw new Unsupported(`uniform ${u.name} is a ${t}, which raea's uniforms don't carry`);
            const name = u.name;
            const access = type === "bool" ? `(u_${name}() != 0u)` : `u_${name}()`;
            uniforms.push({ name, type: stored, read: () => u.nodeUniform!.value, stored: type === "bool" ? "bool" : stored === "u32" || stored === "i32" ? "u32" : "f32" });
            slots += slotsOf(stored);
            // in the vertex stage the model's matrix is Bevy's (the instance's); the fragment reads three's
            map.set(u.name, modelWorld.has(node) ? { vertex: "three_model(tsl_instance)", fragment: access } : { vertex: access, fragment: access });
          }
        }
        members.set(x.name, map);
      } else if (x.isSampledTexture) {
        if (x.isSampledCubeTexture || x.isSampledTexture3D) throw new Unsupported(`texture ${x.name} is a cube or 3D texture (raea's shaders take 2D pictures)`);
        const t = x.texture;
        const src = srcOf(t);
        if (!t || !src) throw new Unsupported(`texture ${x.name} has no picture raea can load (a render target, a video, a canvas it can't read)`);
        textures.push({ name: x.name, texture: t });
        if (flipsY(t)) flipped.add(`tex_${x.name}`);
      } else if (x.isSampler) {
        // its texture's: samp_NAME
      } else if (x.isBuffer && (x as unknown as { nodeUniform?: { array?: unknown[] } }).nodeUniform?.array) {
        // a uniformArray (lettra's scramble pool): its elements packed one after another, read by index (u_NAME(i))
        const n = (x as unknown as { nodeUniform: { id: number; array: unknown[]; elementType: string } }).nodeUniform;
        const type = ELEMENT[n.elementType];
        if (!type) throw new Unsupported(`uniform array ${x.name} holds ${n.elementType}s, which raea's uniforms don't carry`);
        const name = `buf${n.id}`;
        uniforms.push({ name, type, array: n.array.length, read: () => n.array, stored: "f32" });
        slots += slotsOf(type, n.array.length);
        buffers.push({ wgsl: `NodeBuffer_${n.id}`, name });
      } else if (x.isStorageBuffer) {
        // a storage buffer (`.element(i)`): the engine's buffer shared by its attribute's key (R3F22), named below
        if (!isStorage((x as unknown as { attribute?: unknown }).attribute)) throw new Unsupported(`storage buffer ${x.name} has no storage attribute raea can share`);
      } else throw new Unsupported(`binding ${x.name} is a ${x.isBuffer ? "uniform array (uniformArray)" : (x as object).constructor.name}, which raea's shaders don't carry`);
    }
  }
  if (slots > SLOTS) throw new Unsupported(`its uniforms take ${slots} vec4 slots, more than raea's ${SLOTS}`);
  if (textures.length > TEXTURES) throw new Unsupported(`it samples ${textures.length} textures, more than raea's ${TEXTURES}`);
  // each storage buffer's WGSL name (three's NodeBuffer_N, a stage's uniform of type storageBuffer) as the engine's
  const storageNames = new Map<string, string>();
  for (const stage of ["vertex", "fragment"]) {
    for (const u of b.uniforms?.[stage] ?? []) if (u.type === "storageBuffer" && isStorage(u.node?.value)) storageNames.set(u.name, storage.name(u.node!.value as StorageAttribute));
  }
  /** the storage buffers each stage declares */
  const declared = { vertex: new Set<string>(), fragment: new Set<string>() };

  const common = (src: string, stage: "vertex" | "fragment") => {
    let s = src.replace(/^\s*diagnostic\s*\([^)]*\)\s*;\s*$/gm, "");
    // three's bindings go: the prelude binds the engine's
    const groupTypes = new Set<string>();
    s = s.replace(/@binding\(\s*\d+\s*\)\s*@group\(\s*\d+\s*\)\s*var(?:<[^>]*>)?\s+(\w+)\s*:\s*([^;]+);/g, (_all, name: string, type: string) => {
      if (members.has(name)) groupTypes.add(type.trim());
      // a storage buffer stays, read-only and unbound: the engine binds tsl_sN (three-shader storage.rs)
      const st = storageNames.get(name);
      if (st && !declared[stage].has(st)) {
        declared[stage].add(st);
        return `var<storage, read> ${st} : ${type.trim()};`;
      }
      return "";
    });
    for (const [from, to] of storageNames) s = word(s, from, to);
    for (const b of buffers) {
      s = s.replace(new RegExp(`struct\\s+${b.wgsl}Struct\\s*\\{[^}]*\\};?`, "g"), "");
      s = s.replace(new RegExp(`var<uniform>\\s+${b.wgsl}\\s*:[^;]*;`, "g"), "");
      s = indexed(s, `${b.wgsl}.value`, (i) => `u_${b.name}(u32( ${i} ))`);
    }
    for (const t of groupTypes) s = s.replace(new RegExp(`struct\\s+${t}\\s*\\{[^}]*\\};?`, "g"), "");
    for (const [group, map] of members) s = s.replace(new RegExp(`(?<![A-Za-z0-9_.])${group}\\.(\\w+)(?![A-Za-z0-9_])`, "g"), (all, m: string) => map.get(m)?.[stage] ?? all);
    for (const { name } of textures) {
      s = word(s, `${name}_sampler`, `samp_${name}`);
      s = word(s, name, `tex_${name}`);
    }
    s = flipSamples(s, flipped);
    s = word(s, "VaryingsStruct", "ThreeVaryings");
    return word(s, "builtinClipSpace", "clip");
  };

  // ---- the vertex stage ----
  vertex = common(vertex, "vertex");
  const ve = vertex.match(/@vertex\s*fn\s+main\s*\(([\s\S]*?)\)\s*->\s*ThreeVaryings\s*\{/);
  if (!ve) throw new Unsupported("three's vertex stage has no main returning its varyings");
  const vlets: string[] = ["\ttsl_instance = v.instance;"];
  const own: string[] = [];
  /** drawn as copies: the copy's number is its own attribute a0; the attributes it reads a copy at a time */
  const perCopy: { name: string; attribute: THREE.BufferAttribute }[] = [];
  if (copies) {
    own.push("tsl_copy");
    vlets.push("\tlet tsl_copy = u32(v.a0.x);");
  }
  /** the storage buffers the vertex stage reads that three didn't declare in it (`.toAttribute()`) */
  const extra: string[] = [];
  for (const p of params(ve[1])) {
    // three's instance: the copy, or 0 for a mesh drawn once (Bevy's instance index is its place in a batch)
    if (p.builtin === "instance_index") vlets.push(`\tlet ${p.name} = ${copies ? "tsl_copy" : "0u"};`);
    else if (p.location !== undefined) {
      const a = ATTRIBUTES[p.name];
      const made = b.bufferAttributes?.find((x) => x.name === p.name)?.node;
      const sa = storageOfAttribute(made);
      const mv = made?.value as { array?: unknown; isInstancedBufferAttribute?: boolean; isInstancedInterleavedBuffer?: boolean } | undefined;
      if (a) vlets.push(`\tlet ${p.name} = ${fit(`v.${a.field}`, a.type, p.type)};`);
      else if (sa) {
        // `.toAttribute()` of a storage buffer: the shared buffer read at the copy being drawn
        if (!sa.isStorageInstancedBufferAttribute) throw new Unsupported(`it reads ${p.name}, a storage buffer a vertex at a time (raea reads one a copy at a time)`);
        const name = storage.name(sa);
        if (!declared.vertex.has(name)) {
          declared.vertex.add(name);
          extra.push(declaration(name, sa));
        }
        vlets.push(`\tlet ${p.name} = ${fit(`${name}.value[${copies ? "tsl_copy" : "0u"}]`, elementType(sa), p.type)};`);
      } else if (copies && mv && mv.array === copies.instanceMatrix.array) {
        // a column of the copy's matrix: identity, as the engine bakes each copy's matrix into its vertices
        const col = Math.round((made?.bufferOffset ?? 0) / 4);
        vlets.push(`\tlet ${p.name} = vec4<f32>(${[0, 1, 2, 3].map((i) => (i === col ? "1.0" : "0.0")).join(", ")});`);
      } else if (copies && mv?.isInstancedBufferAttribute) {
        // instanceColor or another attribute a copy at a time: one of the copy's own, which the engine puts on each of
        // the copy's vertices
        const attr = mv as unknown as THREE.BufferAttribute;
        // (three wraps instanceColor and a geometry's own in attributes of its own over the same array)
        const name = attr.array === copies.instanceColor?.array ? "instanceColor" : (Object.entries(copies.geometry.attributes).find(([, x]) => (x as THREE.BufferAttribute).array === attr.array)?.[0] ?? `copy_${p.name}`);
        if (own.length === ATTRIBUTES_OWN) throw new Unsupported(`it reads more than ${ATTRIBUTES_OWN} vertex attributes of the mesh's own (${[...own, name].join(", ")})`);
        vlets.push(`\tlet ${p.name} = ${fit(`v.a${own.length}`, "vec4<f32>", p.type)};`);
        own.push(name);
        perCopy.push({ name, attribute: attr });
      } else {
        // an attribute three made itself, or one a copy at a time on a mesh not drawn as copies: the mesh raea draws
        // carries per-vertex attributes only (R3F16)
        const ga = (b as unknown as { geometry?: THREE.BufferGeometry }).geometry?.getAttribute(p.name) as (THREE.BufferAttribute & { isInstancedBufferAttribute?: boolean; isInterleavedBufferAttribute?: boolean; data?: { isInstancedInterleavedBuffer?: boolean } }) | undefined;
        if (!ga) throw new Unsupported(`it reads ${p.name}, an attribute three makes for itself`);
        if (ga.isInstancedBufferAttribute || ga.data?.isInstancedInterleavedBuffer) throw new Unsupported(`it reads ${p.name}, an attribute a copy at a time`);
        // one of the mesh's own (lettra's layoutX, glyphIndex, cellUv): a0..a3, four floats each
        if (own.length === ATTRIBUTES_OWN) throw new Unsupported(`it reads more than ${ATTRIBUTES_OWN} vertex attributes of the mesh's own (${[...own, p.name].join(", ")})`);
        vlets.push(`\tlet ${p.name} = ${fit(`v.a${own.length}`, "vec4<f32>", p.type)};`);
        own.push(p.name);
      }
    } else throw new Unsupported(`its vertex stage reads the builtin ${p.builtin}`);
  }
  vertex = vertex.replace(ve[0], `fn three_vertex(v: ThreeVertex) -> ThreeVaryings {\n${vlets.join("\n")}\n`);
  vertex = "var<private> tsl_instance : u32;\n" + extra.join("") + (flipped.size ? FLIP : "") + vertex;
  const varyings = vertex.match(/struct\s+ThreeVaryings\s*\{[^}]*\};?/)?.[0];
  if (!varyings) throw new Unsupported("three's vertex stage declares no varyings");

  // ---- the fragment stage ----
  fragment = common(fragment, "fragment");
  const fe = fragment.match(/@fragment\s*fn\s+main\s*\(([\s\S]*?)\)\s*->\s*(\w+)\s*\{/);
  if (!fe) throw new Unsupported("three's fragment stage has no main");
  const outs = structs(fragment).get(fe[2]);
  const colour = fragment.match(new RegExp(`struct\\s+${fe[2]}\\s*\\{[^}]*?@location\\(\\s*0\\s*\\)\\s*(\\w+)`))?.[1];
  if (!outs || !colour) throw new Unsupported("three's fragment stage writes no colour");
  const flets: string[] = [];
  for (const p of params(fe[1])) {
    if (p.location !== undefined) flets.push(`\tlet ${p.name} = in.${p.name};`);
    else if (p.builtin === "front_facing") flets.push(`\tlet ${p.name} = front;`);
    else if (p.builtin === "position") flets.push(`\tlet ${p.name} = in.clip;`);
    else throw new Unsupported(`its fragment stage reads the builtin ${p.builtin}`);
  }
  fragment = fragment.replace(fe[0], `fn three_fragment(in: ThreeVaryings, front: bool) -> vec4<f32> {\n${flets.join("\n")}\n`);
  fragment = fragment.replace(/return\s+(\w+)\s*;/g, (all, v: string) => (fragment.includes(`var<private> ${v} : ${fe[2]}`) ? `return ${v}.${colour};` : all));
  fragment = varyings + "\n" + (flipped.size ? FLIP : "") + fragment;

  return { vertex, fragment, uniforms, textures, attributes: own, storage: storage.spec(), ...(copies ? { copies: perCopy } : {}) };
}

// ---- the values, this frame ----

function flat(v: unknown, type: string): number[] {
  if (typeof v === "number") return [v];
  if (typeof v === "boolean") return [v ? 1 : 0];
  if (v instanceof THREE.Color) return type === "vec4<f32>" ? [v.r, v.g, v.b, 1] : [v.r, v.g, v.b];
  if (v instanceof THREE.Matrix4 || v instanceof THREE.Matrix3) return [...v.elements];
  if (v && typeof (v as { toArray?: unknown }).toArray === "function") return (v as THREE.Vector4).toArray();
  if (Array.isArray(v)) return v.flatMap((x) => flat(x, type));
  throw new Unsupported(`a uniform's value ${String(v)} isn't a number, vector, colour or matrix`);
}

/** the graph's uniforms as three's node updates leave them for this object, camera and time */
function values(c: Compiled, m: THREE.Material, at: NodeAt): ShaderUniform[] {
  const f = c.frame as unknown as Record<string, unknown> & { updateNode(n: unknown): void };
  f.renderer = standIn();
  f.scene = at.scene;
  f.camera = at.camera;
  f.object = at.object;
  f.material = m;
  f.time = at.time;
  f.deltaTime = at.delta ?? 0;
  (f as unknown as { frameId: number }).frameId++;
  (f as unknown as { renderId: number }).renderId++;
  for (const n of c.builder.updateNodes) f.updateNode(n);
  return c.uniforms.map((u) => {
    const value = flat(u.read(), u.type);
    return { name: u.name, type: u.type, value, ...(u.array ? { array: u.array } : {}) };
  });
}

const warned = new Set<string>();

/** a node material as raea's ThreeShaderMaterial, or undefined with the reason named once (then drawn flat) */
export function nodeMaterialOf(m: THREE.Material, at: NodeAt): Part | undefined {
  try {
    const c = compile(m, at);
    const vals = values(c, m, at);
    const props: Props = {
      // <ThreeShaderMaterial wgsl>: the contract's two stages and its uniforms' types; this frame's values apart
      wgsl: { vertex: c.vertex, fragment: c.fragment, uniforms: vals.map(({ name, type, array }) => ({ name, type, ...(array ? { array } : {}) })), ...(c.attributes.length ? { attributes: c.attributes } : {}), ...(c.storage.length ? { storage: c.storage } : {}), ...(c.lit ? { lit: true } : {}) },
      ...(c.lit && (m as { fog?: boolean }).fog === false ? { fog: false } : {}),
      uniforms: Object.fromEntries(vals.map((u) => [u.name, u.value])),
      textures: Object.fromEntries(c.textures.map(({ name, texture }) => [name, { src: srcOf(texture)!, colorSpace: texture.colorSpace !== THREE.SRGBColorSpace && !onGpu(texture) ? "linear" : "srgb" }])),
      // three's node materials write linear colour (the renderer's output pass tone-maps and encodes), as raea's do
      output: "linear",
      transparent: m.transparent,
      depthWrite: m.depthWrite,
      side: m.side === THREE.BackSide ? "back" : m.side === THREE.DoubleSide ? "double" : "front",
      blending: m.blending === THREE.AdditiveBlending ? "add" : m.blending === THREE.MultiplyBlending ? "multiply" : m.blending === THREE.NoBlending ? "none" : "normal",
    };
    if (m.alphaTest > 0) props.alphaTest = m.alphaTest;
    return { type: "ThreeShaderMaterial", props, ...(c.copies ? { copies: c.copies } : {}) };
  } catch (e) {
    // a graph three itself can't build against the stand-in (what it asks of a GPU raea hasn't answered) is named and drawn
    // flat, as a refused one is: never the whole render refused for one material (docs/r3f.feature R3F16)
    if (!(e instanceof Error)) throw e;
    const k = `${m.type}:${e.message}`;
    if (!warned.has(k)) {
      warned.add(k);
      console.warn(`raea/r3f: ${m.type} "${m.name || m.uuid.slice(0, 8)}" can't run in raea's engine: ${e.message}; drawn flat in its colour`);
    }
    return undefined;
  }
}

/** a node material with no node of its own (a MeshStandardNodeMaterial as three's MeshStandardMaterial): the classic
 *  material's type, which raea draws natively, lit by its own lights */
export function classicOf(m: THREE.Material): string | undefined {
  const own = m as unknown as Record<string, unknown>;
  const custom = Object.keys(own).some((k) => k.endsWith("Node") && own[k] != null && (own[k] as AnyNode).isNode);
  if (custom) return undefined;
  const t = m.type.replace("NodeMaterial", "Material");
  return t !== m.type ? t : undefined;
}
