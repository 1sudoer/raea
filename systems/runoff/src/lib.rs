//! Rain's run-off over a model's surfaces, after Dorsey, Pedersen and Hanrahan (1996, "Flow and Changes in
//! Appearance"): drops land where the rain reaches, run down each face along gravity, cross onto the next face over
//! shared edges, drip from the edges of undersides and fall to what's below, and carry what they pick up (dust off
//! a ledge's top, rust off iron, green off copper, white off zinc) down to where they leave it, as Chen et al.'s
//! gamma-tons (2005) do. Three rules from how fronts weather:
//!   - a drop at the bottom of a vertical face about to drip first looks along that edge for a wall it meets head
//!     on within CAPTURE (a ledge's end face meets the wall behind it): it clings into that corner and runs on down
//!     the wall. So a sill sheds most of its water clear off its front and streaks the wall under its two ends.
//!   - a face that drains (a roof, to its gutter) ends a drop at once; so does a walking surface (the ground).
//!   - a drop on a level face keeps one heading (from its seed) till it leaves the face: a sill's top sends some of
//!     its drops off the front and some off the ends, by the lengths of those edges.
//! Every drop is its own seeded walk and the sums are integer, so a run gives the same maps every time.

mod walk;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU64, Ordering};

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;
use rayon::prelude::*;

/// Bumped when a run's sums would change for the same input (a bake that uses them keys on it).
pub const VERSION: &str = "raea-sys-runoff 2";

/// What a drop picks up crossing a face, and leaves below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    None = 0,
    Grime = 1,
    Rust = 2,
    Copper = 3,
    Zinc = 4,
}

const KINDS: usize = 5;

/// Per triangle: what water picks up from it, and whether a drop ends on it.
#[derive(Clone, Copy, Debug)]
pub struct Role {
    pub gives: Kind,
    pub walk: bool,
    pub drains: bool,
}

/// The model: positions, triangles, their planes' normals and roles, and each triangle's corners in the atlas (texel
/// coordinates, texel i covering [i, i+1)).
pub struct Surface<'a> {
    pub pos: &'a [Vec3],
    pub tri: &'a [[u32; 3]],
    pub nrm: &'a [Vec3],
    pub role: &'a [Role],
    pub uv: &'a [[[f32; 2]; 3]],
    pub mesh: &'a TriMesh,
}

/// Where the rain lands: a point on a triangle, and how much water (square metres of rain caught).
#[derive(Clone, Copy, Debug)]
pub struct Drop {
    pub p: Vec3,
    pub tri: u32,
    pub water: f32,
}

/// The atlas: water passing each texel (square metres of rain caught upstream, per metre across), what it left there,
/// and the kind it left most of.
pub struct Out {
    pub flow: Vec<f32>,
    pub deposit: Vec<f32>,
    pub kind: Vec<Kind>,
}

impl Out {
    /// Texel i as bytes for a map, at `d` texels a metre: flow (water passing per metre across, log scale from 0.02 to
    /// 20 square metres a metre), deposit (left per square metre, log scale from 1e-4 to 0.1) and the kind (0, 60, 120,
    /// 180, 240 for none, grime, rust, copper, zinc).
    pub fn bytes(&self, i: usize, d: f32) -> [u8; 3] {
        let log = |v: f32, lo: f32, hi: f32| ((1.0 + v / lo).ln() / (1.0 + hi / lo).ln()).clamp(0.0, 1.0);
        let b = |v: f32| (v * 255.0).round() as u8;
        [b(log(self.flow[i] * d, 0.02, 20.0)), b(log(self.deposit[i] * d * d, 1e-4, 0.1)), self.kind[i] as u8 * 60]
    }
}

/// Fixed point for the sums: a billionth of a square metre (a drop's stain spread over a wall is that small).
const UNIT: f64 = 1e9;

pub(crate) struct Acc {
    w: u32,
    h: u32,
    flow: Vec<AtomicU64>,
    dep: Vec<[AtomicU64; KINDS]>,
}

impl Acc {
    fn new(w: u32, h: u32) -> Acc {
        let n = (w * h) as usize;
        Acc { w, h, flow: (0..n).map(|_| AtomicU64::new(0)).collect(), dep: (0..n).map(|_| std::array::from_fn(|_| AtomicU64::new(0))).collect() }
    }
    fn at(&self, uv: [f32; 2]) -> usize {
        let x = (uv[0].floor() as i64).clamp(0, self.w as i64 - 1) as usize;
        let y = (uv[1].floor() as i64).clamp(0, self.h as i64 - 1) as usize;
        y * self.w as usize + x
    }
    /// Water w over a segment from a to b (atlas coordinates): all of it passes each texel the segment crosses, and
    /// what it left is spread over them.
    pub(crate) fn line(&self, a: [f32; 2], b: [f32; 2], w: f32, left: &[f32; KINDS]) {
        let n = ((b[0] - a[0]).hypot(b[1] - a[1]) * 1.5).ceil().max(1.0) as usize;
        let mut last = usize::MAX;
        for i in 0..n {
            let t = (i as f32 + 0.5) / n as f32;
            let k = self.at([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            if k != last {
                self.flow[k].fetch_add((w as f64 * UNIT).round() as u64, Ordering::Relaxed);
                last = k;
            }
            for (j, &l) in left.iter().enumerate() {
                if l > 0.0 {
                    self.dep[k][j].fetch_add((l as f64 / n as f64 * UNIT).round() as u64, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Every drop walked; the atlas's sums.
pub fn run(s: &Surface, drops: &[Drop], up: Vec3, w: u32, h: u32) -> Out {
    let adj = walk::Adjacency::of(s);
    let acc = Acc::new(w, h);
    drops.par_iter().enumerate().for_each(|(i, d)| walk::drop(s, &adj, &acc, up, d, i as u32));
    let flow = acc.flow.iter().map(|v| (v.load(Ordering::Relaxed) as f64 / UNIT) as f32).collect();
    let (mut deposit, mut kind) = (Vec::with_capacity(acc.dep.len()), Vec::with_capacity(acc.dep.len()));
    for d in &acc.dep {
        let v: [f32; KINDS] = std::array::from_fn(|j| (d[j].load(Ordering::Relaxed) as f64 / UNIT) as f32);
        deposit.push(v.iter().sum());
        let best = (1..KINDS).max_by(|&a, &b| v[a].total_cmp(&v[b]).then(b.cmp(&a))).unwrap();
        kind.push(if v[best] > 0.0 { [Kind::None, Kind::Grime, Kind::Rust, Kind::Copper, Kind::Zinc][best] } else { Kind::None });
    }
    Out { flow, deposit, kind }
}
