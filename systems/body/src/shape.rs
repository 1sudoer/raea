//! A solid thing (one to hold, or one of the scene's to keep out of), as a shape with a distance: a ball, a box, a capsule or a cylinder, placed and turned. One signed
//! distance does every job the hands need: where a ray from outside meets the surface (hold points), which way the
//! surface faces there (how the palm lies), and how near a finger is (where its curl stops).

use glam::{Quat, Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Solid {
    Ball { r: f32 },
    /// half its width, height and depth
    Box { half: Vec3 },
    /// a rod with rounded ends along its own y: half the straight part's length, and its radius
    Capsule { half: f32, r: f32 },
    /// a can along its own y: half its height, and its radius
    Cylinder { half: f32, r: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thing {
    pub solid: Solid,
    /// its centre and turn, in the frame the body is posed in
    pub at: Vec3,
    pub turn: Quat,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub at: Vec3,
    /// the way the surface faces there, outward
    pub normal: Vec3,
}

impl Solid {
    /// signed distance from a point in the solid's own frame: negative inside
    pub fn distance(&self, p: Vec3) -> f32 {
        match *self {
            Solid::Ball { r } => p.length() - r,
            Solid::Box { half } => {
                let q = p.abs() - half;
                q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
            }
            Solid::Capsule { half, r } => (p - Vec3::Y * p.y.clamp(-half, half)).length() - r,
            Solid::Cylinder { half, r } => {
                let d = Vec2::new(Vec2::new(p.x, p.z).length() - r, p.y.abs() - half);
                d.max(Vec2::ZERO).length() + d.max_element().min(0.0)
            }
        }
    }

    /// the radius of a ball about its centre that holds it
    pub fn bound(&self) -> f32 {
        match *self {
            Solid::Ball { r } => r,
            Solid::Box { half } => half.length(),
            Solid::Capsule { half, r } => half + r,
            Solid::Cylinder { half, r } => Vec2::new(half, r).length(),
        }
    }
}

impl Thing {
    pub fn new(solid: Solid, at: Vec3) -> Self {
        Thing { solid, at, turn: Quat::IDENTITY }
    }

    /// a point of the frame the body is in, in the thing's own
    pub fn local(&self, p: Vec3) -> Vec3 {
        self.turn.inverse() * (p - self.at)
    }

    /// a point of the thing's own frame, out in the body's
    pub fn world(&self, p: Vec3) -> Vec3 {
        self.at + self.turn * p
    }

    pub fn distance(&self, p: Vec3) -> f32 {
        self.solid.distance(self.local(p))
    }

    /// the way out of the surface nearest a point
    pub fn normal(&self, p: Vec3) -> Vec3 {
        let (q, e) = (self.local(p), 1e-3);
        let d = |o: Vec3| self.solid.distance(q + o);
        let g = Vec3::new(d(Vec3::X * e) - d(Vec3::X * -e), d(Vec3::Y * e) - d(Vec3::Y * -e), d(Vec3::Z * e) - d(Vec3::Z * -e));
        (self.turn * g).normalize_or(Vec3::Y)
    }

    /// Where a ray starting outside first meets the surface, within `far` metres (stepping by the distance).
    pub fn cast(&self, from: Vec3, dir: Vec3, far: f32) -> Option<Hit> {
        let mut t = 0.0;
        for _ in 0..96 {
            let p = from + dir * t;
            let d = self.distance(p);
            if d < 1e-4 {
                return Some(Hit { at: p, normal: self.normal(p) });
            }
            t += d;
            if t > far {
                return None;
            }
        }
        None
    }

    /// the eight corners of the box round it, where it is
    pub fn corners(&self) -> [Vec3; 8] {
        let half = match self.solid {
            Solid::Ball { r } => Vec3::splat(r),
            Solid::Box { half } => half,
            Solid::Capsule { half, r } => Vec3::new(r, half + r, r),
            Solid::Cylinder { half, r } => Vec3::new(r, half, r),
        };
        [0, 1, 2, 3, 4, 5, 6, 7].map(|i: usize| self.world(Vec3::new(if i & 1 == 0 { -half.x } else { half.x }, if i & 2 == 0 { -half.y } else { half.y }, if i & 4 == 0 { -half.z } else { half.z })))
    }

    /// the same thing, moved and turned with a frame it rides in
    pub fn moved(&self, at: Vec3, turn: Quat) -> Thing {
        Thing { solid: self.solid, at, turn }
    }
}
