//! One drop's walk: down a face along gravity (or along its heading on a level face), over the edge into the next
//! face, along a corner where the next face rises, off an underside or a free edge to whatever is below. What the drop
//! picks up and leaves is summed into the atlas as it goes (Acc::line).

use std::collections::HashMap;

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};

use crate::{Acc, Drop, Kind, Surface, KINDS};

const NONE: u32 = u32::MAX;
/// How far a drop at the foot of an end face looks along the edge for the wall it meets (a sill's depth and more).
pub(crate) const CAPTURE: f32 = 0.25;
/// A face whose normal points down more than this sheds what reaches it.
const UNDERSIDE: f32 = -0.2;
/// Dirt picked up a metre run, per square metre of water; the run over which half of what's carried is left.
const PICK: f32 = 0.05;
const LEAVE: f32 = 1.2;
/// Water soaked into the surface: the run over which a drop keeps 1/e of itself.
const SOAK: f32 = 8.0;
/// What a drip keeps of itself when it lands.
const SPLASH: f32 = 0.7;
const STEPS: usize = 256;
/// How far a drop strays from the fall line, either side (ASSUMED: tears under sill ends widen about 1 in 7).
const MEANDER: f32 = 0.07;
/// A run is summed in pieces this long.
const PIECE: f32 = 0.25;
const EPS: f32 = 1e-4;

/// Across each triangle's edge k (corner k to k+1): the triangle that shares it, or NONE.
pub(crate) struct Adjacency(Vec<[u32; 3]>);

impl Adjacency {
    pub(crate) fn of(s: &Surface) -> Adjacency {
        // corners within a millimetre are one point, so primitives and boxes that touch edge to edge join
        let key = |v: Vec3| ((v.x * 1e3).round() as i64, (v.y * 1e3).round() as i64, (v.z * 1e3).round() as i64);
        let mut edges: HashMap<_, Vec<(u32, usize)>> = HashMap::new();
        for (t, c) in s.tri.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (key(s.pos[c[k] as usize]), key(s.pos[c[(k + 1) % 3] as usize]));
                edges.entry(if a < b { (a, b) } else { (b, a) }).or_default().push((t as u32, k));
            }
        }
        let mut adj = vec![[NONE; 3]; s.tri.len()];
        for list in edges.values() {
            // more than two on one edge (a fin, boxes meeting): each goes on to the one that bends it least
            for &(t, k) in list {
                let n = s.nrm[t as usize];
                let best = list.iter().filter(|(o, _)| *o != t).max_by(|a, b| n.dot(s.nrm[a.0 as usize]).total_cmp(&n.dot(s.nrm[b.0 as usize])).then(b.0.cmp(&a.0)));
                if let Some(&(o, _)) = best {
                    adj[t as usize][k] = o;
                }
            }
        }
        Adjacency(adj)
    }
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

fn corners(s: &Surface, t: u32) -> [Vec3; 3] {
    let c = s.tri[t as usize];
    [s.pos[c[0] as usize], s.pos[c[1] as usize], s.pos[c[2] as usize]]
}

/// Barycentric weights of p in t (p on its plane), and the rate each changes per metre along d.
fn bary(s: &Surface, t: u32, p: Vec3, d: Vec3) -> ([f32; 3], [f32; 3]) {
    let [a, b, c] = corners(s, t);
    let (v0, v1) = (b - a, c - a);
    let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
    let den = (d00 * d11 - d01 * d01).max(1e-12);
    let w = |v2: Vec3| {
        let (d20, d21) = (v2.dot(v0), v2.dot(v1));
        let (l1, l2) = ((d11 * d20 - d01 * d21) / den, (d00 * d21 - d01 * d20) / den);
        [l1, l2]
    };
    let [l1, l2] = w(p - a);
    let [r1, r2] = w(d);
    ([1.0 - l1 - l2, l1, l2], [-r1 - r2, r1, r2])
}

fn uv_at(s: &Surface, t: u32, l: [f32; 3]) -> [f32; 2] {
    let u = &s.uv[t as usize];
    [l[0] * u[0][0] + l[1] * u[1][0] + l[2] * u[2][0], l[0] * u[0][1] + l[1] * u[1][1] + l[2] * u[2][1]]
}

