// raea/wear (docs/wear.feature WR5): a model weathered by what weathers it. `wearBake` bakes a .glb's causes once
// (rain's reach, the sun, run-off and what it leaves, touch, height over the walk; raea-sys-weathering, kept in the
// composition's asset store), and `wearShader` gives the shader that draws them on each surface by its material class.
//
//   const baked = wearBake({ src: staticFile("house.glb"), classes: { facade: "plaster", rail: "iron", "*": "brick" } });
//   const wear = wearShader({ baked, age: { years: 60, soot: 0.8 }, weather: { day: 300, wet: 0.4 } });
//   <Model src={baked.file}>
//     <ExtendedMaterial base={<StandardMaterial />} extension={<ShaderExtension {...wear} />} />
//   </Model>
//
// The same spec goes on one glTF material (<StandardMaterial material="facade"/> as the base), a
// <MeshStandardMaterial shader={wear}>, or a <SurfaceShader {...wear}> over the whole scene. A surface without the
// bake's second UV set is drawn as it was. Crumbling (WR6) follows the same age: render lost to the brick, paint to
// the primer and the wood, enamel to the iron, cracks from the openings' corners, panes broken or boarded, slates
// gone; `wearDebris(baked, age)` is the debris that age has brought down, where the bake put it.
import { hasNative, native, type NativeCalls } from "@raea/app/native";
import { ABANDON_FROM, CAUSES } from "./causes";
import { debugHook, DEBUG_CAUSES, type WearCause } from "./debug";
import { HELPERS, RECIPES, SHARED, WEATHER } from "./recipes";

export { DEBUG_CAUSES, type WearCause } from "./debug";
export { ABANDON_FROM, CRACK_REACH, CRACK_WEIGHT, LOSS } from "./causes";
export { WEAR_CONDITIONS, WEATHER_RATES, weatherAt, type WearCondition, type WearSpell } from "./weather";

/** The material classes, in the bake's order (causes2's alpha is 16 x (the place + 1)), each with its colour in the
 *  debug view and what the causes do to it. */
export const WEAR_CLASSES = [
  { name: "plaster", colour: [0.95, 0.9, 0.75], marks: "washed clean and chalky; soot grey where sheltered, darkest under cornices; dark tears from sill ends; splash to 0.4 m and a damp tide at 0.6-1.2 m; by neglect blown to the brick behind at the damp foot, under sills, at knocked arrises and along cracks, and cracked from the openings' corners; chipped arrises low down; a grease band" },
  { name: "brick", colour: [0.75, 0.3, 0.2], marks: "mortar washed out; soot black where sheltered; efflorescence under ledges; a tide line, green on north faces; spalled faces; cracks stepping along the joints from the openings' corners" },
  { name: "sandstone", colour: [0.9, 0.75, 0.45], marks: "washed light; a black crust where sheltered; black and white streaks under ledges; rounded arrises" },
  { name: "granite", colour: [0.6, 0.6, 0.62], marks: "little: soot where sheltered" },
  { name: "wood", colour: [0.55, 0.35, 0.15], marks: "sun-faded and alligatored on the faces the sun reaches; peeling to the primer and the wood at bottom rails, and by neglect wherever the coat's loosened; rot at the foot; worn to the wood where held and kicked" },
  { name: "iron", colour: [0.25, 0.3, 0.4], marks: "paint flaking at joints and arrises; rust at the foot and on the stone below; bright where held" },
  { name: "zinc", colour: [0.7, 0.8, 0.85], marks: "a pale patina where rain-washed; white streaks below" },
  { name: "copper", colour: [0.3, 0.7, 0.55], marks: "verdigris where rain-washed, brown-black where sheltered; green streaks below" },
  { name: "slate", colour: [0.35, 0.35, 0.45], marks: "lichen where the sun misses; soot in the laps; slates gone by neglect, most along ridges, verges and eaves" },
  { name: "felt", colour: [0.15, 0.15, 0.15], marks: "greyed by the sun; washed where the rain runs" },
  { name: "glass", colour: [0.5, 0.85, 1.0], marks: "a grime film where sheltered, thickest at the bottom and corners; panes broken or boarded by neglect" },
  { name: "enamel", colour: [0.2, 0.3, 0.9], marks: "rust bled from its screws; chipped to black iron at holes and edges, and by neglect flaking in sheets to rusting iron" },
  { name: "brass", colour: [0.9, 0.7, 0.2], marks: "dull brown, more where rain-washed; bright where touched" },
  { name: "ground", colour: [0.4, 0.55, 0.3], marks: "dirt in corners and at the wall's foot; moss where the sun misses; polished where feet pass" },
] as const satisfies readonly { name: string; colour: [number, number, number]; marks: string }[];

