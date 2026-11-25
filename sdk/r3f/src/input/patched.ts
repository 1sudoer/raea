// A lit material patched by onBeforeCompile (docs/r3f.feature R3F24: drei's MeshWobbleMaterial, MeshDistortMaterial and
// MeshReflectorMaterial, an app's own vertex displacement) as raea's <ThreeShaderMaterial> of three's own program with
// the patch applied, as three compiles it: ShaderLib's standard or physical source handed to the material's
// onBeforeCompile with the material's defines and uniforms, once a program as three does (again when its version or
// customProgramCacheKey changes), its uniforms refreshed from the material each frame as three's renderer refreshes
// them, the defines three's WebGLProgram sets for its maps, sides and vertex colours in front.
//
// Its lighting is Bevy's, as a lit TSL material's is (R3F21): the fragment stage is cut where three's lights begin
// (<lights_physical_fragment>) and writes the surface it has made by then for raea's engine to light
// (raea-sys-three-shader's lit stage, glsl.rs): base (diffuseColor), normal (view space, w the roughness), emissive
// (w the metalness) and the point in world space. A patch that changes what comes after (the light, the output) is
// named once and left out.
import * as THREE from "three";
import type { Part } from "./material";
import { expandIncludes, shaderPart } from "./shader";

type Lit = THREE.MeshStandardMaterial;
type Program = THREE.WebGLProgramParametersWithUniforms & { shaderName: string; shaderType: string };

/** whether a material has an onBeforeCompile of its own (three's own is empty) */
export const isPatched = (m: THREE.Material): boolean => m.onBeforeCompile !== THREE.Material.prototype.onBeforeCompile;

/** whether raea draws a material as its patched program (a lit standard or physical one with a patch, or drawn without
 * the depth test, which Bevy's standard material can't be: docs/img2raea.feature IR5) */
export const drawsPatched = (m: THREE.Material): boolean => (m.type === "MeshStandardMaterial" || m.type === "MeshPhysicalMaterial") && (isPatched(m) || !m.depthTest);

const LIGHTS_AT = "#include <lights_physical_fragment>";
/** what follows the cut in three's own program: a patched program whose tail differs changed the light or output */
const TAIL = THREE.ShaderLib.physical.fragmentShader.slice(THREE.ShaderLib.physical.fragmentShader.indexOf(LIGHTS_AT));
/** the chunks only three's lighting reads, left out with it */
const DEAD = /^[ \t]*#include +<(lights_pars_begin|lights_physical_pars_fragment|envmap_common_pars_fragment|envmap_physical_pars_fragment|cube_uv_reflection_fragment|transmission_pars_fragment|shadowmap_pars_fragment|iridescence_fragment|aomap_pars_fragment|lightmap_pars_fragment)>/gm;
const OUTS = ["raeaBase", "raeaNormal", "raeaEmissive", "raeaWorld"];
const WRITE = [
  "\traeaBase = diffuseColor;",
  "\traeaNormal = vec4( normal, roughnessFactor );",
  "\traeaEmissive = vec4( totalEmissiveRadiance, metalnessFactor );",
  "\t// the point in world space: three's view matrix turned back (a camera's has no scale)",
  "\traeaWorld = vec4( transpose( mat3( viewMatrix ) ) * ( - vViewPosition - viewMatrix[ 3 ].xyz ), 1.0 );",
  "}",
  "",
].join("\n");

/** the maps three's program reads, each by its USE_ and _UV defines, its uniform and its Transform */
const MAPS = ["map", "alphaMap", "emissiveMap", "roughnessMap", "metalnessMap", "normalMap", "bumpMap", "displacementMap"] as const;
/** maps three only lights with, which Bevy's lighting doesn't take from a surface */
const UNREAD = ["aoMap", "lightMap"] as const;

const warned = new Set<string>();
function warnOnce(k: string, why: string) {
  if (warned.has(k)) return;
  warned.add(k);
  console.warn(`raea/r3f: ${k} ${why}`);
}

