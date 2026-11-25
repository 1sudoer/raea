// One WESL function per material class, answering each cause as docs/wear.feature WR5's table has it, then what every
// class shares (stains, rising damp, splash) and the season and the day's weather over them (WR9), which nothing bakes.
// Each recipe changes the surface's own colour, roughness and metalness, so a model keeps its textures and the marks
// lie over them. First tried on the game project's city tile (its components/wear/look.ts), and moved here.
// Notes on causes: Bonazza (NABU MedShip 2019) for black crusts on sheltered stone and rain-washed stone staying
// light; the rest are the marks a street of 19th-century fronts shows, by the cause each map measures.

/** A recipe's WESL: `fn wear_<class>(s: ptr<function, WearSurf>, c: WearCauses, g: WearGeo, m: WearMarks)`. */
export const RECIPES: Record<string, string> = {
  plaster: /* wgsl */ `
  // rain-washed: soot rinsed off, the paint chalked paler and flatter
  (*s).col = mix((*s).col, (*s).col * 1.06 + vec3<f32>(0.02), m.wash * 0.35 * m.age);
  (*s).col = mix((*s).col, vec3<f32>(dot((*s).col, vec3<f32>(0.3, 0.45, 0.25))) * 1.05, (0.15 * c.sun + 0.1 * m.wash) * m.wear);
  (*s).rough = mix((*s).rough, 0.95, m.wash * 0.4 * m.age);
  // sheltered: soot grey, darkest tucked under cornices
  (*s).col = mix((*s).col, (*s).col * vec3<f32>(0.3, 0.29, 0.27), m.crust);
  // WR6: the render lost where the causes loosen it (the damp foot, soaked faces, knocked arrises, along a crack) and
  // neglect lets it go, to the brick 2 cm behind it; the broken edge's lip shades the hole under it
  let lost = m.loss * m.neglect;
  let lossAt = 0.8 - 0.34 * lost;
  let q = g.p * vec2<f32>(0.9, 1.4) + vec2<f32>(c.elem * 0.31, 5.0);
  let blowN = wear_fbm2(q);
  let gone = smoothstep(lossAt, lossAt + 0.012, blowN) * step(0.03, lost);
  let over = smoothstep(lossAt, lossAt + 0.012, wear_fbm2(q + vec2<f32>(0.0, 0.042)));
  let lip = smoothstep(lossAt - 0.035, lossAt, blowN) * (1.0 - gone) * step(0.03, lost);
  let behind = wear_bond(g.p + g.par * 0.02) * mix(1.0, 0.55, m.crust);
  (*s).col = mix((*s).col, behind * (1.0 - 0.55 * (1.0 - over)), gone);
  (*s).col = (*s).col * (1.0 - 0.4 * lip);
  (*s).rough = mix((*s).rough, 0.95, gone);
  // chipped arrises low down: the lime coat knocked off to the brick
  (*s).col = mix((*s).col, mix(vec3<f32>(0.78, 0.76, 0.7), wear_bond(g.p), 0.5), m.chip * (1.0 - gone));
  // a grease band where shoulders and hubs rub
  (*s).col = mix((*s).col, (*s).col * vec3<f32>(0.5, 0.48, 0.45), c.touch * 0.6 * g.wall);
  (*s).rough = mix((*s).rough, 0.55, c.touch * 0.5);`,

  brick: /* wgsl */ `
  // rain-washed: the mortar washed out of the joints, so they read as dark lines
  let j = wear_bond_joint(g.p) * g.wall;
  (*s).col = mix((*s).col, (*s).col * 0.55, j * m.wash * m.age * 0.6);
  // sheltered: soot black
  (*s).col = mix((*s).col, vec3<f32>(0.04, 0.035, 0.03), clamp(m.crust * 1.2, 0.0, 0.9));
  // under a ledge: efflorescence, the salts the soaked water leaves as it dries
  let eff = smoothstep(0.35, 0.75, c.flow) * (1.0 - m.wash) * smoothstep(0.45, 0.7, wear_fbm2(g.p * vec2<f32>(3.0, 1.2))) * m.age;
  (*s).col = mix((*s).col, vec3<f32>(0.82, 0.8, 0.76), eff * 0.6);
  // the ground band: green on faces the sun misses
  let algae = (1.0 - c.sun) * (1.0 - smoothstep(0.3, 1.5, g.h)) * smoothstep(0.4, 0.7, wear_fbm2(g.p * 1.7)) * m.age * g.wall;
  (*s).col = mix((*s).col, vec3<f32>(0.16, 0.22, 0.1), algae * 0.6);
  // spalled faces: frost in damp brick takes the fired face off, the softer core showing paler and rough
  let spall = step(0.74 - 0.3 * m.damp - 0.15 * (1.0 - m.upkeep), wear_vn2(g.p * vec2<f32>(4.0, 12.9) + vec2<f32>(c.elem, 0.0))) * max(m.low, c.edge) * step(0.1, m.wear);
  (*s).col = mix((*s).col, (*s).col * vec3<f32>(1.2, 1.02, 0.9) + vec3<f32>(0.03), spall * 0.8);
  (*s).rough = mix((*s).rough, 0.98, max(spall, m.wash * 0.3));`,

  sandstone: /* wgsl */ `
  // rain-washed: the stone washed light
  (*s).col = mix((*s).col, (*s).col * 1.18 + vec3<f32>(0.03), m.wash * 0.6 * m.age);
  // sheltered: the black gypsum crust
  (*s).col = mix((*s).col, vec3<f32>(0.05, 0.045, 0.04), clamp(m.crust * 1.3, 0.0, 0.95));
  // under a ledge: the run-off's paths washed white between the black (the deposit's grime is the shared stain)
  (*s).col = mix((*s).col, (*s).col * 1.25 + vec3<f32>(0.04), m.tear * (1.0 - m.wash) * 0.5 * m.age);
  // rounded arrises: worn back and pale
  (*s).col = mix((*s).col, (*s).col * 1.12, c.edge * 0.5 * m.age);
  (*s).rough = mix((*s).rough, 0.97, c.edge * m.age);`,

  granite: /* wgsl */ `
  // granite barely weathers: some soot where it's sheltered, a cleaner face where the rain reaches
  (*s).col = mix((*s).col, vec3<f32>(0.06, 0.055, 0.05), m.crust * 0.6);
  (*s).col = mix((*s).col, (*s).col * 1.08, m.wash * 0.3 * m.age);`,

  wood: /* wgsl */ `
  // painted wood: sun-faded on the faces the sun reaches (south and west), chalked, and alligatored there
  (*s).col = mix((*s).col, vec3<f32>(dot((*s).col, vec3<f32>(0.33))) * 1.25 + vec3<f32>(0.04), c.sun * 0.45 * m.wear);
  let cells = abs(wear_vn2(g.p * vec2<f32>(14.0, 9.0)) - 0.5);
  (*s).col = (*s).col * (1.0 - 0.45 * (1.0 - smoothstep(0.015, 0.05, cells)) * smoothstep(0.3, 0.8, c.sun) * m.wear);
  // under a ledge and at the bottom rails: peeling where the run-off soaks the timber, and by neglect (WR6) wherever
  // the coat's loosened: the top coat off to the red-lead primer, then to the grey wood
  let wetWood = clamp(c.flow + (1.0 - smoothstep(0.04, 0.5, g.h)) * g.wall, 0.0, 1.0);
  let bare = wear_fbm2(g.p * vec2<f32>(9.0, 4.0)) + 0.2 * wetWood * m.wear + 0.25 * m.loss * m.neglect;
  let reach = max(wetWood, m.loss * smoothstep(0.1, 0.5, m.neglect)) * step(0.1, m.wear);
  let primer = smoothstep(0.46, 0.49, bare) * reach;
  let peel = smoothstep(0.53, 0.56, bare) * reach;
  (*s).col = mix((*s).col, vec3<f32>(0.58, 0.34, 0.25) * (0.9 + 0.2 * wear_vn2(g.p * 25.0)), primer * (1.0 - peel));
  (*s).col = mix((*s).col, vec3<f32>(0.3, 0.27, 0.22) * (0.8 + 0.3 * wear_vn2(vec2<f32>(g.p.x * 30.0, g.p.y * 2.0))), max(peel, m.chip));
  // rot at the foot
  let rot = (1.0 - smoothstep(0.02, 0.25, g.h)) * m.wear * g.wall;
  (*s).col = mix((*s).col, vec3<f32>(0.12, 0.08, 0.05), rot * 0.8);
  (*s).rough = mix((*s).rough, 0.95, max(rot, peel));
  // worn to the wood where held and kicked, in patches where boots and hands land
  let worn = c.touch * smoothstep(0.42, 0.58, wear_fbm2(g.p * vec2<f32>(9.0, 4.0)) + 0.12 * (c.touch - 0.5));
  (*s).col = mix((*s).col, vec3<f32>(0.15, 0.1, 0.065) * (0.8 + 0.3 * wear_vn2(vec2<f32>(g.p.x * 40.0, g.p.y * 4.0))), worn * 0.9);
  (*s).col = mix((*s).col, (*s).col * 0.7, c.touch * 0.35);
  (*s).rough = mix((*s).rough, 0.42, c.touch * 0.7);`,

  iron: /* wgsl */ `
  // painted iron: the paint flaking at joints and arrises and where water stands, rust at the foot (its run-off
  // stains the stone below: the deposit's kind, read on the classes under it)
  let edgy = c.edge * step(12.0, u_age().w);
  let footRust = 1.0 - smoothstep(0.05, 0.4, g.h);
  let rust = 0.8 * clamp((0.35 * m.wash + 0.6 * edgy + 0.5 * c.cav + 0.6 * footRust) * smoothstep(0.4, 0.8, wear_fbm2(g.p * 4.0) + 0.25 * (1.0 - m.upkeep)), 0.0, 1.0) * m.wear;
  (*s).col = mix((*s).col, vec3<f32>(0.11, 0.045, 0.02) * (0.7 + 0.5 * wear_vn2(g.p * 21.0)), rust);
  (*s).rough = mix((*s).rough, 0.95, rust);
  (*s).metal = mix((*s).metal, 0.0, rust);
  // bright where held
  (*s).col = mix((*s).col, vec3<f32>(0.2, 0.19, 0.18), c.touch);
  (*s).metal = mix((*s).metal, 0.9, c.touch * 0.8);
  (*s).rough = mix((*s).rough, 0.32, c.touch);`,

  zinc: /* wgsl */ `
  // rain-washed: the pale blue-grey carbonate patina (its white run-off is the deposit's, on what's below)
  let pat = m.wash * m.age * smoothstep(0.3, 0.7, wear_vn2(g.p * 3.0) + 0.3);
  (*s).col = mix((*s).col, vec3<f32>(0.62, 0.64, 0.63), pat * 0.7);
  (*s).metal = mix((*s).metal, 0.2, pat);
  (*s).rough = mix((*s).rough, 0.8, pat);`,

  copper: /* wgsl */ `
  // rain-washed: verdigris, slower than the rest (decades); sheltered copper goes brown-black instead
  let slow = 1.0 - exp(-max(u_age().x, 0.0) / 40.0);
  let green = m.wash * smoothstep(0.1, 0.8, slow) * (0.7 + 0.3 * wear_vn2(g.p * 5.0));
  (*s).col = mix((*s).col, vec3<f32>(0.18, 0.09, 0.05), m.age * (1.0 - green) * 0.8);
  (*s).col = mix((*s).col, vec3<f32>(0.25, 0.52, 0.42), green);
  (*s).metal = mix((*s).metal, 0.0, max(green, m.age * 0.5));
  (*s).rough = mix((*s).rough, 0.85, green);`,

  slate: /* wgsl */ `
  // lichen on the faces the sun misses, soot in the laps
  let lichen = (1.0 - c.sun) * smoothstep(0.55, 0.75, wear_fbm2(g.p * 2.5 + vec2<f32>(c.elem * 0.13, 1.0))) * m.age;
  (*s).col = mix((*s).col, vec3<f32>(0.2, 0.22, 0.13), lichen * 0.6);
  (*s).col = mix((*s).col, vec3<f32>(0.03), m.crust * 0.5);
  // WR6: slates slipped and gone, the odd one by neglect and many once the house is empty, more along the ridge, the
  // verges and the eaves, the battens and the
  // felt under them showing (courses 11 cm up the slope's height, slates 25 cm across, broken bond)
  let course = floor(g.p.y / 0.11);
  let across = g.p.x / 0.25 + select(0.0, 0.5, i32(course) % 2 == 1);
  let missing = step(wear_h21(vec2<f32>(floor(across) + c.elem * 17.0, course)), 0.1 * (0.15 * m.neglect + 0.85 * m.abandon) * (1.0 + 2.0 * c.edge));
  let batten = 1.0 - smoothstep(0.0, 0.18, abs(fract(g.p.y / 0.11) - 0.5) * 2.0 - 0.6);
  (*s).col = mix((*s).col, mix(vec3<f32>(0.04, 0.035, 0.03), vec3<f32>(0.3, 0.21, 0.12), batten), missing);
  (*s).rough = mix((*s).rough, 0.95, missing);`,

  felt: /* wgsl */ `
  // roofing felt: greyed by the sun, its grit washed off where the rain runs
  (*s).col = mix((*s).col, vec3<f32>(dot((*s).col, vec3<f32>(0.33))) * 1.2 + vec3<f32>(0.03), c.sun * 0.4 * m.wear);
  (*s).col = mix((*s).col, (*s).col * 0.8, m.tear * m.age * 0.5);`,

  glass: /* wgsl */ `
  // sheltered: a grime film, thickest in the corners and at the bottom where the frame holds the dirt
  let film = clamp(m.shelter * 0.5 + c.cav * 0.8 + (1.0 - m.wash) * 0.2, 0.0, 1.0) * m.age * (1.0 - 0.6 * m.upkeep);
  (*s).col = mix((*s).col, (*s).col * vec3<f32>(0.62, 0.6, 0.55), film * 0.7);
  (*s).rough = mix((*s).rough, 0.55, film * 0.6);
  // WR6: a house left empty has its panes broken and boarded over, one fate per pane
  let fate = wear_h21(vec2<f32>(c.elem, 11.0));
  if (fate < 0.16 * m.abandon) {
    // boarded: planks of grey, weathered deal across the opening
    let row = fract(g.p.y / 0.15);
    let tone = 0.75 + 0.4 * wear_h21(vec2<f32>(floor(g.p.y / 0.15), c.elem));
    let gap = 1.0 - smoothstep(0.0, 0.06, min(row, 1.0 - row));
    (*s).col = vec3<f32>(0.36, 0.32, 0.26) * tone * (0.85 + 0.3 * wear_vn2(vec2<f32>(g.p.x * 30.0, g.p.y * 3.0))) * (1.0 - 0.7 * gap);
    (*s).rough = 0.9;
    (*s).metal = 0.0;
  } else if (fate < 0.38 * m.abandon) {
    // broken: the dark room behind, shards left in the putty at the frame
    let shard = smoothstep(0.52, 0.56, wear_fbm2(g.p * 7.0 + vec2<f32>(c.elem, 0.0)) * 0.6 + 0.7 * c.cav);
    (*s).col = mix(vec3<f32>(0.012, 0.011, 0.01), (*s).col, shard);
    (*s).rough = mix(1.0, (*s).rough, shard);
  }`,

  enamel: /* wgsl */ `
  // rust bled from its screws, in short tails below them
  let col = floor(g.p.x * 6.0);
  let screw = step(0.8, wear_h21(vec2<f32>(col, c.elem)));
  let tail = screw * (1.0 - smoothstep(0.0, 0.04, abs(fract(g.p.x * 6.0) - 0.5) / 6.0)) * smoothstep(0.3, 0.9, wear_vn2(vec2<f32>(col, g.p.y * 3.0)));
  (*s).col = mix((*s).col, vec3<f32>(0.3, 0.12, 0.04), tail * m.wear * 0.8);
  // chipped to black iron at its holes and edges, and by neglect (WR6) flaking off in sheets from them to the iron,
  // which rusts, and rust creeping under the enamel's broken edge
  let chipped = smoothstep(0.65, 0.9, c.edge) * step(0.62 - 0.3 * m.wear, wear_vn2(g.p * 30.0)) * step(0.05, m.wear);
  let flakeN = wear_fbm2(g.p * 14.0 + vec2<f32>(c.elem * 3.1, 0.0));
  let flakeAt = 0.82 - 0.38 * m.neglect * max(smoothstep(0.5, 0.9, c.edge), 0.5 * m.loss + 0.3);
  let flake = smoothstep(flakeAt, flakeAt + 0.01, flakeN) * step(0.05, m.neglect);
  let creep = smoothstep(flakeAt - 0.03, flakeAt, flakeN) * (1.0 - flake) * step(0.05, m.neglect);
  let iron = mix(vec3<f32>(0.03, 0.03, 0.035), vec3<f32>(0.2, 0.08, 0.03), smoothstep(0.3, 0.7, wear_vn2(g.p * 40.0)) * flake);
  (*s).col = mix((*s).col, iron, max(chipped, flake));
  (*s).col = mix((*s).col, vec3<f32>(0.25, 0.1, 0.04), creep * 0.7);
  (*s).rough = mix((*s).rough, 0.75, max(chipped, flake));`,

  brass: /* wgsl */ `
  // tarnished dull brown, more where the rain reaches; bright where hands touch it
  let tarnish = (0.5 + 0.5 * m.wash) * m.age * (1.0 - 0.8 * m.upkeep);
  (*s).col = mix((*s).col, vec3<f32>(0.3, 0.22, 0.1), tarnish * 0.8);
  (*s).rough = mix((*s).rough, 0.7, tarnish);
  (*s).col = mix((*s).col, vec3<f32>(0.85, 0.68, 0.35), c.touch);
  (*s).metal = mix((*s).metal, 1.0, c.touch);
  (*s).rough = mix((*s).rough, 0.25, c.touch);`,

  ground: /* wgsl */ `
  // the street: dirt where the sweepings and the wind leave it (the wall's foot, the gutter, every corner), moss in
  // the joints the sun never dries, paving polished where feet pass
  let corner = smoothstep(0.08, 0.4, c.cav) * m.age;
  (*s).col = mix((*s).col, vec3<f32>(0.085, 0.075, 0.06) * (0.7 + 0.6 * wear_vn2(g.p * 5.0)), corner * 0.75);
  let moss = (1.0 - c.sun) * smoothstep(0.5, 0.75, wear_fbm2(g.p * 1.5)) * m.wear;
  (*s).col = mix((*s).col, vec3<f32>(0.11, 0.14, 0.06), moss * 0.55);
  (*s).col = mix((*s).col, (*s).col * 1.12 + vec3<f32>(0.015), c.touch * 0.5);
  (*s).rough = mix((*s).rough, 0.5, c.touch * 0.6);`,
};

