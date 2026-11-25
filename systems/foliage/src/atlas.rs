//! A species' leaves drawn by rules into an atlas of CELLS x CELLS variants (docs/foliage.feature FO2). A leaf is a
//! stalk and blades: each blade has a midrib from an origin at an angle, a width along it (u^p (1-u)^q, its widest at
//! p/(p+q), so p small is a broad base and q small a broad tip), teeth (a saw along the margin), lobes (a wave that
//! cuts into the margin) and veins (secondaries leaving the midrib at an angle). Palmate leaves are several blades from
//! one point over a disc; compound and pinnate ones are leaflets on a stalk. In a cell the leaf stands on its stalk's
//! foot at the bottom middle, x across in leaf lengths (the species' aspect wide), y up from 0 to 1.
//! The channels are what the leaf hook needs, not a colour (stored in a picture the engine reads as sRGB, so the hook
//! re-encodes red, green and blue; alpha is linear):
//!   alpha  the blade, anti-aliased             red    veins
//!   green  blotches (two octaves of noise)     blue   the distance in from the edge (autumn comes from the edge)

use image::RgbaImage;

use crate::Rng;
use crate::species::{LeafKind, Species};

pub const CELLS: u32 = 4;
const CELL: u32 = 256;
/// the alpha is a signed distance to the blade's edge: 0.5 on it, 0 and 1 SPREAD/2 texels out and in, so a hook can
/// widen a leaf as its texels shrink below a pixel (no mipmaps) and a magnified edge stays sharp
pub const SPREAD: f32 = 12.0;

#[derive(Clone, Copy)]
struct Blade {
    o: [f32; 2],
    ang: f32,
    len: f32,
    w: f32,
    p: f32,
    q: f32,
    t0: f32,
    teeth: f32,
    depth: f32,
    lobes: f32,
    veins: f32,
    vein_ang: f32,
    asym: f32,
    sinus: bool,
}

const B: Blade = Blade { o: [0.0, 0.0], ang: 0.0, len: 0.8, w: 0.3, p: 0.5, q: 0.8, t0: 0.0, teeth: 0.0, depth: 0.0, lobes: 0.0, veins: 8.0, vein_ang: 0.8, asym: 0.0, sinus: false };

fn shape(u: f32, p: f32, q: f32) -> f32 {
    let m = (p / (p + q)).powf(p) * (q / (p + q)).powf(q);
    u.powf(p) * (1.0 - u).powf(q) / m
}


impl Blade {
    /// How far inside the blade (x, y) is (leaf lengths; negative outside), and its vein strength 0-1.
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x - self.o[0], y - self.o[1]);
        let (s, c) = self.ang.sin_cos();
        let along = dx * s + dy * c;
        let cross = dx * c - dy * s;
        let t = along / self.len;
        if t < self.t0 - 0.05 || t > 1.05 {
            return (-1.0, 0.0);
        }
        let u = ((t - self.t0) / (1.0 - self.t0)).clamp(0.0, 1.0);
        let side = if cross > 0.0 { 1.0 + self.asym } else { 1.0 - self.asym };
        let mut w = self.w * self.len * shape(u, self.p, self.q) * side;
        if self.lobes > 0.0 {
            w *= 0.42 + 0.58 * (std::f32::consts::PI * u * self.lobes).sin().abs().powf(0.6);
        }
        if self.teeth > 0.0 {
            w *= 1.0 - self.depth * (u * self.teeth).fract() * crate::leaves::smooth(0.0, 0.2, u);
        }
        let mut inside = w - cross.abs();
        if self.sinus && t < 0.04 {
            inside = inside.min(cross.abs() - (0.04 - t) * self.len * 1.2);
        }
        let mid = (1.0 - cross.abs() / (self.len * 0.012 * (1.3 - u))).clamp(0.0, 1.0);
        let k = 1.0 / self.vein_ang.tan();
        let root = t - cross.abs() / self.len * k;
        let f = (root * self.veins + 0.5).fract();
        let d = f.min(1.0 - f) / self.veins * self.len * self.vein_ang.sin();
        let sec = (1.0 - d / (self.len * 0.006)).clamp(0.0, 1.0) * if cross.abs() < w * 0.9 && root > self.t0 + 0.04 { 1.0 } else { 0.0 };
        (inside, mid.max(0.65 * sec))
    }
}

