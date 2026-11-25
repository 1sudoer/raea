//! The touch rules: door leaves, treads, rails, passages and gateways (heights from docs/wear.feature WR3, ASSUMED
//! there until a measured source is read).

use std::collections::{HashMap, HashSet};

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};

use crate::{Mat, Parts, Point, Surface, band, seed};

const LEAF_W: (f32, f32) = (0.7, 1.6);
const LEAF_H: f32 = 1.9;
const LEAF_T: f32 = 0.12;
/// a leaf stands on a floor (anything level) within this under its foot
const STAND: f32 = 0.2;
/// how far a leaf may be set into a step or threshold below the floor it opens onto
const SET_IN: f32 = 0.3;
/// two leaves whose edges are this close in one plane are a pair
const PAIR_GAP: f32 = 0.08;
const HANDLE: (f32, f32) = (0.95, 1.1);
/// the handle band reaches this far in from the lock edge
const HAND_REACH: f32 = 0.3;
const KICK: f32 = 0.3;
/// the kick band's strength at the hinge edge, against 1 at the lock edge (a foot pushes beside the hand; ASSUMED)
const KICK_HINGE: f32 = 0.35;
/// a passage's faces run at least this far each way from a point on them
const DEEP: f32 = 0.3;
const TREAD_D: (f32, f32) = (0.2, 0.45);
const TREAD_W: (f32, f32) = (0.6, 4.0);
/// one step's rise, between treads of a flight
const RISE: (f32, f32) = (0.1, 0.23);
/// a single step's tread at most this over the walking surface
const FIRST_STEP: f32 = 0.26;
const RAIL_L: f32 = 0.5;
const RAIL_X: f32 = 0.12;
const RAIL_Y: (f32, f32) = (0.75, 1.2);
const SHOULDER: (f32, f32) = (1.2, 1.6);
const PASSAGE: f32 = 3.0;
const HUB: (f32, f32) = (0.4, 0.9);
const GATE: (f32, f32) = (2.2, 4.5);
/// a gateway's ceiling within this over its floor
const CEILING: f32 = 6.0;

/// A door leaf: its part, the way across it (horizontal), its span along that way and its lock edge, its facing,
/// the middle of its foot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Door {
    pub part: u32,
    pub across: Vec3,
    pub lo: f32,
    pub hi: f32,
    pub lock: f32,
    pub paired: bool,
    pub facing: Vec3,
    pub foot: Vec3,
}

fn area(s: &Surface, t: u32) -> f32 {
    let [a, b, c] = s.tri[t as usize].map(|v| s.pos[v as usize]);
    (b - a).cross(c - a).length() / 2.0
}

/// The span of a part's corners along a direction.
fn span(s: &Surface, tris: &[u32], d: Vec3) -> (f32, f32) {
    let mut r = (f32::MAX, f32::MIN);
    for &t in tris {
        for &v in &s.tri[t as usize] {
            let x = s.pos[v as usize].dot(d);
            r = (r.0.min(x), r.1.max(x));
        }
    }
    r
}

/// The drop from o to the walking surface below, through anything that isn't one.
fn drop_to_walk(s: &Surface, mut o: Vec3) -> Option<f32> {
    let start = o;
    for _ in 0..8 {
        let h = s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -s.up), 100.0, false)?;
        let q = o - s.up * h.time_of_impact;
        if s.walk[h.subshape as usize] {
            return Some((start - q).dot(s.up));
        }
        o = q - s.up * 0.01;
    }
    None
}

/// The horizontal way a set of corners runs longest, by their spread (principal axis in the level plane).
fn longest(s: &Surface, tris: &[u32]) -> Vec3 {
    let (e1, e2) = s.up.any_orthonormal_pair();
    let pts: Vec<(f32, f32)> = tris.iter().flat_map(|&t| s.tri[t as usize]).map(|v| (s.pos[v as usize].dot(e1), s.pos[v as usize].dot(e2))).collect();
    let n = pts.len().max(1) as f32;
    let (ma, mb) = pts.iter().fold((0.0, 0.0), |(a, b), p| (a + p.0 / n, b + p.1 / n));
    let (mut caa, mut cbb, mut cab) = (0.0, 0.0, 0.0);
    for (a, b) in &pts {
        caa += (a - ma) * (a - ma);
        cbb += (b - mb) * (b - mb);
        cab += (a - ma) * (b - mb);
    }
    let th = 0.5 * (2.0 * cab).atan2(caa - cbb);
    e1 * th.cos() + e2 * th.sin()
}