/** Shared helpers some recipes call: a brick bond in metres on a face, and its joints (named apart from the recipes'
 *  own wear_<class>). */
export const HELPERS = /* wgsl */ `
fn wear_bond_joint(p: vec2<f32>) -> f32 {
  let row = floor(p.y / 0.0775);
  let bx = p.x / 0.25 + select(0.0, 0.5, i32(row) % 2 == 1);
  let f = vec2<f32>(fract(bx), fract(p.y / 0.0775));
  return max(1.0 - smoothstep(0.0, 0.05, min(f.x, 1.0 - f.x)), 1.0 - smoothstep(0.0, 0.14, min(f.y, 1.0 - f.y)));
}
fn wear_bond(p: vec2<f32>) -> vec3<f32> {
  let row = floor(p.y / 0.0775);
  let bx = p.x / 0.25 + select(0.0, 0.5, i32(row) % 2 == 1);
  let tone = 0.8 + 0.35 * wear_h21(vec2<f32>(floor(bx), row));
  return mix(vec3<f32>(0.42, 0.2, 0.13) * tone, vec3<f32>(0.3, 0.29, 0.27), wear_bond_joint(p));
}
// autumn's fallen leaves: lime and plane in ochre and brown, a few red
fn wear_leaf(p: vec2<f32>) -> vec3<f32> {
  let k = wear_vn2(p * 9.0);
  let base = mix(vec3<f32>(0.42, 0.26, 0.07), vec3<f32>(0.2, 0.11, 0.045), smoothstep(0.3, 0.7, k));
  return mix(base, vec3<f32>(0.36, 0.1, 0.035), smoothstep(0.78, 0.92, wear_vn2(p * 23.0)));
}
`;