/// A leaf: a stalk (from the foot to `stalk`, its half-width), blades and discs (centre, radius).
struct Leaf {
    stalk: [f32; 2],
    stalk_w: f32,
    blades: Vec<Blade>,
    discs: Vec<([f32; 2], f32)>,
}

impl Leaf {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        let (sx, sy) = (self.stalk[0], self.stalk[1]);
        let l2 = sx * sx + sy * sy;
        let h = ((x * sx + y * sy) / l2.max(1e-6)).clamp(0.0, 1.0);
        let stalk = self.stalk_w - (x - sx * h).hypot(y - sy * h);
        let mut best = (stalk, if stalk > 0.0 { 1.0 } else { 0.0 });
        for b in &self.blades {
            let (i, v) = b.at(x, y);
            if i > best.0 {
                best = (i, if i > 0.0 { v } else { 0.0 });
            } else if i > 0.0 {
                best.1 = best.1.max(v);
            }
        }
        for &(c, r) in &self.discs {
            best.0 = best.0.max(r - (x - c[0]).hypot(y - c[1]));
        }
        best
    }
}

/// The leaf of `kind`, varied by `j` (-1..1 numbers).
fn leaf(kind: LeafKind, j: [f32; 4]) -> Leaf {
    let tilt = j[0] * 0.06;
    let one = |stalk: f32, b: Blade| Leaf { stalk: [0.0, stalk], stalk_w: 0.009, blades: vec![Blade { o: [0.0, stalk], ang: tilt, ..b }], discs: vec![] };
    match kind {
        LeafKind::Cordate => one(0.17, Blade { len: 0.8, w: 0.5 + 0.04 * j[1], p: 0.38, q: 0.85, t0: -0.12, teeth: 24.0, depth: 0.07, veins: 6.0, vein_ang: 0.75, asym: 0.08 * j[2], sinus: true, ..B }),
        LeafKind::Ovate => one(0.08, Blade { len: 0.9, w: 0.3 + 0.03 * j[1], p: 0.55, q: 0.8, t0: -0.02, teeth: 28.0, depth: 0.09, veins: 12.0, vein_ang: 0.7, asym: 0.12 + 0.04 * j[2], ..B }),
        LeafKind::Lanceolate => one(0.06, Blade { len: 0.93, w: 0.11 + 0.015 * j[1], p: 0.65, q: 0.7, teeth: 45.0, depth: 0.04, veins: 14.0, vein_ang: 0.9, ..B }),
        LeafKind::Triangular => one(0.22, Blade { len: 0.78, w: 0.44 + 0.03 * j[1], p: 0.3, q: 1.1, teeth: 16.0, depth: 0.12, veins: 7.0, vein_ang: 0.8, ..B }),
        LeafKind::Rhombic => one(0.24, Blade { len: 0.76, w: 0.5 + 0.04 * j[1], p: 0.45, q: 0.85, teeth: 18.0, depth: 0.06, veins: 6.0, vein_ang: 0.8, ..B }),
        LeafKind::Oak => one(0.05, Blade { len: 0.95, w: 0.32 + 0.03 * j[1], p: 0.6, q: 0.6, t0: -0.03, lobes: 4.5 + 0.5 * j[2], veins: 5.0, vein_ang: 0.8, ..B }),
        LeafKind::Palmate | LeafKind::Plane => {
            let maple = kind == LeafKind::Palmate;
            let (foot, angs, lens, w, teeth, depth, disc) = if maple {
                (0.36, [0.0, 0.75, -0.75, 1.45, -1.45], [0.62, 0.55, 0.55, 0.33, 0.33], 0.26, 3.5, 0.35, 0.2)
            } else {
                (0.3, [0.0, 0.8, -0.8, 1.5, -1.5], [0.66, 0.58, 0.58, 0.3, 0.3], 0.34, 5.0, 0.2, 0.24)
            };
            let blades = angs.iter().zip(lens).map(|(&a, l)| Blade { o: [0.0, foot], ang: a + tilt, len: l * (1.0 + 0.05 * j[1]), w, p: 0.3, q: if maple { 1.0 } else { 0.9 }, teeth, depth, veins: 4.0, vein_ang: 0.9, ..B }).collect();
            Leaf { stalk: [0.0, foot], stalk_w: 0.009, blades, discs: vec![([0.0, foot + 0.06], disc)] }
        }
        LeafKind::Compound => {
            let blades = [(0.0, 0.68), (0.5, 0.62), (-0.5, 0.62), (1.0, 0.5), (-1.0, 0.5), (1.5, 0.32), (-1.5, 0.32)]
                .iter()
                .map(|&(a, l)| Blade { o: [0.0, 0.3], ang: a + tilt, len: l * (1.0 + 0.05 * j[1]), w: 0.15, p: 1.1, q: 0.6, t0: 0.05, teeth: 30.0, depth: 0.04, veins: 14.0, vein_ang: 0.8, ..B })
                .collect();
            Leaf { stalk: [0.0, 0.3], stalk_w: 0.01, blades, discs: vec![] }
        }
        LeafKind::Pinnate => {
            let pairs = 6;
            let mut blades: Vec<Blade> = (0..pairs)
                .flat_map(|k| {
                    let y = 0.18 + 0.6 * k as f32 / (pairs - 1) as f32;
                    [1.0f32, -1.0].map(|s| Blade { o: [0.0, y], ang: s * (1.2 + 0.1 * j[2]), len: 0.2, w: 0.32, p: 0.7, q: 0.7, veins: 6.0, ..B })
                })
                .collect();
            blades.push(Blade { o: [0.0, 0.78], ang: tilt, len: 0.2, w: 0.32, p: 0.7, q: 0.7, veins: 6.0, ..B });
            Leaf { stalk: [0.0, 0.9], stalk_w: 0.006, blades, discs: vec![] }
        }
    }
}

