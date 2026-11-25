// A lit TSL node material (docs/r3f.feature R3F21): a MeshStandardNodeMaterial or MeshPhysicalNodeMaterial (or a
// Lambert, Phong or toon one) with nodes of its own, lit by Bevy's lights and shadows instead of three's. three's
// builder still writes the whole surface (colorNode, opacityNode, normalNode, roughnessNode, metalnessNode,
// emissiveNode, the vertex moved by positionNode), but with no lights of three's: the material's output is replaced
// (three's `context.getOutput`) by four values for the engine, which systems/three-shader's surface.rs hands to Bevy's
// PbrInput and lights as raea's StandardMaterial is lit:
//   base      the colour and opacity (three's DiffuseColor)
//   normal    xyz the normal in view space (three's normalView), w the roughness three lights with (Roughness)
//   emissive  xyz the emission (EmissiveColor), w the metalness (Metalness)
//   world     xyz the point in world space (positionWorld, after positionNode)
// The fragment stage then returns that surface (`fn three_surface(in, front) -> ThreeSurface`), and the varyings carry
// the instance (`three_instance`, set by the engine's vertex entry) so the fragment reads the mesh's shadow flags.
import type * as THREE from "three";
import * as TSLTyped from "three/tsl";

// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
const { outputStruct, diffuseColor, normalView, roughness, metalness, emissive, positionWorld, vec4, float } = TSLTyped as unknown as Record<string, any>;

/** the lit node materials whose surface Bevy lights (Basic, shadow and volume ones keep three's own output) */
const LIT = new Set(["MeshStandardNodeMaterial", "MeshPhysicalNodeMaterial", "MeshSSSNodeMaterial", "MeshLambertNodeMaterial", "MeshPhongNodeMaterial", "MeshToonNodeMaterial"]);

export interface LitBuilder {
  context: { getOutput?: (result: unknown) => unknown } & object;
}

/** whether a node material's graph is a surface Bevy lights: a lit kind, its fragment three's surface (no fragmentNode) */
export function isLit(m: THREE.Material): boolean {
  const own = m as unknown as { lights?: boolean; fragmentNode?: unknown; isMeshStandardNodeMaterial?: boolean };
  return own.lights === true && own.fragmentNode == null && (LIT.has(m.type) || own.isMeshStandardNodeMaterial === true);
}

/** the material's roughness and metalness where three's lighting model keeps none: Lambert is fully rough, Phong's
 *  shininess a GGX roughness (material.ts's roughnessOfShininess), neither metal */
function surfaceOf(m: THREE.Material): [unknown, unknown] {
  if ((m as unknown as { isMeshStandardNodeMaterial?: boolean }).isMeshStandardNodeMaterial) return [roughness, metalness];
  const shininess = (m as unknown as { shininess?: number }).shininess;
  const r = shininess === undefined ? 1 : Math.min(1, Math.sqrt(Math.sqrt(2 / (Math.max(0, shininess) + 2))));
  return [float(r), float(0)];
}

/** the builder set to write the surface (no lights of three's: the caller leaves its lightsNode unset) */
export function litBuilder(b: LitBuilder, m: THREE.Material): void {
  const [r, metal] = surfaceOf(m);
  b.context.getOutput = () => outputStruct(vec4(diffuseColor), vec4(normalView, r), vec4(emissive, metal), vec4(positionWorld, float(1)));
}

const FIELDS = ["base", "normal", "emissive", "world"];
const SURFACE = `struct ThreeSurface {\n${FIELDS.map((f) => `\t${f} : vec4<f32>,`).join("\n")}\n};`;

/** the rewritten stages (tsl.ts's contract, its fragment returning three's first output) made the lit contract: the
 *  fragment returns the whole surface, and the varyings carry the instance */
export function litStages(vertex: string, fragment: string): { vertex: string; fragment: string } {
  // three's output struct (OutputType, its members m0..m3) and the private variable the fragment fills
  const out = fragment.match(/var<private>\s+(\w+)\s*:\s*(\w+)\s*;/g)?.map((d) => d.match(/var<private>\s+(\w+)\s*:\s*(\w+)/)!).find(([, , t]) => new RegExp(`struct\\s+${t}\\s*\\{[^}]*@location\\(\\s*3\\s*\\)`).test(fragment));
  if (!out) throw new Error("three's lit fragment stage has no four outputs");
  const [, v, type] = out;
  let f = fragment.replace(new RegExp(`struct\\s+${type}\\s*\\{[^}]*\\};?`), SURFACE);
  f = f.replace(new RegExp(`var<private>\\s+${v}\\s*:\\s*${type}\\s*;`), `var<private> ${v} : ThreeSurface;`);
  FIELDS.forEach((name, i) => (f = f.replace(new RegExp(`(?<![A-Za-z0-9_])${v}\\.m${i}(?![A-Za-z0-9_])`, "g"), `${v}.${name}`)));
  f = f.replace(/fn three_fragment\(in: ThreeVaryings, front: bool\) -> vec4<f32> \{/, "fn three_surface(in: ThreeVaryings, front: bool) -> ThreeSurface {");
  f = f.replace(new RegExp(`return\\s+${v}\\.base\\s*;`, "g"), `return ${v};`);
  return { vertex: carryInstance(vertex), fragment: carryInstance(f) };
}

/** `three_instance` added to the varyings, at the location after three's last (flat: an index) */
function carryInstance(src: string): string {
  return src.replace(/struct\s+ThreeVaryings\s*\{([^}]*)\}/, (all, body: string) => {
    const at = Math.max(-1, ...[...body.matchAll(/@location\(\s*(\d+)\s*\)/g)].map((m) => Number(m[1]))) + 1;
    return `struct ThreeVaryings {\n\t@location( ${at} ) @interpolate( flat ) three_instance : u32,${body}}`;
  });
}