/** What every class shares after its own recipe: the run-off's stains on what it reaches (not the metals that made
 *  them), rising damp and its salts in what draws water up, and the splash-back over the walk. */
export const SHARED = /* wgsl */ `
fn wear_shared(s: ptr<function, WearSurf>, c: WearCauses, g: WearGeo, m: WearMarks) {
  let metal = c.cls == 5 || c.cls == 6 || c.cls == 7 || c.cls == 12;
  var stainCol = vec3<f32>(0.045, 0.04, 0.035);
  if (c.kind == 2) { stainCol = vec3<f32>(0.24, 0.1, 0.035); } else if (c.kind == 3) { stainCol = vec3<f32>(0.1, 0.32, 0.24); } else if (c.kind == 4) { stainCol = vec3<f32>(0.66, 0.66, 0.64); }
  if (!metal && c.kind > 0) { (*s).col = mix((*s).col, stainCol, m.stain * select(0.7, 0.85, c.kind >= 2)); }
  // plaster, brick, sandstone and granite draw the ground's water up
  let porous = select(0.0, 1.0, c.cls >= 0 && c.cls <= 3);
  (*s).col = mix((*s).col, (*s).col * vec3<f32>(0.6, 0.63, 0.58), m.damp * 0.8 * porous);
  (*s).col = mix((*s).col, vec3<f32>(0.8, 0.79, 0.74), m.salt * 0.3 * porous * smoothstep(0.4, 0.75, wear_vn2(vec2<f32>(g.p.x * 1.3, g.p.y * 3.0))));
  if (c.cls != 13 && c.cls != 10) { (*s).col = mix((*s).col, (*s).col * 0.62, m.splash * 0.7); }
  (*s).rough = mix((*s).rough, 0.97, m.crust * 0.5);
  // WR6: cracks from the openings' corners, opening as neglect grows and gathering dirt along them; in brick a crack
  // steps along the joints. Drawn no thinner than a pixel, fading as it gets thinner than one
  let opened = smoothstep(0.1, 0.7, m.neglect) * porous;
  let wide = 0.0035 * opened;
  let dd = max(c.crack + (wear_vn2(g.p * 38.0) - 0.5) * 0.006, 0.0);
  let line = (1.0 - smoothstep(0.5 * wide, 0.5 * wide + g.px, dd)) * min(1.0, wide / max(g.px, 0.0001) + 0.25) * step(0.0001, wide);
  let stepped = (1.0 - smoothstep(0.02, 0.05, c.crack)) * wear_bond_joint(g.p) * opened;
  let crack = select(line, stepped, c.cls == 1);
  let halo = (1.0 - smoothstep(0.0, 0.03, dd)) * opened;
  (*s).col = (*s).col * (1.0 - 0.8 * crack) * (1.0 - 0.15 * halo);
}
`;