/// The way a drop goes on t: down the face, or along `heading` on a level one (None when it has nowhere to go).
fn downhill(s: &Surface, t: u32, up: Vec3, heading: Vec3) -> Option<Vec3> {
    let n = s.nrm[t as usize];
    let g = -up - n * n.dot(-up);
    if g.length() > 0.05 {
        return Some(g.normalize());
    }
    (heading - n * n.dot(heading)).try_normalize()
}

/// The first surface along a ray, its triangle and point, past `skip` metres.
fn cast(s: &Surface, o: Vec3, d: Vec3, far: f32) -> Option<(u32, Vec3)> {
    let h = s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, d), far, false)?;
    Some((h.subshape, o + d * h.time_of_impact))
}

/// One drop, walked till it ends; its water and dirt summed into `acc`.
pub(crate) fn drop(s: &Surface, adj: &Adjacency, acc: &Acc, up: Vec3, d0: &Drop, seed: u32) {
    let (mut t, mut p, mut water) = (d0.tri, d0.p, d0.water);
    let mut load = [0f32; KINDS];
    // a level face's heading, drawn once a drop: a sill's top sends its drops off every edge by the edges' share
    let a = hash(seed, 1) * std::f32::consts::TAU;
    let side = if up.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let (e1, e2) = (up.cross(side).normalize(), up.cross(up.cross(side)).normalize());
    let mut heading = e1 * a.cos() + e2 * a.sin();
    let mut along: Option<Vec3> = None;
    let wander = (hash(seed, 7) * 2.0 - 1.0) * MEANDER.tan();
    for _ in 0..STEPS {
        let r = s.role[t as usize];
        if r.walk || r.drains || water < 1e-7 {
            return;
        }
        let n = s.nrm[t as usize];
        if n.dot(up) < UNDERSIDE {
            match fall(s, p + n * 0.003, up) {
                Some((t2, q)) => (t, p, water, along) = (t2, q, water * SPLASH, None),
                None => return,
            }
            continue;
        }
        let Some(dir) = along.or_else(|| downhill(s, t, up, heading)) else { return };
        // drops wander off the fall line, each its own way, so a tear widens as it runs down (MEANDER either side)
        let dir = if along.is_none() && !level(s, t, up) { (dir + n.cross(dir) * wander).normalize() } else { dir };
        // the exit: the first edge the line p + s dir crosses
        let (l, dl) = bary(s, t, p, dir);
        let mut exit: Option<(f32, usize)> = None;
        for i in 0..3 {
            if dl[i] < -EPS {
                let sd = (-l[i] / dl[i]).max(0.0);
                if exit.is_none_or(|(b, _)| sd < b) {
                    exit = Some((sd, (i + 1) % 3));
                }
            }
        }
        let Some((len, edge)) = exit else { return };
        // a face carries on behind what stands on it (a wall behind its sills and string courses): what the run meets
        // head on first, just off the face, stops it there
        let o = p + n * 0.003;
        let met = cast(s, o, dir, len + 0.003).filter(|(t2, _)| *t2 != t && s.nrm[*t2 as usize].dot(dir) < -0.3);
        let (len, met) = match met {
            Some((t2, h)) => ((h - o).length().min(len), Some((t2, h + s.nrm[t2 as usize] * 0.001))),
            None => (len, None),
        };
        let q = p + dir * len;
        // what this run picks up, and what it leaves of what it carries
        // in pieces of PIECE, so a stain fades down the run rather than lying even along it
        let pieces = (len / PIECE).ceil().max(1.0) as usize;
        let at = |f: f32| [l[0] + dl[0] * len * f, l[1] + dl[1] * len * f, l[2] + dl[2] * len * f];
        for j in 0..pieces {
            let (f0, f1) = (j as f32 / pieces as f32, (j + 1) as f32 / pieces as f32);
            let step = len / pieces as f32;
            let mut left = [0f32; KINDS];
            if r.gives != Kind::None {
                load[r.gives as usize] += PICK * step * water;
            }
            let keep = (-step / LEAVE).exp();
            for k in 0..KINDS {
                left[k] = load[k] * (1.0 - keep);
                load[k] *= keep;
            }
            acc.line(uv_at(s, t, at(f0)), uv_at(s, t, at(f1)), water, &left);
            water *= (-step / SOAK).exp();
        }
        // over the edge
        let c = corners(s, t);
        let ev = (c[(edge + 1) % 3] - c[edge]).normalize_or_zero();
        let across = adj.0[t as usize][edge];
        let (next, head_on) = match met {
            Some(m) => (Some(m), true),
            None if across != NONE => (Some((across, q)), false),
            None => (probe(s, t, q, dir), true),
        };
        match next {
            Some((t2, q2)) => {
                let n2 = s.nrm[t2 as usize];
                if level(s, t2, up) && rises(s, t2, q2, up, heading) && !rises(s, t2, q2, up, -heading) {
                    heading = -heading; // a ledge met from the wall above it: the drop goes off its front, not back in
                }
                if n2.dot(up) < UNDERSIDE {
                    // about to drip: a wall met head on along this edge takes it into the corner, else it falls
                    match drip(s, t, q, n, dir, ev, up) {
                        Some((t3, q3)) => (t, p, water, along) = (t3, q3, water * SPLASH, None),
                        None => return,
                    }
                } else if (head_on && n.dot(up) > 0.3) || rises(s, t2, q2, up, heading) {
                    // the next face climbs away (a ledge meeting the wall above it): along the corner instead
                    let j = n.cross(n2).normalize_or_zero();
                    let sign = if j.dot(-up).abs() > 0.05 { j.dot(-up).signum() } else if j.dot(dir).abs() > 0.05 { j.dot(dir).signum() } else { 1.0 };
                    if along.is_some() {
                        return; // a corner's end in a corner: it pools there
                    }
                    along = Some(j * sign);
                    p = q - dir * 0.002;
                } else {
                    if along.is_some() && n.dot(n2) < 0.99 {
                        along = None;
                    }
                    (t, p) = (t2, q2 + downhill(s, t2, up, heading).unwrap_or(dir) * 0.001);
                }
            }
            None => match drip(s, t, q, n, dir, ev, up) {
                Some((t3, q3)) => (t, p, water, along) = (t3, q3, water * SPLASH, None),
                None => return,
            },
        }
    }
}

