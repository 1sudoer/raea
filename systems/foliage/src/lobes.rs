//! The crown's masses (docs/foliage.feature FO1): a broadleaf crown seen from a street is a few clouds of foliage,
//! each the leafy end of one scaffold limb, with sky in the notches between them, and each cloud is itself bumped by
//! the shoots on its outer face. So the crown is drawn as big lobes set round its envelope (the species' profile, now
//! only the crown's reach), each carrying smaller bumps on the side that faces out, and the attraction points that
//! grow the branches are put in the bumps' outer shells, where light reaches, with a few through each lobe's inside.

use std::f32::consts::TAU;

use parry3d::glamx::Vec3;

use crate::grow::Envelope;
use crate::species::Species;
use crate::Rng;

/// A cloud of foliage: its centre, its radius across and how much taller than wide it is.
/// how far apart two big lobes' centres are kept, of their radii summed: under 1 they overlap, so the crown is one
/// mass with notches, not separate balls
const APART: f32 = 0.48;

#[derive(Clone, Debug)]
pub(crate) struct Lobe {
    pub c: Vec3,
    pub r: f32,
    pub tall: f32,
}

impl Lobe {
    /// where p is in the lobe: 0 at its centre, 1 on its surface
    fn depth(&self, p: Vec3) -> f32 {
        let d = p - self.c;
        Vec3::new(d.x, d.y / self.tall, d.z).length() / self.r
    }
}

/// The big lobes, and each one's bumps.
pub(crate) fn lobes(sp: &Species, env: &Envelope, rng: &mut Rng) -> Vec<(Lobe, Vec<Lobe>)> {
    // more lobes on a bigger crown and up a taller one (a poplar's column is a stack of them)
    let column = ((env.top - env.base) / (2.0 * env.rad)).max(1.0);
    let n = (sp.lobes * (env.rad / 5.0).sqrt().clamp(0.7, 1.5) * column).round().max(1.0) as usize;
    let tall = (sp.height_per_width / 1.4).clamp(0.85, 1.7);
    let lean = Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)).normalize_or_zero() * sp.skew * env.rad;
    let h = env.top - env.base;
    let mut big: Vec<Lobe> = Vec::new();
    // the top first: every crown has one
    let r = sp.lobe * env.rad * rng.range(0.8, 1.05);
    big.push(Lobe { c: Vec3::new(0.0, env.top - r * tall * 0.85, 0.0) + lean, r, tall });
    let mut tries = 0;
    while big.len() < n && tries < n * 80 {
        tries += 1;
        let r = sp.lobe * env.rad * rng.range(0.7, 1.15);
        let t = rng.range(0.1, 0.85);
        let a = rng.range(0.0, TAU);
        let y = env.base + h * t;
        let rr = env.r(y, a);
        // more lobes where the crown is broad
        if rr < r * 0.5 || rng.f() > rr / env.rad {
            continue;
        }
        let d = (rr - r * rng.range(0.55, 0.9)).max(0.0);
        let c = Vec3::new(a.cos() * d, y.clamp(env.base + r * tall * 0.45, env.top - r * tall * 0.5), a.sin() * d) + lean * t;
        // apart enough that a notch shows between neighbours
        if big.iter().any(|l| (Vec3::new(l.c.x - c.x, (l.c.y - c.y) / tall, l.c.z - c.z)).length() < (l.r + r) * APART) {
            continue;
        }
        big.push(Lobe { c, r, tall });
    }
    let core = env.centre();
    big.into_iter()
        .map(|l| {
            let out = Vec3::new(l.c.x - core.x, (l.c.y - core.y) * 0.6 + 0.3 * l.r, l.c.z - core.z).normalize_or(Vec3::Y);
            // enough bumps to cover the lobe's outer face (each a disc of about half the lobe's radius)
            let m = 12 + (rng.f() * 7.0) as usize;
            let bumps = (0..m)
                .filter_map(|_| {
                    let d = (out * 0.9 + rng.unit()).normalize_or(out);
                    if d.dot(out) < -0.1 {
                        return None;
                    }
                    let r = l.r * rng.range(0.38, 0.62);
                    let c = l.c + Vec3::new(d.x, d.y * l.tall, d.z) * (l.r - r * rng.range(0.2, 0.6));
                    Some(Lobe { c, r, tall: l.tall })
                })
                .collect();
            (l, bumps)
        })
        .collect()
}

