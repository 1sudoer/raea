// A cause in false colour (docs/wear.feature WR5): the shader's `lit` hook returns the cause's own colour in place of
// the lit one, so light and exposure don't change it. Render with <ToneMapping mode="none"/> and no fog to read the
// values exactly; a mesh without the bake's second UV set keeps its lit colour.

/** Each cause the debug mode shows, as the colour it's drawn in (WESL over `c: WearCauses`). */
export const DEBUG_CAUSES = {
  /** rain's reach: black where none comes, white where it all does */
  sky: "vec3<f32>(c.sky)",
  /** the year's sun: black in shade all year, yellow in the most */
  sun: "vec3<f32>(1.0, 0.85, 0.2) * c.sun",
  /** occlusion within 0.5 m */
  cavity: "vec3<f32>(c.cav)",
  /** nearness to a convex arris */
  edge: "vec3<f32>(c.edge)",
  /** run-off passing, in blue */
  flow: "vec3<f32>(0.1, 0.4, 1.0) * c.flow",
  /** what the run-off left, in the colour of what it left: grey grime, rust, green copper, white zinc */
  deposit: "wear_kind_colour(c.kind) * c.dep",
  /** hands, feet and wheels, in amber over the sky term in grey */
  touch: "mix(vec3<f32>(0.1 + 0.25 * c.sky), vec3<f32>(1.0, 0.62, 0.08), c.touch)",
  /** height over the walk: blue at the walk, through green and yellow, red at 4 m and over */
  ground: "wear_ramp(c.ground / 4.0)",
  /** each element its own colour (a front, a ledge: what an upkeep draw covers) */
  element: "vec3<f32>(wear_h21(vec2<f32>(c.elem, 1.0)), wear_h21(vec2<f32>(c.elem, 2.0)), wear_h21(vec2<f32>(c.elem, 3.0)))",
  /** the share of the whole sky seen */
  open: "vec3<f32>(c.open)",
  /** December's sun, in pale blue */
  winter: "vec3<f32>(0.6, 0.8, 1.0) * c.dec",
  /** driven snow's reach */
  drift: "vec3<f32>(c.drift)",
  /** where blown leaves and snow settle, in orange */
  litter: "vec3<f32>(1.0, 0.5, 0.1) * c.litter",
  /** the material class, by WEAR_CLASSES' colour; magenta where the bake found none */
  class: "wear_class_colour(c.cls)",
  /** nearness to a crack from an opening's corner (WR6), in red */
  crack: "vec3<f32>(1.0, 0.25, 0.1) * (1.0 - c.crack / 0.25)",
  /** how readily the coat comes off (WR6): the damp foot, soaked faces, knocked arrises, cracks */
  loss: "vec3<f32>(wear_loss(c))",
} as const;

export type WearCause = keyof typeof DEBUG_CAUSES;

/** The `lit` hook drawing `cause`; `palette` is each class's colour in WEAR_CLASSES' order. */
export function debugHook(cause: WearCause, palette: [number, number, number][]): string {
  const expr = DEBUG_CAUSES[cause];
  if (expr === undefined) throw new TypeError(`wearShader: debug is one of ${Object.keys(DEBUG_CAUSES).join(", ")}, got ${JSON.stringify(cause)}`);
  const v = (c: [number, number, number]) => `vec3<f32>(${c.map((x) => x.toFixed(3)).join(", ")})`;
  const cases = palette.map((c, i) => `    case ${i}: { return ${v(c)}; }`).join("\n");
  return /* wgsl */ `
fn wear_kind_colour(k: i32) -> vec3<f32> {
  switch k {
    case 2: { return vec3<f32>(0.75, 0.3, 0.08); }
    case 3: { return vec3<f32>(0.2, 0.8, 0.55); }
    case 4: { return vec3<f32>(0.95); }
    default: { return vec3<f32>(0.45); }
  }
}
fn wear_ramp(t: f32) -> vec3<f32> {
  let x = clamp(t, 0.0, 1.0);
  return clamp(vec3<f32>(1.5 - abs(4.0 * x - 3.0), 1.5 - abs(4.0 * x - 2.0), 1.5 - abs(4.0 * x - 1.0)), vec3<f32>(0.0), vec3<f32>(1.0));
}
fn wear_class_colour(k: i32) -> vec3<f32> {
  switch k {
${cases}
    default: { return vec3<f32>(1.0, 0.0, 1.0); }
  }
}
fn lit(in: VertexOutput, pbr: PbrInput, color: vec4<f32>) -> vec4<f32> {
  var out = color;
  @if(VERTEX_UVS_B) {
    let c = wear_causes(in.uv_b);
    out = vec4<f32>(${expr}, 1.0);
  }
  return out;
}
`;
}