export type WearClass = (typeof WEAR_CLASSES)[number]["name"];
export type WearBakeRequest = NativeCalls["wear.bake"]["args"];
export type WearBaked = NativeCalls["wear.bake"]["answer"];

const bakes = new Map<string, WearBaked>();

/** The bake of `req.src` under its env and classes: the model with TEXCOORD_1 and the cause maps, made once and kept
 *  in the composition's asset store (the same request comes back from it). Needs the raea engine. */
export function wearBake(req: WearBakeRequest): WearBaked {
  const key = JSON.stringify(req);
  let b = bakes.get(key);
  if (!b) {
    if (!hasNative()) throw new Error("wearBake: baking needs the raea engine (raea render)");
    b = native("wear.bake", req);
    if (!b || typeof b.file !== "string") throw new Error(`wearBake: the bake gave nothing for ${req.src}`);
    bakes.set(key, b);
  }
  return b;
}

/** What years of weather left: the same in every season. */
export interface WearAge {
  /** years since the last repair (0 is new; the marks come on over 20-30 years and then hold) */
  years?: number;
  /** how sooty the air was, 0..1 (a coal-burning city near 1) */
  soot?: number;
  /** how well kept, 0..1, around which each element (a front, a ledge) draws its own */
  upkeep?: number;
}

/** The day and its weather (docs/wear.feature WR9): drawn over the causes, never baked. The season's own share of
 *  damp, dust and leaves follows from `day` unless given. */
export interface WearWeather {
  /** the day of the year, 1-366 (172 by default: midsummer) */
  day?: number;
  /** rain on the surfaces now, 0..1 */
  wet?: number;
  /** snow lying, 0..1 */
  snow?: number;
  /** driven snow plastered on what the storm reaches, 0..1 */
  storm?: number;
  /** frost, 0..1 */
  ice?: number;
  /** spring's damp and algae, 0..1 */
  damp?: number;
  /** summer's dust, 0..1 */
  dust?: number;
  /** autumn's fallen leaves, 0..1 */
  leaves?: number;
  /** December's sun thinning snow between falls, 0..1 */
  melt?: number;
}

/** A thing the bake's rules put on the model (spots.json, docs/wear.feature WR3, WR6): what, who put it there, its
 *  middle, the face it's on and its size (across, up); debris also carries the loss potential of the plaster above it. */
export interface WearSpot {
  what: "bill" | "street plate" | "nameplate" | "debris";
  by: string;
  at: [number, number, number];
  n: [number, number, number];
  size: [number, number];
  loss?: number;
}

const spotsOf = new Map<string, WearSpot[]>();

/** The spots the bake put down (bills, street plates, nameplates, debris), read once from its spots.json. */
export function wearSpots(baked: Pick<WearBaked, "spots">): WearSpot[] {
  const path = baked?.spots;
  if (typeof path !== "string") throw new TypeError("wearSpots: baked is wearBake's answer, with its spots");
  let s = spotsOf.get(path);
  if (!s) {
    if (!hasNative()) throw new Error("wearSpots: reading the bake's spots needs the raea engine (raea render)");
    const got = native("asset.read", { path });
    // spots.json is ASCII (the rules' names and numbers), so atob's binary string is its text
    s = JSON.parse(atob(got.base64)) as WearSpot[];
    spotsOf.set(path, s);
  }
  return s;
}

