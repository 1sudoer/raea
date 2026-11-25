//! Where the rules say a thing belongs, each with who put it there (docs/wear.feature WR3). The sizes are ASSUMED
//! until a source is read:
//!   - bills (the bill poster): on plain walls (no opening within PLAIN) the rain doesn't reach, 0.5-2.5 m up,
//!     facing open ground, GAP_BILL apart;
//!   - street plates (the city): on a building's corner, 2.5-3 m up, one on each face, where the arris runs from the
//!     walking surface to 3.5 m and both faces look onto open ground;
//!   - nameplates (the tenant): beside a single door's lock side, 1.5 m up, on the wall.
//! Debris under lost plaster is raea-sys-damage's (WR6), by the loss potential raea/wear's recipes use.
//! Candidates are taken in the order of a seed from their place, each kept if none kept is nearer than its gap.

use std::collections::BTreeMap;

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};

use crate::bands::Door;
use crate::{Mat, Point, Surface, seed};

/// A thing a rule says belongs here: what, who put it there, its middle, the face it's on, its size (across, up).
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub what: &'static str,
    pub by: &'static str,
    pub at: Vec3,
    pub n: Vec3,
    pub size: [f32; 2],
}

const BILL: [f32; 2] = [0.6, 0.85];
const GAP_BILL: f32 = 3.0;
/// a bill's wall has no opening this far across either way
const PLAIN: f32 = 3.0;
const PLATE: [f32; 2] = [0.6, 0.18];
/// an arris's points within this across belong to one corner; plates on one face of a corner this far apart at least
const CORNER: f32 = 0.4;
const GAP_PLATE: f32 = 2.0;
const NAMEPLATE: [f32; 2] = [0.3, 0.12];
/// open ground in front: a ray out of the face runs this far free
const OPEN: f32 = 2.0;
/// a ray that runs this far free reaches the sky
const SKY_FAR: f32 = 200.0;

fn standing(s: &Surface, q: &Point) -> bool {
    q.n.dot(s.up).abs() < 0.2
}

fn open(s: &Surface, p: Vec3, n: Vec3, far: f32) -> bool {
    s.mesh.cast_local_ray(&Ray::new(p + n * 0.01, n), far, false).is_none()
}

/// How many of a fixed fan of rays out of a face reach the sky (none hit within SKY_FAR): three elevations by three
/// bearings across the face's front, and straight up.
fn sky(s: &Surface, p: Vec3, n: Vec3) -> usize {
    let across = s.up.cross(n).normalize_or_zero();
    let mut dirs = vec![s.up];
    for el in [15f32, 40.0, 65.0] {
        for az in [-60f32, 0.0, 60.0] {
            let (el, az) = (el.to_radians(), az.to_radians());
            dirs.push((n * az.cos() + across * az.sin()) * el.cos() + s.up * el.sin());
        }
    }
    dirs.iter().filter(|&&d| s.mesh.cast_local_ray(&Ray::new(p + n * 0.05, d), SKY_FAR, false).is_none()).count()
}

/// The face at p is plain wall for PLAIN across each way and from 0.5 m below to 2.5 m above: every ray into it meets
/// plaster or masonry within 0.15 m of its plane (a door, a window, a sill or a recess there is an opening).
fn plain(s: &Surface, p: Vec3, n: Vec3) -> bool {
    let (t, plane) = (s.up.cross(n).normalize_or_zero(), p.dot(n));
    let steps = (PLAIN / 0.5) as i32;
    (-steps..=steps).all(|i| {
        (0..7).all(|j| {
            let o = p + t * (i as f32 * 0.5) + s.up * (j as f32 * 0.5 - 0.5) + n * 0.5;
            s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -n), 0.8, false).is_some_and(|h| {
                matches!(s.mat[h.subshape as usize], Mat::Plaster | Mat::Masonry) && ((o - n * h.time_of_impact).dot(n) - plane).abs() < 0.15
            })
        })
    })
}

/// Keep candidates in seed order, none nearer than `gap` to one kept.
fn thin(c: Vec<Spot>, gap: f32) -> Vec<Spot> {
    thin_if(c, gap, |_| true)
}

/// As thin, keeping only candidates that pass `ok` (tested only on those the gap leaves).
fn thin_if(c: Vec<Spot>, gap: f32, ok: impl Fn(&Spot) -> bool) -> Vec<Spot> {
    thin_by(c, gap, |s| s.at, ok)
}

/// Candidates of any kind kept in seed order of where each is (`at`), none nearer than `gap` to one kept, and only
/// those that pass `ok` (tested only on those the gap leaves): thin's rule for what carries a spot (raea-sys-damage's
/// debris with its loss).
pub fn thin_by<T>(mut c: Vec<T>, gap: f32, at: impl Fn(&T) -> Vec3, ok: impl Fn(&T) -> bool) -> Vec<T> {
    c.sort_by_cached_key(|x| seed(at(x)));
    let mut kept: Vec<T> = Vec::new();
    for x in c {
        if !kept.iter().any(|k| at(k).distance(at(&x)) < gap) && ok(&x) {
            kept.push(x);
        }
    }
    kept
}