/// `want` attraction points: in the bumps' outer shells, by their size, few on a lobe's side facing the crown's
/// core (it's shaded); `interior` of them through the big lobes; none under the clear trunk or far out of the
/// envelope.
pub(crate) fn points(sp: &Species, env: &Envelope, crown: &[(Lobe, Vec<Lobe>)], want: usize, rng: &mut Rng) -> Vec<Vec3> {
    let all: Vec<(&Lobe, &Lobe)> = crown.iter().flat_map(|(l, bs)| bs.iter().map(move |b| (l, b))).collect();
    let total: f32 = all.iter().map(|(_, b)| b.r * b.r).sum();
    let core = env.centre();
    let mut pts = Vec::with_capacity(want);
    let mut tries = 0;
    while pts.len() < want && tries < want * 20 && !all.is_empty() {
        tries += 1;
        let mut pick = rng.f() * total;
        let &(big, bump) = all.iter().find(|(_, b)| {
            pick -= b.r * b.r;
            pick <= 0.0
        }).unwrap_or(&all[all.len() - 1]);
        let p = if rng.f() < sp.interior {
            let u = rng.unit() * rng.f().cbrt();
            big.c + Vec3::new(u.x, u.y * big.tall, u.z) * big.r
        } else {
            let u = rng.unit();
            let out = (bump.c - core).normalize_or(Vec3::Y);
            if u.dot(out) < -0.2 && rng.f() < 0.75 {
                continue;
            }
            bump.c + Vec3::new(u.x, u.y * bump.tall, u.z) * bump.r * (1.0 - sp.shell * rng.f())
        };
        let reach = env.r(p.y.clamp(env.base, env.top), p.z.atan2(p.x)).max(0.3);
        if p.y < env.base + 0.2 || p.x.hypot(p.z) > reach * 1.2 + 0.5 || p.y > env.top + 0.5 {
            continue;
        }
        pts.push(p);
    }
    pts
}

/// Each big lobe's scaffold limb: from the frame grown so far (trunk, leader, fork limbs), from the node nearest a
/// point on the axis below the lobe (so limbs leave at about 50 degrees from upright), toward the lobe's centre,
/// turning a little each step, stopping inside it. Returns the new nodes as (position, parent).
pub(crate) fn scaffolds(sp: &Species, crown: &[(Lobe, Vec<Lobe>)], frame: &[Vec3], step: f32, rng: &mut Rng) -> Vec<(Vec3, usize)> {
    let mut out: Vec<(Vec3, usize)> = Vec::new();
    for (l, _) in crown {
        let across = l.c.x.hypot(l.c.z);
        let from = Vec3::new(0.0, l.c.y - across * 0.85, 0.0);
        let Some(start) = (0..frame.len()).filter(|&i| frame[i].y < l.c.y - 0.3 * l.r).min_by(|&a, &b| (frame[a] - from).length().total_cmp(&(frame[b] - from).length())) else {
            continue;
        };
        let (mut p, mut at) = (frame[start], start);
        let mut d = ((l.c - p).normalize_or(Vec3::Y) + Vec3::Y * 0.3).normalize();
        for _ in 0..400 {
            if l.depth(p) < 0.4 {
                break;
            }
            d = (d * 0.75 + (l.c - p).normalize_or(Vec3::Y) * 0.25 + rng.unit() * sp.crook).normalize();
            p += d * step;
            out.push((p, at));
            at = frame.len() + out.len() - 1;
        }
    }
    out
}