/** a patched fragment stage cut at three's lights, writing its surface instead */
export function surfaceStage(fragment: string, what: string): string {
  const at = fragment.indexOf(LIGHTS_AT);
  if (at < 0) throw new Error(`raea/r3f: ${what}'s patch took out three's <lights_physical_fragment>, where raea lights its surface`);
  if (fragment.slice(at).trim() !== TAIL.trim()) warnOnce(`${what}'s patch past three's lighting`, "is left out: raea's engine lights its surface");
  const head = fragment.slice(0, at).replace(DEAD, "");
  const main = head.search(/\bvoid\s+main\s*\(/);
  return `${head.slice(0, main)}${OUTS.map((o, i) => `layout(location = ${i}) out highp vec4 ${o};`).join("\n")}\n${head.slice(main)}${WRITE}`;
}

/** the defines three's WebGLProgram puts in front for this material (its maps, sides, vertex colours, alpha test) */
function definesOf(m: Lit): Record<string, string> {
  const d: Record<string, string> = {};
  for (const k of MAPS) {
    if (!(m as unknown as Record<string, THREE.Texture | null>)[k]) continue;
    const K = k.toUpperCase();
    d[`USE_${K}`] = "";
    d[`${K}_UV`] = "uv";
  }
  if (m.normalMap) d.USE_NORMALMAP_TANGENTSPACE = "";
  if (m.flatShading) d.FLAT_SHADED = "";
  if (m.side === THREE.DoubleSide) d.DOUBLE_SIDED = "";
  if (m.side === THREE.BackSide) d.FLIP_SIDED = "";
  if (m.vertexColors) d.USE_COLOR = "";
  if (m.alphaTest > 0) d.USE_ALPHATEST = "";
  for (const k of UNREAD) if ((m as unknown as Record<string, unknown>)[k]) warnOnce(`a patched ${m.type}'s ${k}`, "is left out (Bevy lights the surface)");
  return d;
}

/** the material's values in its program's uniforms, as three's renderer sets them each frame (refreshUniformsCommon,
 *  refreshUniformsStandard) */
function refresh(u: Record<string, THREE.IUniform>, m: Lit) {
  (u.diffuse.value as THREE.Color).copy(m.color);
  u.opacity.value = m.opacity;
  (u.emissive.value as THREE.Color).copy(m.emissive).multiplyScalar(m.emissiveIntensity);
  u.roughness.value = m.roughness;
  u.metalness.value = m.metalness;
  if (m.alphaTest > 0) u.alphaTest.value = m.alphaTest;
  const back = m.side === THREE.BackSide ? -1 : 1;
  (u.normalScale.value as THREE.Vector2).copy(m.normalScale).multiplyScalar(back);
  u.bumpScale.value = m.bumpScale * back;
  u.displacementScale.value = m.displacementScale;
  u.displacementBias.value = m.displacementBias;
  for (const k of MAPS) {
    const t = (m as unknown as Record<string, THREE.Texture | null>)[k];
    if (!t) continue;
    u[k].value = t;
    if (t.matrixAutoUpdate) t.updateMatrix();
    (u[`${k}Transform`].value as THREE.Matrix3).copy(t.matrix);
  }
}

/** each patched material's program, patched when three would compile it */
const programs = new WeakMap<THREE.Material, { key: string; p: Program }>();

function programOf(m: Lit, defines: Record<string, string>): Program {
  const key = `${m.version}|${m.customProgramCacheKey()}|${JSON.stringify(defines)}`;
  const had = programs.get(m);
  if (had?.key === key) return had.p;
  const lib = m.type === "MeshPhysicalMaterial" ? THREE.ShaderLib.physical : THREE.ShaderLib.standard;
  // three hands the patch the material's own defines (a patch may add to them, as drei's reflector adds USE_UV)
  const p = { shaderName: m.type, shaderType: m.type, uniforms: THREE.UniformsUtils.clone(lib.uniforms), vertexShader: lib.vertexShader, fragmentShader: lib.fragmentShader, defines: { ...(m.defines ?? {}) } } as unknown as Program;
  m.onBeforeCompile(p, undefined as unknown as THREE.WebGLRenderer);
  programs.set(m, { key, p });
  return p;
}

/** A lit material with an onBeforeCompile of its own as the <ThreeShaderMaterial> of its patched program */
export function patchedPart(m: Lit): Part {
  const defines = definesOf(m);
  const p = programOf(m, defines);
  refresh(p.uniforms, m);
  const fragment = surfaceStage(p.fragmentShader, m.type);
  // only the uniforms the program declares (three's lights' and environment's go with its lighting)
  const text = expandIncludes(`${p.vertexShader}\n${fragment}`);
  const uniforms = Object.fromEntries(Object.entries(p.uniforms).filter(([k]) => new RegExp(`\\b${k}\\b`).test(text)));
  // three's light counts, each 0 as its prefix sets them with no lights of its own (its shadow and light-map guards)
  const counts = Object.fromEntries([...new Set(text.match(/\bNUM_\w+/g))].filter((n) => !n.includes("CLIPPING")).map((n) => [n, "0"]));
  const sm = new THREE.ShaderMaterial({ uniforms, vertexShader: p.vertexShader, fragmentShader: fragment, defines: { ...counts, ...defines, ...p.defines } });
  Object.assign(sm, { transparent: m.transparent, opacity: m.opacity, side: m.side, depthWrite: m.depthWrite, depthTest: m.depthTest, blending: m.blending, alphaTest: m.alphaTest, vertexColors: m.vertexColors, toneMapped: m.toneMapped });
  const part = shaderPart(sm);
  (part.props.glsl as { lit?: boolean }).lit = true;
  // a lit surface's colour is scene light, tone-mapped by Bevy as raea's standard material's is
  part.props.output = "linear";
  if (m.fog === false) part.props.fog = false;
  return part;
}
