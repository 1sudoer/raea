//! Where a building breaks (docs/wear.feature WR6), by rules over the weathering bake's texels:
//!   - cracks run from the re-entrant corners of a wall's openings (a window's, a door's), where a wall's stresses
//!     gather, away from the opening and steeper than 45 degrees, 0.35-1.25 m, wandering as they go; each texel gets
//!     its distance to the nearest crack on its own wall (to REACH), which raea/wear draws, wider as neglect grows;
//!   - the loss potential (`loss`): how readily a point's coat (render, paint, enamel) comes off, from what loosens
//!     it: the damp foot of a wall, run-off soaking where the rain doesn't wash it, a knocked arris low down, a crack.
//!     raea/wear's recipes lose the coat where noise passes a threshold that potential and neglect lower, by the same
//!     formula (its test reads LOSS here);
//!   - debris on the walking surface under plaster with a high loss potential, each with that potential, so a scene
//!     puts down only what its neglect has brought down (raea/wear's wearDebris).
//! Sizes and weights are ASSUMED until measured.

#[cfg(test)]
mod tests;

use parry3d::bounding_volume::Aabb;
use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};
use rayon::prelude::*;

use raea_core_ease::smoothstep as smooth;
use raea_sys_touch::{Mat, Point, Spot, Surface};

/// A cache key's part: a change to the rules changes every bake that used them.
pub const VERSION: &str = "raea-sys-damage 3";

/// How far a texel's distance to a crack is kept (metres): the cause map holds 1 - d / REACH.
pub const REACH: f32 = 0.25;

/// The loss potential's edges and weights, in the order `loss` uses them (raea/wear's causes.ts has the same list,
/// and its test compares them): the foot band (ground 0.7-1.3 m), the soak (flow 0.5-0.9, the rain's reach washing
/// it from 0.35-0.85), the arris (edge 0.75-0.9, below 1.4-2.2 m), the crack (within 0.12 m), then the weights of
/// the soak, the arris and the crack against the foot's 1.
pub const LOSS: [f32; 13] = [0.7, 1.3, 0.5, 0.9, 0.35, 0.85, 0.75, 0.9, 1.4, 2.2, 0.12, 0.8, 0.7];
const CRACK_WEIGHT: f32 = 0.6;

/// a probe's reach across the wall either way when looking for an opening's corner
const PROBE: f32 = 0.12;
/// a probe meets the wall when it lands within this of the texel's plane
const PLANE: f32 = 0.04;
/// corner texels this near with the same opening are one corner
const CORNER: f32 = 0.3;
/// the share of re-entrant corners that crack (ASSUMED: about half; which ones by the corner's place, so a house
/// cracks the same way every bake)
const CRACKED: u64 = 50;
const STEP: f32 = 0.04;
const DEBRIS: [f32; 2] = [0.5, 0.3];
const GAP_DEBRIS: f32 = 1.5;
/// debris only under plaster this ready to come off
const DEBRIS_LOSS: f32 = 0.5;
/// how far out from the wall a heap lies (metres), and how far below the wall's foot the walk may be
const DEBRIS_OUT: f32 = 0.2;
const DROP: f32 = 0.5;
/// a wall the rain reaches less than this is indoors (a room's wall, the back of a hollow model): nothing falls on a
/// walk from it
const OUTDOORS: f32 = 0.02;

/// How readily the coat at `q` comes off (0-1), with `crack` its distance to the nearest crack (metres).
pub fn loss(q: &Point, crack: f32) -> f32 {
    let l = &LOSS;
    let foot = 1.0 - smooth(l[0], l[1], q.ground);
    let soak = smooth(l[2], l[3], q.flow) * (1.0 - smooth(l[4], l[5], q.rain));
    let arris = smooth(l[6], l[7], q.edge) * (1.0 - smooth(l[8], l[9], q.ground));
    let near = 1.0 - smooth(0.0, l[10], crack);
    foot.max(l[11] * soak).max(l[12] * arris).max(CRACK_WEIGHT * near)
}