pub(crate) fn doors(s: &Surface, parts: &Parts) -> Vec<Door> {
    let mut out: Vec<Door> = Vec::new();
    for (k, tris) in parts.tris().iter().enumerate() {
        if tris.is_empty() || s.mat[tris[0] as usize] != Mat::Wood {
            continue;
        }
        let Some(&big) = tris.iter().filter(|&&t| s.nrm[t as usize].dot(s.up).abs() < 0.2).max_by(|&&a, &&b| area(s, a).total_cmp(&area(s, b))) else { continue };
        let n = s.nrm[big as usize];
        let facing = (n - s.up * n.dot(s.up)).normalize_or_zero();
        if facing == Vec3::ZERO {
            continue;
        }
        let across = s.up.cross(facing).normalize();
        let ((lo, hi), (y0, y1), (t0, t1)) = (span(s, tris, across), span(s, tris, s.up), span(s, tris, facing));
        if !(LEAF_W.0..=LEAF_W.1).contains(&(hi - lo)) || y1 - y0 < LEAF_H || t1 - t0 > LEAF_T {
            continue;
        }
        // it stands on a floor in front or behind, within STAND of its foot: the leaf may start under a step or a
        // threshold it is set into, so the floor is looked for from SET_IN over the foot, and the higher floor
        // found is the foot the handle's height is taken from
        let foot = across * ((lo + hi) / 2.0) + s.up * y0 + facing * ((t0 + t1) / 2.0);
        let floor = [1.0f32, -1.0]
            .iter()
            .filter_map(|&side| {
                let o = foot + facing * (side * ((t1 - t0) / 2.0 + 0.05)) + s.up * SET_IN;
                let h = s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -s.up), SET_IN + STAND + 0.05, false)?;
                (s.nrm[h.subshape as usize].dot(s.up) > 0.7).then_some(SET_IN - h.time_of_impact)
            })
            .max_by(f32::total_cmp);
        if let Some(rise) = floor {
            out.push(Door { part: k as u32, across, lo, hi, lock: f32::NAN, paired: false, facing, foot: foot + s.up * rise });
        }
    }
    // a pair: two leaves in one plane whose edges meet; their lock sides are where they meet
    let meets = |a: &Door| {
        out.iter().filter(|b| b.part != a.part).find_map(|b| {
            if a.facing.dot(b.facing).abs() < 0.95 || (b.foot - a.foot).dot(a.facing).abs() > 0.15 {
                return None;
            }
            let (c, hw) = (b.foot.dot(a.across), (b.hi - b.lo) / 2.0);
            if (c - hw - a.hi).abs() < PAIR_GAP {
                Some(a.hi)
            } else if (a.lo - (c + hw)).abs() < PAIR_GAP {
                Some(a.lo)
            } else {
                None
            }
        })
    };
    let locks: Vec<Option<f32>> = out.iter().map(meets).collect();
    for (d, m) in out.iter_mut().zip(locks) {
        (d.lock, d.paired) = match m {
            Some(l) => (l, true),
            None => (if seed(d.foot) & 1 == 0 { d.lo } else { d.hi }, false),
        };
    }
    out
}

/// A tread: a point on its middle line, the way across it and half its width.
struct Tread {
    mid: Vec3,
    along: Vec3,
    half: f32,
}

/// The treads: each part's level faces grouped by height (2 cm), each group step-sized, in a flight (another tread a
/// rise away beside it) or a single step at the walking surface.
fn treads(s: &Surface, parts: &Parts) -> (HashMap<u32, usize>, Vec<Tread>) {
    let mut groups: HashMap<(u32, i64), Vec<u32>> = HashMap::new();
    for t in 0..s.tri.len() as u32 {
        if s.nrm[t as usize].dot(s.up) > 0.9 && !s.walk[t as usize] && matches!(s.mat[t as usize], Mat::Masonry | Mat::Wood) {
            let y = s.pos[s.tri[t as usize][0] as usize].dot(s.up);
            groups.entry((parts.of[t as usize], (y * 50.0).round() as i64)).or_default().push(t);
        }
    }
    let mut keys: Vec<_> = groups.keys().copied().collect();
    keys.sort();
    let mut cands: Vec<(Tread, f32, Vec<u32>)> = Vec::new();
    for k in keys {
        let tris = &groups[&k];
        let along = longest(s, tris);
        let depth = s.up.cross(along);
        let ((a0, a1), (d0, d1)) = (span(s, tris, along), span(s, tris, depth));
        if !(TREAD_W.0..=TREAD_W.1).contains(&(a1 - a0)) || !(TREAD_D.0..=TREAD_D.1).contains(&(d1 - d0)) {
            continue;
        }
        let y = k.1 as f32 / 50.0;
        let mid = along * ((a0 + a1) / 2.0) + depth * ((d0 + d1) / 2.0) + s.up * y;
        cands.push((Tread { mid, along, half: (a1 - a0) / 2.0 }, y, tris.clone()));
    }
    let mut of = HashMap::new();
    for (i, (t, y, tris)) in cands.iter().enumerate() {
        let flight = cands.iter().any(|(u, z, _)| {
            let h = (t.mid - u.mid) - s.up * (y - z);
            (RISE.0..=RISE.1).contains(&(y - z).abs()) && h.length() < 0.7
        });
        let first = || drop_to_walk(s, t.mid + s.up * 0.01).is_some_and(|d| d <= FIRST_STEP + 0.01);
        if flight || first() {
            for &tr in tris {
                of.insert(tr, i);
            }
        }
    }
    (of, cands.into_iter().map(|c| c.0).collect())
}