pub(crate) fn spots(s: &Surface, doors: &[Door], pts: &[Point]) -> Vec<Spot> {
    let wall = |q: &Point| matches!(s.mat[q.tri as usize], Mat::Plaster | Mat::Masonry) && standing(s, q);
    let mut out = Vec::new();

    // bills: centred 0.95-2.05 m up, so the sheet stays within 0.5-2.5 m, on a plain wall (a firewall, a lot's wall, a
    // hoarding): Berlin's columns (Litfaß, from 1855) were brought in to keep bills off house fronts
    let bills = pts.iter().filter(|q| wall(q) && q.rain < 0.35 && (0.95..=2.05).contains(&q.ground) && open(s, q.p, q.n, OPEN));
    let bills: Vec<Spot> = bills.map(|q| Spot { what: "bill", by: "bill poster", at: q.p, n: q.n, size: BILL }).collect();
    out.extend(thin_if(bills, GAP_BILL, |b| plain(s, b.at, b.n)));

    // street plates: on a corner whose arris runs up from the walking surface past 3.5 m (the arris points within
    // CORNER across the level plane fill every 0.5 m of height to there), where a second face of the corner has a
    // candidate too and both look onto open ground; one per face of each corner
    let (e1, e2) = s.up.any_orthonormal_pair();
    let flat = |p: Vec3| (p.dot(e1), p.dot(e2));
    let cell = |p: Vec3| ((flat(p).0 / CORNER).floor() as i64, (flat(p).1 / CORNER).floor() as i64);
    let mut arris: BTreeMap<(i64, i64), Vec<&Point>> = BTreeMap::new();
    for q in pts.iter().filter(|q| wall(q) && q.edge > 0.8 && q.ground < 4.0) {
        arris.entry(cell(q.p)).or_default().push(q);
    }
    const NINE: [(i64, i64); 9] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 0), (0, 1), (1, -1), (1, 0), (1, 1)];
    let near = |p: Vec3| {
        let (c, (x, y)) = (cell(p), flat(p));
        NINE.iter().map(move |&(i, j)| (c.0 + i, c.1 + j)).filter_map(|k| arris.get(&k)).flatten().filter(move |q| {
            let (a, b) = flat(q.p);
            (a - x).hypot(b - y) <= CORNER
        })
    };
    let tall = |p: Vec3| {
        let bins: std::collections::BTreeSet<i64> = near(p).map(|q| (q.ground * 2.0).floor() as i64).collect();
        (0..7).all(|b| bins.contains(&b))
    };
    let cands: Vec<&Point> = arris.values().flatten().copied().filter(|q| (2.5..=3.0).contains(&q.ground) && tall(q.p) && open(s, q.p, q.n, OPEN * 2.0)).collect();
    let mut by_face: BTreeMap<(i64, i64, i64), Vec<Spot>> = BTreeMap::new();
    for q in cands.iter().filter(|q| cands.iter().any(|r| r.n.dot(q.n) < 0.5 && (r.p - q.p).length() <= CORNER * 2.0)) {
        let k = ((q.n.x * 4.0).round() as i64, (q.n.y * 4.0).round() as i64, (q.n.z * 4.0).round() as i64);
        by_face.entry(k).or_default().push(Spot { what: "street plate", by: "city", at: q.p, n: q.n, size: PLATE });
    }
    for spots in by_face.into_values() {
        out.extend(thin(spots, GAP_PLATE));
    }

    // nameplates: beside a single leaf's lock edge, on whichever side of the leaf looks onto open ground
    for d in doors.iter().filter(|d| !d.paired) {
        let out_of = if d.lock >= d.hi - 1e-4 { 1.0 } else { -1.0 };
        let beside = d.foot + d.across * (d.lock - d.foot.dot(d.across) + out_of * (0.05 + NAMEPLATE[0] / 2.0 + 0.1)) + s.up * 1.5;
        // the outside: of the two wall faces beside the leaf, the one that sees more sky (a hall sees none; a
        // balcony or a hood over a street door hides only part of it)
        let face = |side: Vec3| {
            let o = beside + side * 1.0;
            let h = s.mesh.cast_local_ray_and_get_normal(&Ray::new(o, -side), 1.5, false)?;
            let (n, at) = (s.nrm[h.subshape as usize], o - side * h.time_of_impact);
            let ok = n.dot(side) > 0.8 && matches!(s.mat[h.subshape as usize], Mat::Plaster | Mat::Masonry) && open(s, at, n, OPEN);
            ok.then(|| (sky(s, at, n), at, n))
        };
        let best = [d.facing, -d.facing].into_iter().filter_map(face).max_by(|a, b| a.0.cmp(&b.0));
        if let Some((_, at, n)) = best.filter(|b| b.0 > 0) {
            out.push(Spot { what: "nameplate", by: "tenant", at, n, size: NAMEPLATE });
        }
    }

    out.sort_by(|a, b| a.what.cmp(b.what).then(a.at.x.total_cmp(&b.at.x)).then(a.at.y.total_cmp(&b.at.y)).then(a.at.z.total_cmp(&b.at.z)));
    out
}