fn wallish(m: Mat) -> bool {
    matches!(m, Mat::Plaster | Mat::Masonry)
}

fn standing(s: &Surface, q: &Point) -> bool {
    q.n.dot(s.up).abs() < 0.2 && wallish(s.mat[q.tri as usize])
}

/// The wall of the face at `p` (normal `n`) goes on at `at`: a ray into it from 0.3 m out meets plaster or masonry
/// facing the same way within PLANE of p's plane (a reveal, a recess, a sill in front or nothing at all is no wall).
fn wall_at(s: &Surface, p: Vec3, n: Vec3, at: Vec3) -> bool {
    let o = at + n * 0.3;
    s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -n), 0.6, false).is_some_and(|h| {
        let t = h.subshape as usize;
        wallish(s.mat[t]) && s.nrm[t].dot(n) > 0.9 && ((o - n * h.time_of_impact - p).dot(n)).abs() < PLANE
    })
}

/// An opening's corner: where it is, its wall's normal, and the way a crack leaves it (in the wall's plane, away
/// from the opening).
#[derive(Clone, Debug, PartialEq)]
pub struct Corner {
    pub at: Vec3,
    pub n: Vec3,
    pub away: Vec3,
}

/// The re-entrant corners of the walls' openings. A wall texel is beside one when, of the eight probes PROBE away
/// across and up the wall, only one diagonal misses the wall: the opening is that way, and the wall goes on along
/// both its edges. A building's outer corner (an edge missing too) or a pier between two openings isn't one.
pub fn corners(s: &Surface, pts: &[Point]) -> Vec<Corner> {
    let found: Vec<(Vec3, Vec3, Vec3)> = pts
        .par_iter()
        .filter(|q| standing(s, q))
        .filter_map(|q| {
            let t = s.up.cross(q.n).normalize_or_zero();
            let on = |d: Vec3| wall_at(s, q.p, q.n, q.p + d);
            let (x, y) = (t * PROBE, s.up * PROBE);
            if !(on(x) && on(-x) && on(y) && on(-y)) {
                return None;
            }
            let open: Vec<Vec3> = [x + y, x - y, -x + y, -x - y].into_iter().filter(|&d| !on(d)).collect();
            (open.len() == 1).then(|| (q.p, q.n, -open[0].normalize()))
        })
        .collect();
    // one corner per cluster: the texels nearest it see it alike; keep their middle
    let mut out: Vec<(Corner, u32)> = Vec::new();
    let mut found = found;
    found.sort_by_key(|f| raea_sys_touch::seed(f.0));
    for (p, n, away) in found {
        match out.iter_mut().find(|(c, _)| c.away.dot(away) > 0.99 && c.n.dot(n) > 0.95 && c.at.distance(p) < CORNER) {
            Some((c, k)) => {
                c.at = (c.at * *k as f32 + p) / (*k + 1) as f32;
                *k += 1;
            }
            None => out.push((Corner { at: p, n, away }, 1)),
        }
    }
    let mut out: Vec<Corner> = out.into_iter().map(|(c, _)| c).collect();
    out.sort_by(|a, b| a.at.x.total_cmp(&b.at.x).then(a.at.y.total_cmp(&b.at.y)).then(a.at.z.total_cmp(&b.at.z)));
    out
}

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn mix(h: u64, k: u64) -> u64 {
    let mut x = h ^ k.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^ (x >> 29)
}

