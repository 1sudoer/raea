//! What a point on a model is exposed to through the year (docs/wear.feature WR9): the sun on chosen days, weather
//! driven along a cone, and how sheltered it is where blown things settle. The weathering bake builds causes0's sun
//! and causes3 on it, so a season or a day's weather is drawn over them at render time and never baked:
//!   sun     the share of the sun's path on the days asked (every `step` hours it stands 3 degrees up) the point
//!           sees, weighted by n.s: the whole year's for what fades and dries, December's for where snow outlasts a thaw
//!   drift   driven snow's reach: rays up its cone (its axis the way the snow comes from), each seen one counting
//!           n.d, over what an open level roof gets; a wall facing a storm gets as much as a roof, one in its lee none
//!   litter  where blown leaves and snow settle, on faces looking up: hemmed in close by (low rays out across the
//!           face meeting something within LITTER_REACH: a kerb's gutter, a step's corner, a sill against its window)
//!           and in the lee (rays toward the wind meeting something within LEE_REACH)
//! The weights of litter's two parts are ASSUMED until a snow or leaf count against a photograph says otherwise.
//! Samples are a rotated R2 sequence per point, so the result is the same on every run.

use std::f32::consts::TAU;

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};
use parry3d::shape::TriMesh;

#[cfg(test)]
mod tests;

pub const VERSION: &str = "raea-sys-exposure 2";

/// Days of the year (1-366) for the whole year's sun: the middle of each month.
pub const YEAR: [f32; 12] = [15.0, 46.0, 74.0, 105.0, 135.0, 166.0, 196.0, 227.0, 258.0, 288.0, 319.0, 349.0];
/// Days for December's sun, either side of the solstice: where snow and frost outlast a thaw.
pub const DECEMBER: [f32; 3] = [335.0, 355.0, 15.0];

const OFF: f32 = 0.01;
const FAR: f32 = 400.0;
const LITTER_REACH: f32 = 1.5;
const LEE_REACH: f32 = 3.0;
/// litter = ENCLOSED x hemmed in + LEE x in the lee (ASSUMED)
const ENCLOSED: f32 = 0.7;
const LEE: f32 = 0.3;

/// The model's frame: up, north, and its latitude in degrees.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub up: Vec3,
    pub north: Vec3,
    pub lat: f32,
}

/// Weather driven along a cone: the way it travels (a unit vector, down and with the wind) and the cone's half-angle
/// in radians.
#[derive(Clone, Copy, Debug)]
pub struct Driven {
    pub dir: Vec3,
    pub spread: f32,
}

/// What one point is exposed to, each 0-1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Exposure {
    pub sun: f32,
    pub december: f32,
    pub drift: f32,
    pub litter: f32,
}

/// The sun's directions on `days`, every `step` hours it stands 3 degrees up.
pub fn sun_path(f: &Frame, days: &[f32], step: f32) -> Vec<Vec3> {
    let east = f.north.cross(f.up).normalize();
    let phi = f.lat.to_radians();
    let mut out = Vec::new();
    for &n in days {
        let dec = (23.44f32).to_radians() * (TAU * (284.0 + n) / 365.0).sin();
        let mut t: f32 = 0.0;
        while t < 24.0 {
            let ha = (15.0 * (t - 12.0)).to_radians();
            let alt = (phi.sin() * dec.sin() + phi.cos() * dec.cos() * ha.cos()).asin();
            if alt > 3f32.to_radians() {
                let az = (-ha.sin()).atan2(dec.tan() * phi.cos() - phi.sin() * ha.cos());
                out.push((alt.cos() * (az.cos() * f.north + az.sin() * east) + alt.sin() * f.up).normalize());
            }
            t += step;
        }
    }
    out
}

/// Everything a point's exposure needs, worked out once for a model.
pub struct Exposer<'a> {
    mesh: &'a TriMesh,
    up: Vec3,
    year: Vec<Vec3>,
    december: Vec<Vec3>,
    snow: Driven,
    /// the snow's rays and what a level open roof gets of them
    rays: u32,
    level: f32,
}