/// The rails: iron or wood parts long, thin and at hand height.
fn rails(s: &Surface, parts: &Parts, doors: &[Door]) -> HashSet<u32> {
    let door: HashSet<u32> = doors.iter().map(|d| d.part).collect();
    let mut out = HashSet::new();
    for (k, tris) in parts.tris().iter().enumerate() {
        if tris.is_empty() || door.contains(&(k as u32)) || !matches!(s.mat[tris[0] as usize], Mat::Metal | Mat::Wood) {
            continue;
        }
        let along = longest(s, tris);
        let ((a0, a1), (x0, x1), (y0, y1)) = (span(s, tris, along), span(s, tris, s.up.cross(along)), span(s, tris, s.up));
        if a1 - a0 < RAIL_L || x1 - x0 > RAIL_X || y1 - y0 > RAIL_X {
            continue;
        }
        let top = along * ((a0 + a1) / 2.0) + s.up.cross(along) * ((x0 + x1) / 2.0) + s.up * (y1 + 0.01);
        if drop_to_walk(s, top).is_some_and(|d| (RAIL_Y.0..=RAIL_Y.1).contains(&d)) {
            out.insert(k as u32);
        }
    }
    out
}

/// A wall's point: the shoulder band if another wall faces it under PASSAGE away, the hub band if one faces it GATE
/// away under a ceiling.
fn walls(s: &Surface, q: &Point) -> f32 {
    let o = q.p + q.n * 0.01;
    let Some(h) = s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, q.n), GATE.1, false) else { return 0.0 };
    if s.nrm[h.subshape as usize].dot(q.n) > -0.8 {
        return 0.0;
    }
    let d = h.time_of_impact;
    // people pass between the two faces only if the floor between them is walked: straight down from the middle the
    // first face is a walking surface (between a window's reveals it is the sill)
    // and only along a face as deep as a body (the side of a projecting surround is not a passage wall)
    let t = s.up.cross(q.n).normalize_or_zero();
    let deep = [1.0f32, -1.0].iter().all(|&k| {
        let o = q.p + t * (k * DEEP) + q.n * 0.05;
        s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -q.n), 0.1, false).is_some_and(|h| s.nrm[h.subshape as usize].dot(q.n) > 0.9)
    });
    if !deep {
        return 0.0;
    }
    let mid = o + q.n * (d / 2.0);
    if !s.mesh.cast_local_ray_and_get_normal(&Ray::new(mid, -s.up), 100.0, false).is_some_and(|h| s.walk[h.subshape as usize]) {
        return 0.0;
    }
    let mut v: f32 = 0.0;
    if d < PASSAGE {
        v = v.max(0.8 * band(q.ground, SHOULDER.0, SHOULDER.1, 0.05));
    }
    if (GATE.0..=GATE.1).contains(&d) && s.mesh.cast_local_ray(&Ray::new(mid, s.up), CEILING, false).is_some() {
        v = v.max(band(q.ground, HUB.0, HUB.1, 0.05));
    }
    v
}

pub(crate) fn touch(s: &Surface, parts: &Parts, doors: &[Door], pts: &[Point]) -> Vec<f32> {
    let door_of: HashMap<u32, &Door> = doors.iter().map(|d| (d.part, d)).collect();
    let (tread_of, treads) = treads(s, parts);
    let rails = rails(s, parts, doors);
    pts.iter()
        .map(|q| {
            let (t, part) = (q.tri as usize, parts.of[q.tri as usize]);
            let mut v: f32 = 0.0;
            if let Some(d) = door_of.get(&part) {
                let y = (q.p - d.foot).dot(s.up);
                let from_lock = (q.p.dot(d.across) - d.lock).abs();
                let reach = (1.0 - from_lock / HAND_REACH).clamp(0.0, 1.0);
                let kick = KICK_HINGE + (1.0 - KICK_HINGE) * (1.0 - from_lock / (d.hi - d.lo)).clamp(0.0, 1.0);
                v = v.max(band(y, HANDLE.0, HANDLE.1, 0.05) * reach).max(band(y, 0.0, KICK, 0.05) * kick);
            }
            if let Some(&i) = tread_of.get(&q.tri) {
                let tr = &treads[i];
                let x = (q.p - tr.mid).dot(tr.along) / tr.half;
                v = v.max((1.0 - x * x).max(0.0));
            }
            if rails.contains(&part) && q.n.dot(s.up) > 0.5 {
                v = 1.0;
            }
            if q.n.dot(s.up).abs() < 0.2 && matches!(s.mat[t], Mat::Plaster | Mat::Masonry) && (HUB.0 - 0.05..=SHOULDER.1 + 0.05).contains(&q.ground) {
                v = v.max(walls(s, q));
            }
            v
        })
        .collect()
}