/** WR9: the season and the day's weather over the causes, never baked: rain wetting what it reaches, streaming down
 *  the run-off's paths and puddling in the walk's hollows; snow lying on what looks up and is open, banked where
 *  litter settles, thinned by December's sun, trodden off by touch, plastered by a storm by drift; frost where the
 *  winter sun misses; spring's damp and algae; summer's dust; autumn's leaves. */
export const WEATHER = /* wgsl */ `
fn wear_weather(s: ptr<function, WearSurf>, c: WearCauses, g: WearGeo, m: WearMarks) {
  let wet = u_wx().x;
  let snow = u_wx().y;
  let storm = u_wx().z;
  let ice = u_wx().w;
  let damp = u_season().x;
  let dust = u_season().y;
  let leaves = u_season().z;
  let melt = u_season().w;
  let up = smoothstep(0.55, 0.85, g.up);
  // spring: damp lingers on north faces and plinths; green algae where the run-off and the shade meet
  (*s).col = (*s).col * (1.0 - 0.25 * damp * (1.0 - c.sun) * (0.5 + 0.5 * m.low) * g.wall);
  (*s).col = mix((*s).col, vec3<f32>(0.14, 0.2, 0.08), damp * smoothstep(0.4, 0.8, c.flow) * (1.0 - c.sun) * g.wall * 0.5);
  // summer's dust on sills, ledges and treads
  (*s).col = mix((*s).col, vec3<f32>(0.47, 0.43, 0.37), dust * (1.0 - wet) * up * (0.3 + 0.5 * c.sky) * 0.45);
  // rain: only what it reaches is wet, the run-off's paths stream and shine, the walk's hollows hold puddles
  let rained = wet * smoothstep(0.08, 0.45, c.sky);
  let rill = wet * smoothstep(0.5, 0.85, c.flow) * g.wall;
  (*s).col = (*s).col * (1.0 - 0.35 * rained) * (1.0 - 0.3 * rill);
  (*s).rough = mix((*s).rough, (*s).rough * 0.55, rained);
  (*s).rough = mix((*s).rough, 0.1, rill);
  let level = smoothstep(0.9, 0.98, g.up);
  let pud = wet * level * smoothstep(0.1, 0.4, c.sky) * smoothstep(0.62, 0.74, wear_fbm2(g.p * 0.7 + vec2<f32>(3.0, 9.0)) + 0.3 * c.litter + 0.25 * c.cav);
  (*s).col = mix((*s).col, (*s).col * 0.5, pud);
  (*s).rough = mix((*s).rough, 0.03, pud);
  // autumn's leaves where blown things settle, dark once rained on
  let leaf = leaves * up * smoothstep(0.12, 0.45, c.litter + 0.35 * wear_fbm2(g.p * 4.0) - 0.1);
  (*s).col = mix((*s).col, wear_leaf(g.p) * mix(1.0, 0.55, wet), leaf * 0.9);
  // frost on what the winter sun misses
  let frost = ice * (1.0 - c.dec) * smoothstep(0.02, 0.3, c.open) * (0.6 + 0.4 * wear_vn2(g.p * 40.0));
  (*s).col = mix((*s).col, vec3<f32>(0.82, 0.86, 0.9), frost * 0.5);
  (*s).rough = mix((*s).rough, 0.9, frost * 0.5);
  // snow lying on what looks up and is open above, banked where litter settles, plastered on what a storm drives at
  let cover = smoothstep(0.3, 0.6, wear_fbm2(g.p * 1.9) * 0.7 + 0.45 * snow);
  let lies = snow * up * cover * smoothstep(0.02, 0.25, c.open) * (1.0 - 0.8 * c.dec * melt) * (1.0 - c.touch);
  let bank = snow * up * smoothstep(0.05, 0.35, c.litter);
  let flake = smoothstep(0.35, 0.65, wear_fbm2(vec2<f32>(g.p.x, g.p.y) * 3.1) + 0.45 * c.drift - 0.2);
  let plastered = storm * (1.0 - up) * smoothstep(0.2, 0.6, c.drift) * flake;
  let sn = clamp(max(max(lies, bank), plastered), 0.0, 1.0);
  // old snow in the lee greys with the soot
  let snowCol = mix(vec3<f32>(0.92, 0.93, 0.95), vec3<f32>(0.62, 0.61, 0.6), u_age().y * c.litter * 0.5);
  (*s).col = mix((*s).col, snowCol, sn);
  (*s).rough = mix((*s).rough, 0.85, sn);
  (*s).metal = mix((*s).metal, 0.0, sn);
}
`;
