//! Plants grown from their species (docs/foliage.feature FO1-FO4): a request (a species, a height, a crown width, a
//! clear trunk and a seed) grows a tree by space colonization in the species' crown (grow.rs), gives its branches the
//! pipe model's radii and tubes (tube.rs), sets its leaves on its outer shoots as cards in eight groups by the order
//! they fall (leaves.rs), draws the species' leaf atlas by rules (atlas.rs) and writes one .glb (out.rs).
//!
//! Nothing here knows a season: a leaf's vertex colour carries its own random number, how exposed it is and how far
//! along its shoot it sits, and a surface hook draws the day over them (raea/foliage). The same request gives the
//! same bytes.

use parry3d::glamx::Vec3;

mod atlas;
mod grow;
mod leaves;
mod light;
mod lobes;
mod out;
mod roots;
mod shape;
pub mod species;
mod tube;
#[cfg(test)]
mod tests;

pub use atlas::{CELLS, atlas};
pub use grow::Node;
pub use shape::silhouette;
pub use species::{LeafKind, Species, species};

pub const VERSION: &str = "raea-sys-foliage 3";

/// The leaf groups: a leaf's group is its random number's eighth, so a group goes when its leaves have fallen.
pub const GROUPS: usize = 8;

/// What to grow. Sizes in metres; the height is the whole tree's, the clear trunk is up to the crown's base.
#[derive(Clone, Debug)]
pub struct Request {
    pub species: String,
    pub height: f32,
    pub width: f32,
    pub clear: f32,
    pub seed: u64,
    /// at most this many leaves (a level of detail's budget); the species' density is thinned to fit
    pub leaves: u32,
    /// the thinnest branch drawn as a tube (m; 0 all of them): a far level of detail drops its twigs and keeps its
    /// leaves where they were
    pub prune: f32,
}

/// Triangles with what the hooks read: position, normal, uv, a second uv (bark: the pattern's period round the
/// branch and the radius), a colour (bark: height, thinness; leaf: its random number, exposure, place on its shoot).
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub uv1: Vec<[f32; 2]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

impl Mesh {
    pub fn tris(&self) -> usize {
        self.idx.chunks_exact(3).len()
    }
}

/// A grown plant: its skeleton, its bark and its leaf groups.
pub struct Plant {
    pub nodes: Vec<Node>,
    pub bark: Mesh,
    pub leaves: Vec<Mesh>,
    /// its pit's radius (m from the trunk): the open soil its roots show in, for the ground to draw
    pub pit: f32,
}

impl Plant {
    pub fn leaf_count(&self) -> usize {
        self.leaves.iter().map(|m| m.idx.len() / 6).sum()
    }
}

/// Grow a plant.
pub fn grow(req: &Request) -> Result<Plant, String> {
    let sp = species(&req.species).ok_or_else(|| format!("no species {:?}; known: {}", req.species, species::ALL.iter().map(|s| s.id).collect::<Vec<_>>().join(", ")))?;
    if !(req.height > 1.0 && req.width > 0.5 && req.clear >= 0.0 && req.clear < req.height) {
        return Err(format!("a tree needs a height over 1 m, a width over 0.5 m and a clear trunk under its height, got {req:?}"));
    }
    let mut rng = Rng::new(req.seed ^ hash_str(sp.id));
    let (nodes, env) = grow::grow(sp, req, &mut rng);
    let mut bark = tube::bark(sp, req, &nodes);
    let pit = roots::foot(sp, req, &nodes, &mut bark);
    let cover = atlas::cover(&atlas(sp, req.seed)).max(0.05);
    let leaves = leaves::leaves(sp, req, &nodes, &env, cover, &mut rng);
    Ok(Plant { nodes, bark, leaves, pit })
}

/// The plant as a .glb: material "bark", and "leaf-0" to "leaf-7" over the species' atlas (alpha mask, both sides).
pub fn glb(req: &Request, plant: &Plant) -> Result<Vec<u8>, String> {
    let sp = species(&req.species).ok_or("no species")?;
    let png = atlas::png(&atlas(sp, req.seed))?;
    Ok(out::glb(sp, plant, &png))
}

/// A seeded stream (splitmix64): the same plant on every machine.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9e37_79b9_7f4a_7c15)
    }
    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// in [0, 1)
    pub(crate) fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub(crate) fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    /// a direction, evenly over the sphere
    pub(crate) fn unit(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let s = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(s * a.cos(), z, s * a.sin())
    }
}

pub(crate) fn hash_str(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// A unit vector across `d`.
pub(crate) fn across(d: Vec3) -> Vec3 {
    let t = if d.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    (t - d * t.dot(d)).normalize()
}