impl<'a> Exposer<'a> {
    pub fn new(mesh: &'a TriMesh, f: &Frame, snow: Driven, rays: u32) -> Exposer<'a> {
        let rays = rays.max(1);
        let from = -snow.dir;
        let level = (0..rays).map(|j| cone(from, snow.spread, sample(0, j)).dot(f.up).max(0.0)).sum::<f32>() / rays as f32;
        Exposer { mesh, up: f.up, year: sun_path(f, &YEAR, 2.0), december: sun_path(f, &DECEMBER, 1.0), snow, rays, level }
    }

    fn hit(&self, o: Vec3, d: Vec3, far: f32) -> bool {
        self.mesh.cast_local_ray(&Ray::new(o, d), far, false).is_some()
    }

    fn lit(&self, o: Vec3, n: Vec3, path: &[Vec3]) -> f32 {
        if path.is_empty() {
            return 0.0;
        }
        path.iter().map(|&s| if n.dot(s) > 0.0 && !self.hit(o, s, FAR) { n.dot(s) } else { 0.0 }).sum::<f32>() / path.len() as f32
    }

    /// Point `i` (its index seeds its samples) at p, facing n.
    pub fn at(&self, i: u32, p: Vec3, n: Vec3) -> Exposure {
        let o = p + n * OFF;
        let from = -self.snow.dir;
        let mut drift = 0.0;
        for j in 0..self.rays {
            let d = cone(from, self.snow.spread, sample(i, j));
            if n.dot(d) > 0.0 && !self.hit(o, d, FAR) {
                drift += n.dot(d);
            }
        }
        let drift = if self.level > 0.0 { drift / self.rays as f32 / self.level } else { 0.0 };
        Exposure { sun: self.lit(o, n, &self.year), december: self.lit(o, n, &self.december), drift: drift.min(1.0), litter: self.litter(o, n) }
    }

    /// On a face looking up: low rays out across it every 45 degrees, and toward the wind, meeting something near.
    fn litter(&self, o: Vec3, n: Vec3) -> f32 {
        let facing = n.dot(self.up);
        if facing < 0.5 {
            return 0.0;
        }
        let (t, b) = across(self.up);
        let low = (10f32).to_radians();
        let hemmed = (0..8)
            .filter(|&k| {
                let a = TAU * k as f32 / 8.0;
                let d = (t * a.cos() + b * a.sin()) * low.cos() + self.up * low.sin();
                self.hit(o, d, LITTER_REACH)
            })
            .count() as f32
            / 8.0;
        let wind = -self.snow.dir;
        let level = (wind - self.up * wind.dot(self.up)).normalize_or_zero();
        let lee = if level == Vec3::ZERO {
            0.0
        } else {
            let side = self.up.cross(level);
            let fan = [(-20f32, 5f32), (0.0, 5.0), (20.0, 5.0), (0.0, 15.0), (0.0, 25.0)];
            fan.iter()
                .filter(|&&(az, el)| {
                    let (az, el) = (az.to_radians(), el.to_radians());
                    let d = (level * az.cos() + side * az.sin()) * el.cos() + self.up * el.sin();
                    self.hit(o, d, LEE_REACH)
                })
                .count() as f32
                / fan.len() as f32
        };
        ((ENCLOSED * hemmed + LEE * lee) * smooth((facing - 0.5) / 0.3)).clamp(0.0, 1.0)
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// The j-th of point i's samples: an R2 sequence rotated by a hash of i.
fn sample(i: u32, j: u32) -> (f32, f32) {
    const A1: f32 = 0.754_877_7;
    const A2: f32 = 0.569_840_3;
    let h = hash(i.wrapping_add(0x9e37_79b9));
    let (o1, o2) = ((h & 0xffff) as f32 / 65536.0, (h >> 16) as f32 / 65536.0);
    ((o1 + A1 * j as f32).fract(), (o2 + A2 * j as f32).fract())
}

fn across(n: Vec3) -> (Vec3, Vec3) {
    let t = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let t = (t - n * t.dot(n)).normalize();
    (t, n.cross(t))
}

/// A direction within `half` radians of `axis`, uniform over that cap, from two uniform numbers.
pub fn cone(axis: Vec3, half: f32, (u1, u2): (f32, f32)) -> Vec3 {
    let c = 1.0 - u1 * (1.0 - half.cos());
    let s = (1.0 - c * c).max(0.0).sqrt();
    let (t, b) = across(axis);
    (axis * c + t * (s * (TAU * u2).cos()) + b * (s * (TAU * u2).sin())).normalize()
}