/// A crack's path from a corner: steeper than its diagonal (a wall settles), each step turned up to 20 degrees
/// about that heading, stopping where the wall ends (another opening, the wall's edge) or at its length.
pub fn crack(s: &Surface, c: &Corner) -> Vec<Vec3> {
    let h = raea_sys_touch::seed(c.at);
    let len = 0.35 + 0.9 * unit(mix(h, 1));
    let side = c.away.dot(s.up).signum();
    let heading = (c.away + s.up * side * 0.45).normalize_or_zero();
    let across = c.n.cross(heading);
    let (mut p, mut run, mut k) = (c.at, 0.0, 0u64);
    let mut path = vec![p];
    while run < len {
        let a = (unit(mix(h, 10 + k)) - 0.5) * 0.7;
        let q = p + (heading * a.cos() + across * a.sin()) * STEP;
        if !wall_at(s, c.at, c.n, q) {
            break;
        }
        path.push(q);
        p = q;
        run += STEP;
        k += 1;
    }
    path
}

fn to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Whether the corner at `at` cracks (CRACKED in a hundred do).
fn cracked(at: Vec3) -> bool {
    raea_sys_touch::seed(at).rotate_right(29) % 100 < CRACKED
}

/// Each point's distance to the nearest crack on its own wall (same facing, within 5 cm of its plane), at most REACH;
/// with the corners the cracks start from.
pub fn cracks(s: &Surface, pts: &[Point]) -> (Vec<f32>, Vec<Corner>) {
    let cs: Vec<Corner> = corners(s, pts).into_iter().filter(|c| cracked(c.at)).collect();
    let paths: Vec<(Vec3, Vec<Vec3>, Vec3, Vec3)> = cs
        .iter()
        .map(|c| {
            let path = crack(s, c);
            let b = Aabb::from_points(path.iter().copied());
            (c.n, path, b.mins - Vec3::splat(REACH), b.maxs + Vec3::splat(REACH))
        })
        .collect();
    let d = pts
        .par_iter()
        .map(|q| {
            if !standing(s, q) {
                return REACH;
            }
            let mut best = REACH;
            for (n, path, lo, hi) in &paths {
                if n.dot(q.n) < 0.95 || (q.p - path[0]).dot(*n).abs() > 0.05 || q.p.cmplt(*lo).any() || q.p.cmpgt(*hi).any() {
                    continue;
                }
                for w in path.windows(2) {
                    best = best.min(to_segment(q.p, w[0], w[1]));
                }
            }
            best
        })
        .collect();
    (d, cs)
}

/// Where a heap from the foot of `q` lands: straight down from DEBRIS_OUT in front of it onto the walk, or nowhere
/// when the first thing below is something else (a step, a plinth, a sill).
fn landing(s: &Surface, q: &Point) -> Option<Vec3> {
    let from = q.p + q.n * DEBRIS_OUT;
    let h = s.mesh.cast_local_ray_and_get_normal(&Ray::new(from, -s.up), q.ground + DROP, false)?;
    s.walk[h.subshape as usize].then(|| from - s.up * h.time_of_impact)
}

/// Debris on the walking surface in front of outdoor plaster whose loss potential is high, one a GAP_DEBRIS apart,
/// each with the potential of the plaster above it.
pub fn debris(s: &Surface, pts: &[Point], crack: &[f32]) -> Vec<(Spot, f32)> {
    let c: Vec<(Spot, f32)> = pts
        .iter()
        .zip(crack)
        .filter(|(q, _)| standing(s, q) && s.mat[q.tri as usize] == Mat::Plaster && q.ground <= LOSS[1] && q.rain >= OUTDOORS)
        .map(|(q, &d)| (q, loss(q, d)))
        .filter(|(_, l)| *l >= DEBRIS_LOSS)
        .filter_map(|(q, l)| Some((Spot { what: "debris", by: "weather", at: landing(s, q)?, n: s.up, size: DEBRIS }, l)))
        .collect();
    let mut kept = raea_sys_touch::thin_by(c, GAP_DEBRIS, |x| x.0.at, |_| true);
    kept.sort_by(|a, b| a.0.at.x.total_cmp(&b.0.at.x).then(a.0.at.z.total_cmp(&b.0.at.z)));
    kept
}