fn hash2(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    (h ^ (h >> 15)) as f32 / u32::MAX as f32
}

fn noise(x: f32, y: f32, s: u32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash2(ix, iy, s) + (hash2(ix + 1, iy, s) - hash2(ix, iy, s)) * ux;
    let b = hash2(ix, iy + 1, s) + (hash2(ix + 1, iy + 1, s) - hash2(ix, iy + 1, s)) * ux;
    a + (b - a) * uy
}

/// A card's length over a leaf's, and its width over its length. One leaf fills its card; a sprig (a twig and its
/// leaves) fills a card about 1.5 + 0.22 a leaf leaf-lengths long and 0.85 of that wide; its twig takes the lower 0.58 of it and its tip leaf the rest.
pub(crate) fn card(sp: &Species) -> (f32, f32) {
    if sp.sprig <= 1 { (1.0, sp.aspect) } else { (1.5 + 0.22 * sp.sprig as f32, 0.85) }
}

/// A leaf on a card: its stalk's foot, its angle off straight up (toward +x), its size (card lengths), its own tone.
struct Placed {
    at: [f32; 2],
    ang: f32,
    scale: f32,
    leaf: Leaf,
    tone: f32,
}

/// A card's leaves and its twig's tip (the twig runs from the foot, the cell's bottom middle). The twig's leaves come
/// alternately left and right, or in opposite pairs, below a leaf at its tip; the lower ones smaller and spread wider.
fn sprig(sp: &Species, rng: &mut Rng) -> (Vec<Placed>, [f32; 2]) {
    let mut j = || [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
    if sp.sprig <= 1 {
        let jj = j();
        return (vec![Placed { at: [0.0, 0.0], ang: 0.0, scale: 1.0, leaf: leaf(sp.leaf, jj), tone: 0.5 + 0.5 * jj[3] }], [0.0, 0.0]);
    }
    let unit = 1.0 / card(sp).0;
    let bend = 0.06 * j()[0];
    let along = |t: f32| [bend * t * t, 0.58 * t];
    let tip = along(1.0);
    let mut out = Vec::new();
    let lateral = sp.sprig as usize - 1;
    let rows = if sp.opposite { lateral.div_ceil(2) } else { lateral };
    for k in 0..lateral {
        let row = if sp.opposite { k / 2 } else { k };
        let t = 0.1 + 0.8 * row as f32 / rows.max(1) as f32;
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let jj = j();
        out.push(Placed { at: along(t), ang: side * (1.25 - 0.5 * t) + 0.15 * jj[0], scale: unit * (0.8 + 0.25 * t) * (1.0 + 0.08 * jj[1]), leaf: leaf(sp.leaf, jj), tone: 0.5 + 0.5 * jj[3] });
    }
    let jj = j();
    out.push(Placed { at: tip, ang: 2.0 * bend + 0.1 * jj[0], scale: unit * 0.9, leaf: leaf(sp.leaf, jj), tone: 0.5 + 0.5 * jj[3] });
    (out, tip)
}

/// The species' atlas: CELLS x CELLS cards, each varied from the seed.
pub fn atlas(sp: &Species, seed: u64) -> RgbaImage {
    let size = CELLS * CELL;
    let mut img = RgbaImage::new(size, size);
    let mut rng = Rng::new(seed ^ 0xa71a5);
    let aspect = card(sp).1;
    let px = aspect.max(1.0) / CELL as f32;
    for cell in 0..CELLS * CELLS {
        let (leaves, tip) = sprig(sp, &mut rng);
        let s = rng.next() as u32;
        let (cx, cy) = ((cell % CELLS) * CELL, (cell / CELLS) * CELL);
        let tl = (tip[0] * tip[0] + tip[1] * tip[1]).max(1e-6);
        for py in 0..CELL {
            for pxi in 0..CELL {
                let x = ((pxi as f32 + 0.5) / CELL as f32 - 0.5) * aspect;
                let y = 1.0 - (py as f32 + 0.5) / CELL as f32;
                // the twig: a vein-coloured line from the foot to its tip
                let h = ((x * tip[0] + y * tip[1]) / tl).clamp(0.0, 1.0);
                let twig = 0.007 - (x - tip[0] * h).hypot(y - tip[1] * h);
                let (mut inside, mut vein, mut own, mut tone, mut local) = (twig, if twig > 0.0 { 1.0 } else { 0.0 }, 0.0f32, 0.5, [x, y]);
                for p in &leaves {
                    let (dx, dy) = (x - p.at[0], y - p.at[1]);
                    let (sa, ca) = p.ang.sin_cos();
                    let l = [(dx * ca - dy * sa) / p.scale, (dx * sa + dy * ca) / p.scale];
                    let (i, v) = p.leaf.at(l[0], l[1]);
                    if i * p.scale > inside {
                        (inside, vein, own, tone, local) = (i * p.scale, if i > 0.0 { v } else { 0.0 }, i, p.tone, l);
                    }
                }
                let a = (0.5 + inside / (SPREAD * px)).clamp(0.0, 1.0);
                let blot = 0.6 * (0.65 * noise(local[0] * 10.0, local[1] * 10.0, s) + 0.35 * noise(local[0] * 26.0, local[1] * 26.0, s ^ 7)) + 0.4 * tone;
                let edge = (own / 0.08).clamp(0.0, 1.0);
                img.put_pixel(cx + pxi, cy + py, image::Rgba([(vein * 255.0) as u8, (blot * 255.0) as u8, (edge * 255.0) as u8, (a * 255.0) as u8]));
            }
        }
    }
    img
}

pub(crate) fn png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The share of a cell's pixels that are leaf.
pub(crate) fn coverage(img: &RgbaImage, cell: u32) -> f32 {
    let (cx, cy) = ((cell % CELLS) * CELL, (cell / CELLS) * CELL);
    let n = (0..CELL * CELL).filter(|i| img.get_pixel(cx + i % CELL, cy + i / CELL)[3] > 127).count();
    n as f32 / (CELL * CELL) as f32
}

/// The share of a card the species' leaves cover, over its atlas's cells: the leaf area a card carries is its quad's
/// times this (a lime's sprig about a third, a willow's narrow leaves about a tenth).
pub(crate) fn cover(img: &RgbaImage) -> f32 {
    (0..CELLS * CELLS).map(|c| coverage(img, c)).sum::<f32>() / (CELLS * CELLS) as f32
}

/// Whether a cell's card covers a point (x across in card lengths, y up).
#[cfg(test)]
pub(crate) fn covers(sp: &Species, img: &RgbaImage, cell: u32, x: f32, y: f32) -> bool {
    let (cx, cy) = ((cell % CELLS) * CELL, (cell / CELLS) * CELL);
    let px = ((x / card(sp).1 + 0.5) * CELL as f32) as u32;
    let py = ((1.0 - y) * CELL as f32) as u32;
    img.get_pixel(cx + px.min(CELL - 1), cy + py.min(CELL - 1))[3] > 127
}

/// How far inside a plain leaf of a kind a point is (for the tests).
#[cfg(test)]
pub(crate) fn leaf_at(kind: LeafKind) -> impl Fn(f32, f32) -> f32 {
    let l = leaf(kind, [0.0; 4]);
    move |x, y| l.at(x, y).0
}