/** How far neglect has let things go at an age (wearShader's own: the marks' age, less what upkeep keeps up). */
export function neglectOf(age: WearAge = {}): number {
  const years = age.years ?? 30;
  const upkeep = age.upkeep ?? 0.5;
  return Math.min(1, Math.max(0, (1 - Math.exp(-Math.max(years, 0) / 25)) * (1.15 - upkeep)));
}

/** How far a house is left empty at an age, 0..1 (wearShader's own): neglect past ABANDON_FROM. A lived-in house
 *  mends its panes and slates and sweeps its walk however poorly it is kept. */
export function abandonOf(age: WearAge = {}): number {
  const t = Math.min(1, Math.max(0, (neglectOf(age) - ABANDON_FROM) / (1 - ABANDON_FROM)));
  return t * t * (3 - 2 * t);
}

/** A heap shows where the plaster above it has come off and nobody sweeps: loss potential x abandonment past this. */
export const DEBRIS_AT = 0.3;

/** The debris an age has left lying (WR6): the bake's debris under plaster whose loss potential has let it go, on
 *  the walk of a house left empty. `spots` is wearSpots' answer, or wearBake's to read it. */
export function wearDebris(spots: readonly WearSpot[] | Pick<WearBaked, "spots">, age: WearAge = {}): WearSpot[] {
  const all = Array.isArray(spots) ? (spots as readonly WearSpot[]) : wearSpots(spots as Pick<WearBaked, "spots">);
  const a = abandonOf(age);
  return all.filter((s) => s.what === "debris" && (s.loss ?? 0) * a >= DEBRIS_AT);
}

export interface WearShaderOptions {
  /** wearBake's answer: its maps are the shader's textures, its density decides whether edges can chip */
  baked: Pick<WearBaked, "maps" | "density">;
  /** the classes to draw (the rest are drawn as they are); all of them by default */
  classes?: readonly WearClass[];
  age?: WearAge;
  weather?: WearWeather;
  /** the bake's up (its env.up), [0, 1, 0] by default */
  up?: [number, number, number];
  /** one cause in false colour in place of the weathered surface */
  debug?: WearCause;
}

// a bump over the year's days a..b, 1 at its middle (b may pass 366 into the next year)
const bump = (day: number, a: number, b: number) => {
  const d = day < a ? day + 366 : day;
  if (d < a || d > b) return 0;
  return Math.sin(((d - a) / (b - a)) * Math.PI);
};

/** The season's damp, dust, leaves and melt for a day, with what the weather gives over them. */
export function seasonOf(w: WearWeather = {}): { damp: number; dust: number; leaves: number; melt: number } {
  const day = w.day ?? 172;
  if (!(day >= 1 && day <= 366)) throw new RangeError(`wearShader: weather.day is 1-366, got ${day}`);
  const wet = w.wet ?? 0;
  return {
    damp: w.damp ?? 0.8 * bump(day, 50, 150),
    dust: w.dust ?? 0.8 * bump(day, 140, 260) * (1 - wet),
    leaves: w.leaves ?? bump(day, 270, 345),
    melt: w.melt ?? ((w.snow ?? 0) > 0 ? 0.5 : 0),
  };
}

const unit = (owner: string, v: number | undefined, d: number) => {
  const x = v ?? d;
  if (!(Number.isFinite(x) && x >= 0 && x <= 1)) throw new RangeError(`wearShader: ${owner} is 0..1, got ${v}`);
  return x;
};

/** The shader that draws the bake's causes as wear, by material class: `{ wesl, uniforms, textures }` for a
 *  <ShaderExtension> (raea/pbr), a <MeshStandardMaterial shader> or a <SurfaceShader>. */
