//! Leaves on the outer shoots (docs/foliage.feature FO2): along each shoot within `leafy` m of a tip and thin enough to
//! bear them, at the species' leaves a metre (thinned to the request's budget). Each leaf is a card from its stalk's
//! foot (the atlas cell's bottom middle) to its tip:
//!   - its direction: out from the shoot (a golden-angle turn a leaf), forward along it, out of the crown, and down by
//!     the species' hang;
//!   - its face: toward up and out of the crown, made square to its direction, its tip curled a little;
//!   - its shading normal bent from its face toward out of the crown, so a crown shades as a volume (the leaf hook
//!     uses this normal on both sides);
//!   - its size: shade leaves (deep in the crown) are larger, as they are on a real tree;
//!   - its shading normal: from its face toward where its sky is (light.rs's bent normal), so clumps shade as clumps.
//! Its colour: r its own random number (its group, its fall and its tint), g its sky view through the crown's leaves
//! (light.rs: the hook's ambient occlusion; sunlit leaves colour first), b where it sits on its shoot (1 at the tip:
//! it unfolds last).

use parry3d::glamx::Vec3;

use crate::grow::{Envelope, Node, children};
use crate::species::Species;
use crate::{CELLS, GROUPS, Mesh, Request, Rng, across};

const GOLDEN: f32 = 2.399_963;
/// a shoot thicker than this (m) bears no leaves
const BEARS: f32 = 0.02;
/// `cover` is the share of a card its leaves cover (atlas.rs cover), so the cards carry the species' leaf area.
pub(crate) fn leaves(sp: &Species, req: &Request, nodes: &[Node], env: &Envelope, cover: f32, rng: &mut Rng) -> Vec<Mesh> {
    let kids = children(nodes);
    // each node's distance (along the tree) to its nearest tip
    let mut to_tip = vec![f32::INFINITY; nodes.len()];
    for i in (0..nodes.len()).rev() {
        if kids[i].is_empty() {
            to_tip[i] = 0.0;
        }
        if i > 0 {
            let p = nodes[i].parent;
            to_tip[p] = to_tip[p].min(to_tip[i] + (nodes[i].p - nodes[p].p).length());
        }
    }
    let bears = |i: usize| i > 0 && to_tip[i] < sp.leafy && nodes[i].r < BEARS && nodes[i].p.y > req.clear * 0.9;
    // clumps and sky holes: a card is kept by a low noise through the crown (about 1.5 m across), and the rest are
    // thickened to hold the leaf area
    let keep = |p: Vec3| smooth(sp.gaps - 0.12, sp.gaps + 0.12, 0.65 * noise3(p / 1.5, req.seed) + 0.35 * noise3(p / 0.6, req.seed ^ 9));
    let mid = |i: usize| (nodes[i].p + nodes[nodes[i].parent].p) * 0.5;
    let length: f32 = (1..nodes.len()).filter(|&i| bears(i)).map(|i| (nodes[i].p - nodes[nodes[i].parent].p).length()).sum();
    let kept: f32 = (1..nodes.len()).filter(|&i| bears(i)).map(|i| (nodes[i].p - nodes[nodes[i].parent].p).length() * keep(mid(i))).sum();
    // the crown carries its species' leaf area (leaf area index x the crown's ground area), on cards of a sprig each
    // `cover` full; a budget under that gets fewer, bigger cards (up to 2.5 times as long, or 5 on a pruned level:
    // one seen from far off, where a crown short of its leaf area reads as see-through and a big card does not show)
    let (card_len, card_w) = crate::atlas::card(sp);
    let card_area = (sp.leaf_len * card_len).powi(2) * card_w * cover;
    // leaf area index is over the crown's ground area, which undercounts a column: a crown deeper than 1.3 times its
    // width (a poplar's) carries more in proportion (leaf area density times its volume)
    let column = ((req.height - req.clear) / req.width / 1.3).max(1.0);
    let want = sp.lai * column * std::f32::consts::PI * (req.width / 2.0).powi(2) / card_area;
    let cards = want.min(req.leaves as f32);
    let bigger = (want / cards.max(1.0)).sqrt().min(if req.prune > 0.0 { 5.0 } else { 2.5 });
    let per_m = cards / kept.max(0.05 * length).max(1.0);

    let centre = env.centre();
    // every card first, then their light (light.rs), then the meshes
    struct Card {
        at: Vec3,
        dir: Vec3,
        face: Vec3,
        len: f32,
        own: f32,
        shoot: f32,
        cell: u32,
    }
    let mut cards: Vec<Card> = Vec::new();
    let mut turn = 0.0f32;
    for i in (1..nodes.len()).filter(|&i| bears(i)) {
        let (a, b) = (nodes[nodes[i].parent].p, nodes[i].p);
        let seg = (b - a).length();
        if seg < 1e-4 {
            continue;
        }
        let axis = (b - a) / seg;
        let count = (per_m * seg + rng.f()) as usize;
        for _ in 0..count {
            let at = a + (b - a) * rng.f();
            if rng.f() > keep(at) {
                continue;
            }
            turn += GOLDEN;
            let r0 = across(axis);
            let radial = r0 * turn.cos() + axis.cross(r0) * turn.sin();
            let off = at - centre;
            let out_of = Vec3::new(off.x, off.y * 0.5, off.z).normalize_or_zero();
            let dir = (radial * 0.8 + axis * 0.45 + out_of * 0.35 + Vec3::Y * 0.15 - Vec3::Y * sp.leaf_droop + rng.unit() * 0.2).normalize();
            // leaves face up and out of the crown, but no two alike: from the side a crown still shows faces, not edges
            let face = Vec3::Y * 0.6 + out_of * 0.5 + rng.unit() * 0.7;
            let face = (face - dir * face.dot(dir)).normalize_or_zero();
            if face == Vec3::ZERO {
                continue;
            }
            let len = bigger * sp.leaf_len * card_len * rng.range(0.8, 1.2);
            let (own, shoot) = (rng.f(), 1.0 - (to_tip[i] / sp.leafy).min(1.0));
            let cell = (rng.f() * (CELLS * CELLS) as f32) as u32 % (CELLS * CELLS);
            cards.push(Card { at, dir, face, len, own, shoot, cell });
        }
    }
    let mid: Vec<Vec3> = cards.iter().map(|c| c.at + c.dir * (c.len * 0.5)).collect();
    let area: Vec<f32> = cards.iter().map(|c| c.len * c.len * card_w * cover).collect();
    let (sky, bent) = crate::light::light(&mid, &area);

    let mut out = vec![Mesh::default(); GROUPS];
    for (k, c) in cards.iter().enumerate() {
        // exposure is the leaf's sky view; shade leaves (deep in the crown) are larger, as on a real tree
        let exposed = sky[k].clamp(0.0, 1.0);
        let len = c.len * (1.0 + 0.3 * (1.0 - exposed));
        let w = len * card_w;
        let side = c.face.cross(c.dir);
        // the shading normal leans from the leaf's face to where its sky is (its bent normal)
        let bend = (c.face * 0.35 + bent[k] * 0.65).normalize_or(bent[k]);
        let tip = c.at + c.dir * len - c.face * (len * 0.15);
        let (cu, cv) = ((c.cell % CELLS) as f32 / CELLS as f32, (c.cell / CELLS) as f32 / CELLS as f32);
        let step = 1.0 / CELLS as f32;
        let m = &mut out[((c.own * GROUPS as f32) as usize).min(GROUPS - 1)];
        let base = m.pos.len() as u32;
        for (q, u, v) in [(c.at - side * (w / 2.0), 0.0, 0.0), (c.at + side * (w / 2.0), 1.0, 0.0), (tip - side * (w / 2.0), 0.0, 1.0), (tip + side * (w / 2.0), 1.0, 1.0)] {
            m.pos.push(q.to_array());
            m.nrm.push(bend.to_array());
            // the atlas's rows run top down; a cell's bottom row is the stalk's foot
            m.uv.push([cu + u * step, cv + (1.0 - v) * step]);
            m.col.push([c.own, exposed, c.shoot, 1.0]);
        }
        m.idx.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
    }
    out
}

/// 0 below `a`, 1 above `b`, smoothly between (a leaf's gaps, its teeth's roots in atlas.rs).
pub(crate) use raea_core_ease::smoothstep as smooth;

/// Value noise in 3D, 0 to 1, from a seed.
fn noise3(p: Vec3, seed: u64) -> f32 {
    let h = |x: i32, y: i32, z: i32| {
        let mut k = (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (y as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) ^ (z as u64).wrapping_mul(0x1656_67b1_9e37_79f9) ^ seed;
        k ^= k >> 31;
        k = k.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        (k >> 40) as f32 / (1u64 << 24) as f32
    };
    let (i, f) = (p.floor(), p - p.floor());
    let u = f * f * (Vec3::splat(3.0) - f * 2.0);
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dy: i32, dz: i32| lerp(h(x, y + dy, z + dz), h(x + 1, y + dy, z + dz), u.x);
    lerp(lerp(c(0, 0), c(1, 0), u.y), lerp(c(0, 1), c(1, 1), u.y), u.z)
}