fn level(s: &Surface, t: u32, up: Vec3) -> bool {
    s.nrm[t as usize].dot(up) > 0.97
}

/// Whether a drop put on t at q would go straight back over the edge it came by (the face rises from it).
fn rises(s: &Surface, t: u32, q: Vec3, up: Vec3, heading: Vec3) -> bool {
    let Some(d) = downhill(s, t, up, heading) else { return true };
    let (l, dl) = bary(s, t, q, d);
    (0..3).any(|i| l[i] + dl[i] * 0.01 < -1e-3)
}

/// A free edge: a surface just past it, met head on (a sill's back edge against the wall behind it).
fn probe(s: &Surface, t: u32, q: Vec3, dir: Vec3) -> Option<(u32, Vec3)> {
    let n = s.nrm[t as usize];
    let (t2, h) = cast(s, q - dir * 0.01 + n * 0.002, dir, 0.04)?;
    (t2 != t && s.nrm[t2 as usize].dot(dir) < -0.3).then_some((t2, h + s.nrm[t2 as usize] * 0.001))
}

/// The foot of an end face about to drip: a wall along the edge within CAPTURE, met head on, takes the drop.
/// Where a drop leaving a face at `q` goes: into the corner of a wall met head on, else falling from just off the face.
fn drip(s: &Surface, t: u32, q: Vec3, n: Vec3, dir: Vec3, ev: Vec3, up: Vec3) -> Option<(u32, Vec3)> {
    capture(s, t, q, n, ev).or_else(|| fall(s, q + dir * 0.003 + n * 0.003, up))
}

fn capture(s: &Surface, t: u32, q: Vec3, n: Vec3, ev: Vec3) -> Option<(u32, Vec3)> {
    [ev, -ev].into_iter().filter_map(|e| {
        let (t2, h) = cast(s, q + n * 0.004, e, CAPTURE)?;
        let n2 = s.nrm[t2 as usize];
        (t2 != t && n2.dot(e) < -0.5).then(|| ((h - q).length(), t2, h + n2 * 0.002))
    }).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, t2, h)| (t2, h))
}

/// A drip falling straight down from o: what it lands on.
fn fall(s: &Surface, o: Vec3, up: Vec3) -> Option<(u32, Vec3)> {
    let (t, h) = cast(s, o, -up, 200.0)?;
    Some((t, h + up * 0.001))
}