export function wearShader(o: WearShaderOptions): { wesl: string; uniforms: Record<string, number[]>; textures: Record<string, { src: string; sampler: { filter: "linear"; mipmap: "none"; wrap: "clamp" } }> } {
  const maps = o?.baked?.maps;
  if (!maps || ["causes0", "causes1", "causes2", "causes3"].some((k) => typeof (maps as Record<string, unknown>)[k] !== "string")) throw new TypeError("wearShader: baked is wearBake's answer, with its four maps");
  const names = WEAR_CLASSES.map((c) => c.name) as readonly string[];
  const want = o.classes ?? names;
  for (const c of want) if (!names.includes(c)) throw new TypeError(`wearShader: no class ${JSON.stringify(c)} (${names.join(", ")})`);
  const years = o.age?.years ?? 30;
  if (!(Number.isFinite(years) && years >= 0)) throw new RangeError(`wearShader: age.years is 0 or more, got ${years}`);
  const w = o.weather ?? {};
  const s = seasonOf(w);
  const density = o.baked.density;
  const uniforms = {
    up: o.up ?? [0, 1, 0],
    age: [years, unit("age.soot", o.age?.soot, 0.5), unit("age.upkeep", o.age?.upkeep, 0.5), typeof density === "number" ? density : 4],
    wx: [unit("weather.wet", w.wet, 0), unit("weather.snow", w.snow, 0), unit("weather.storm", w.storm, 0), unit("weather.ice", w.ice, 0)],
    season: [unit("weather.damp", s.damp, 0), unit("weather.dust", s.dust, 0), unit("weather.leaves", s.leaves, 0), unit("weather.melt", s.melt, 0)],
  };
  const sampler = { filter: "linear", mipmap: "none", wrap: "clamp" } as const;
  const textures = { c0: { src: maps.causes0, sampler }, c1: { src: maps.causes1, sampler }, c2: { src: maps.causes2, sampler }, c3: { src: maps.causes3, sampler } };
  if (o.debug !== undefined) {
    if (!(o.debug in DEBUG_CAUSES)) throw new TypeError(`wearShader: debug is one of ${Object.keys(DEBUG_CAUSES).join(", ")}, got ${JSON.stringify(o.debug)}`);
    return { wesl: CAUSES + debugHook(o.debug, WEAR_CLASSES.map((c) => [...c.colour])), uniforms, textures };
  }
  return { wesl: CAUSES + HELPERS + recipes(want) + SHARED + WEATHER + SURFACE, uniforms, textures };
}

/** The recipes of `want` and the switch that picks one by the texel's class. */
function recipes(want: readonly string[]): string {
  let out = "";
  const cases: string[] = [];
  WEAR_CLASSES.forEach((c, i) => {
    if (!want.includes(c.name)) return;
    out += `\nfn wear_${c.name}(s: ptr<function, WearSurf>, c: WearCauses, g: WearGeo, m: WearMarks) {${RECIPES[c.name]}\n}\n`;
    cases.push(`    case ${i}: { wear_${c.name}(s, c, g, m); }`);
  });
  return `${out}
fn wear_class(s: ptr<function, WearSurf>, c: WearCauses, g: WearGeo, m: WearMarks) -> bool {
  switch c.cls {
${cases.join("\n")}
    default: { return false; }
  }
  return true;
}
`;
}

const SURFACE = /* wgsl */ `
fn wear_surface(uv: vec2<f32>, pbr: ptr<function, PbrInput>) {
  let px = length(fwidth((*pbr).world_position.xyz));
  let c = wear_causes(uv);
  let g = wear_geo((*pbr).world_position.xyz, (*pbr).world_normal, c, (*pbr).V, px);
  let m = wear_marks(c, g);
  var s = WearSurf((*pbr).material.base_color.rgb, (*pbr).material.perceptual_roughness, (*pbr).material.metallic);
  if (!wear_class(&s, c, g, m)) { return; }
  wear_shared(&s, c, g, m);
  wear_weather(&s, c, g, m);
  (*pbr).material.base_color = vec4<f32>(s.col, (*pbr).material.base_color.a);
  (*pbr).material.perceptual_roughness = clamp(s.rough, 0.04, 1.0);
  (*pbr).material.metallic = clamp(s.metal, 0.0, 1.0);
}

fn surface(in: VertexOutput, pbr: ptr<function, PbrInput>) {
  @if(VERTEX_UVS_B) {
    wear_surface(in.uv_b, pbr);
  }
}
`;
