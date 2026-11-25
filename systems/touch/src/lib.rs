//! Where hands, feet and wheels wear a model, and where things belong on it (docs/wear.feature WR3), by rules over its
//! triangles' materials and heights, never placed by hand (bands.rs):
//!   - a door leaf (wood, standing on the walking surface, 0.7-1.6 m wide, 1.9 m or more tall, under 0.12 m thick):
//!     a handle band at 0.95-1.1 m on its lock side and a kick band at 0-0.3 m;
//!   - a tread (a level face of a step, 0.2-0.45 m deep, 0.6-4 m wide, in a flight or a single step at the foot):
//!     worn at its middle;
//!   - a rail (iron or wood, at least 0.5 m long, under 0.12 m across, 0.75-1.2 m up): its top;
//!   - walls standing under 3 m apart (a passage): the shoulder band, 1.2-1.6 m;
//!   - walls 2.2-4.5 m apart under a ceiling (a gateway): hub height, 0.4-0.9 m.
//! The lock side is where two leaves meet (a pair); a single leaf's is picked by a seed from its place, as the model
//! says nothing of hinges (ASSUMED). Spots (spots.rs) are where the rules say a thing belongs, each with who put it
//! there. Pure: the same input gives the same output.

mod bands;
mod spots;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;

pub use spots::{Spot, thin_by};

/// Bumped when the output would change for the same input (a bake that uses it keys on it).
pub const VERSION: &str = "raea-sys-touch 5";

/// What a triangle is made of, as far as touch goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mat {
    Plaster,
    /// brick, sandstone, granite
    Masonry,
    Wood,
    /// iron
    Metal,
    Other,
}

/// The model: positions, triangles, their planes' normals, materials and which are walking surfaces, the BVH over
/// them (its subshapes are the triangles) and which way is up.
pub struct Surface<'a> {
    pub pos: &'a [Vec3],
    pub tri: &'a [[u32; 3]],
    pub nrm: &'a [Vec3],
    pub mat: &'a [Mat],
    pub walk: &'a [bool],
    pub mesh: &'a TriMesh,
    pub up: Vec3,
}

/// A point to mark (an atlas texel): where it is, its face's normal and triangle, and what the other cause maps say
/// there: the drop to the walking surface below (metres), the rain's reach (0-1), the run-off passing (0-1, as the
/// flow map stores it) and the nearness to a convex arris (0-1).
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub p: Vec3,
    pub n: Vec3,
    pub tri: u32,
    pub ground: f32,
    pub rain: f32,
    pub flow: f32,
    pub edge: f32,
}

/// Touch at each point (0-1) and the spots, from the rules.
pub fn mark(s: &Surface, pts: &[Point]) -> (Vec<f32>, Vec<Spot>) {
    let parts = Parts::of(s);
    let doors = bands::doors(s, &parts);
    let touch = bands::touch(s, &parts, &doors, pts);
    let spots = spots::spots(s, &doors, pts);
    (touch, spots)
}

/// The parts: triangles of one material joined by shared corners (welded at a millimetre).
pub(crate) struct Parts {
    pub of: Vec<u32>,
    pub n: usize,
}

impl Parts {
    pub fn of(s: &Surface) -> Parts {
        let mut up: Vec<u32> = (0..s.tri.len() as u32).collect();
        fn root(up: &mut [u32], mut i: u32) -> u32 {
            while up[i as usize] != i {
                up[i as usize] = up[up[i as usize] as usize];
                i = up[i as usize];
            }
            i
        }
        let mut first: HashMap<(i64, i64, i64, Mat), u32> = HashMap::new();
        for (t, tri) in s.tri.iter().enumerate() {
            for &v in tri {
                let q = s.pos[v as usize] * 1000.0;
                let k = (q.x.round() as i64, q.y.round() as i64, q.z.round() as i64, s.mat[t]);
                let f = *first.entry(k).or_insert(t as u32);
                let (a, b) = (root(&mut up, f), root(&mut up, t as u32));
                if a != b {
                    up[a.max(b) as usize] = a.min(b);
                }
            }
        }
        let mut ids = HashMap::new();
        let of = (0..s.tri.len() as u32).map(|t| {
            let r = root(&mut up, t);
            let n = ids.len() as u32;
            *ids.entry(r).or_insert(n)
        }).collect();
        Parts { of, n: ids.len() }
    }

    /// Each part's triangles.
    pub fn tris(&self) -> Vec<Vec<u32>> {
        let mut out = vec![Vec::new(); self.n];
        for (t, &p) in self.of.iter().enumerate() {
            out[p as usize].push(t as u32);
        }
        out
    }
}

/// A seed from a place, the same on every machine (the lock side of a single leaf; raea-sys-damage's cracks).
pub fn seed(p: Vec3) -> u64 {
    let q = |v: f32| (v * 100.0).round() as i64 as u64;
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in [q(p.x), q(p.y), q(p.z)] {
        h = (h ^ v).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// 1 inside [a, b], easing to 0 over `soft` outside it.
pub(crate) fn band(v: f32, a: f32, b: f32, soft: f32) -> f32 {
    let d = (a - v).max(v - b).max(0.0);
    (1.0 - d / soft).clamp(0.0, 1.0)
}
