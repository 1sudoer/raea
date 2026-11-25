//! The tree's foot (docs/foliage.feature FO13): the trunk flares where it meets the ground, ridged toward its
//! structural roots, and on the nearest level the roots run out from it: high where they leave the flare, a low hump
//! along the soil, then down into it.
//!
//! What's measured: a temperate tree has 5-15 structural roots that taper fast within 1-2 m of the trunk (Day et al.
//! 2010, Arboriculture & Urban Forestry 36(4), after Wilson 1964); an urban tree's trunk flare is 1.3-1.9 times its
//! diameter at breast height and its roots show 0.48-0.87 m out from it (Lee 2022, AUF 48(4), the flare after North
//! 2015 and Hilbert 2020). The rest is ASSUMED and named where it's set: how high the flare reaches, the furrows
//! between its ridges, a root's size, how high it stands by the trunk, its section (a buttress taller than wide by
//! the trunk, flattened further out).
//!
//! It draws from a stream of its own, so a level with roots and one without keep the same leaves.

use std::f32::consts::{PI, TAU};

use parry3d::glamx::Vec3;

use crate::grow::Node;
use crate::species::Species;
use crate::tube::{BARK, branches};
use crate::{Mesh, Request, Rng, hash_str};

/// How far the foot goes on under the ground (m), so it meets any ground with no edge showing
const DEPTH: f32 = 0.3;
/// How steeply a root dives once it's under the soil (m down a metre out)
const DIVE: f32 = 0.8;
/// The street tree's pit (m from the trunk's centre), the open soil its roots show in: 0.6 m past the flare and never
/// under 1 m (ASSUMED: a 2 m pit, wider round a big trunk). Past it the roots are under the paving. The plant carries
/// it (Plant::pit) for the ground to draw the same pit.
pub(crate) fn pit(sp: &Species, r_dbh: f32) -> f32 {
    (sp.flare * r_dbh + 0.6).max(1.0)
}

struct Root {
    a: f32,
    size: f32,
}

/// One root to draw: from `start` (on the ground) along `heading` (radians round y, as the flare's angle), `r0`
/// thick, its top `t0` above the soil where it starts and meeting the soil `show` metres out.
struct Run {
    start: Vec3,
    heading: f32,
    r0: f32,
    t0: f32,
    show: f32,
}

/// The foot onto the bark: the flare on every level, the roots on the nearest (a request with no pruning). Returns
/// the tree's pit.
pub(crate) fn foot(sp: &Species, req: &Request, nodes: &[Node], m: &mut Mesh) -> f32 {
    let Some(trunk) = branches(nodes).into_iter().next() else { return 1.0 };
    let r_dbh = nodes[trunk[0]].r;
    let pit = pit(sp, r_dbh);
    if trunk.len() < 2 || sp.roots == 0 {
        return pit;
    }
    let mut rng = Rng::new(req.seed ^ hash_str(sp.id) ^ hash_str("roots"));
    let n = sp.roots as usize;
    let roots: Vec<Root> = (0..n).map(|i| Root { a: TAU * (i as f32 + rng.range(-0.3, 0.3)) / n as f32, size: rng.range(0.55, 1.0) }).collect();
    flare(sp, req, nodes, &trunk, &roots, m);
    if req.prune > 0.0 {
        return pit;
    }
    let (c, _) = along_trunk(nodes, &trunk, 0.0);
    let mut forks = vec![];
    for root in &roots {
        let dir = Vec3::new(root.a.cos(), 0.0, -root.a.sin());
        let surf = (sp.surface * rng.range(0.6, 1.4)).clamp(0.05, 0.7);
        // half the trunk's radius at its biggest (ASSUMED); its top stands up to 1.3 times its half-height by the
        // trunk on the species that show their roots most, so even there it's buried a third deep and comes up out of
        // the soil rather than lying on it (ASSUMED; at twice it sat on the soil like a log, 11-17); it meets the soil 1.6-3 trunk radii out, longer
        // on those species (0.5-0.9 m on a 0.32 m radius, Lee 2022's range), and inside the pit, its widest side and
        // its wander clear of the paving's edge
        let r0 = 0.5 * r_dbh * root.size;
        let tall0 = r0 * ASPECT.0.sqrt();
        let inside = pit - 0.5 * r_dbh - r0 - 0.08;
        let run = Run {
            start: Vec3::new(c.x, 0.0, c.z) + dir * (0.5 * r_dbh),
            heading: root.a,
            r0,
            t0: (3.0 * surf * tall0).min(1.3 * tall0),
            show: (r_dbh * (1.6 + 1.4 * root.size) * (0.55 + surf) * rng.range(0.85, 1.15)).clamp(0.25, 1.4).min(inside.max(0.2)),
        };
        let fork = draw(m, &run, &mut rng);
        if root.size > 0.75 {
            forks.push(fork);
        }
    }
    // a big root forks once on its way out, the fork thinner and under the soil sooner
    for f in forks {
        let turn = if rng.f() < 0.5 { -1.0 } else { 1.0 } * rng.range(0.5, 0.9);
        draw(m, &Run { heading: f.heading + turn, r0: 0.6 * f.r0, show: f.show * rng.range(0.5, 0.8), ..f }, &mut rng);
    }
    pit
}

/// A root's section, height over width: where it leaves the trunk (a low buttress) and where it meets the soil (wider
/// than tall, as a surface root spreads under the paving; ASSUMED: at 1.8 and 0.8 they read as blades, 11-17)
const ASPECT: (f32, f32) = (1.25, 0.5);

/// One root: it halves its radius by where it meets the soil (ASSUMED, inside Day et al.'s fast taper), swelling and
/// pinching by a tenth along the way, its top falls off the flare, runs low along the soil and goes under at `show`,
/// then dives. It wanders a little. Returns
/// where a fork leaves it (two fifths of the way to the soil) with what's left of it there.
fn draw(m: &mut Mesh, r: &Run, rng: &mut Rng) -> Run {
    let len = r.show + 0.35;
    let steps = ((len / 0.05).ceil() as usize).clamp(6, 40);
    let x = |d: f32| (d / r.show).clamp(0.0, 1.4);
    // the swellings: two waves of 0.35 and 0.13 m, their phase the root's own (ASSUMED)
    let ph = rng.range(0.0, TAU);
    let rad = |d: f32| r.r0 * (1.0 - 0.5 * x(d)) * (1.0 + 0.07 * (d * 18.0 + ph).sin() + 0.04 * (d * 48.0 + 2.0 * ph).sin());
    let aspect = |d: f32| ASPECT.0 + (ASPECT.1 - ASPECT.0) * x(d).min(1.0);
    let tall = |d: f32| rad(d) * aspect(d).sqrt();
    let top = |d: f32| {
        let u = 1.0 - (d / r.show).min(1.0);
        r.t0 * (0.65 * u.powi(3) + 0.35 * u.powf(0.7)) - DIVE * (d - r.show).max(0.0)
    };
    let mut path = Vec::with_capacity(steps + 1);
    let (mut p, mut h) = (r.start, r.heading);
    for k in 0..=steps {
        let d = len * k as f32 / steps as f32;
        path.push((Vec3::new(p.x, top(d) - tall(d), p.z), d, h));
        h += rng.range(-1.0, 1.0) * 0.07;
        p += Vec3::new(h.cos(), 0.0, -h.sin()) * (len / steps as f32);
    }
    let sides = ((r.r0 * 80.0).round() as usize + 8).clamp(8, 16);
    let rep = (TAU * r.r0 / BARK).round().max(1.0);
    let mut along = 0.0;
    let at = (0.4 * r.show / len * steps as f32).round() as usize;
    let mut fork = Run { start: r.start, heading: r.heading, r0: r.r0, t0: r.t0, show: r.show };
    for (k, &(c, d, h)) in path.iter().enumerate() {
        let prev = path[k.saturating_sub(1)].0;
        let next = path[(k + 1).min(steps)].0;
        let t = (next - prev).normalize_or_zero();
        let up = (Vec3::Y - t * t.y).normalize_or_zero();
        let side = t.cross(up);
        if k > 0 {
            along += (c - prev).length();
        }
        if k == at {
            fork = Run { start: Vec3::new(c.x, 0.0, c.z), heading: h, r0: rad(d), t0: top(d), show: r.show - d };
        }
        let (rh, rw) = (tall(d), rad(d) / aspect(d).sqrt());
        let base = m.pos.len() as u32;
        for s in 0..=sides {
            let a = TAU * s as f32 / sides as f32;
            let q = c + up * (a.cos() * rh) + side * (a.sin() * rw);
            let n = (up * (a.cos() / rh) + side * (a.sin() / rw)).normalize_or_zero();
            m.pos.push(q.to_array());
            m.nrm.push(n.to_array());
            m.uv.push([rep * s as f32 / sides as f32, along / BARK]);
            m.uv1.push([rep, rad(d)]);
        }
        if k > 0 {
            quads(m, base - (sides as u32 + 1), base, sides as u32);
        }
    }
    fork
}

/// The trunk's centre and its tube's radius at height `y` (below the foot, straight down from it), as tube.rs rings it.
fn along_trunk(nodes: &[Node], trunk: &[usize], y: f32) -> (Vec3, f32) {
    let ring = |k: usize| if k == 0 && nodes[trunk[0]].r > nodes[trunk[1]].r { nodes[trunk[1]].r } else { nodes[trunk[k]].r };
    let foot = nodes[trunk[0]].p;
    if y <= foot.y {
        return (Vec3::new(foot.x, y, foot.z), ring(0));
    }
    for k in 1..trunk.len() {
        let (a, b) = (nodes[trunk[k - 1]].p, nodes[trunk[k]].p);
        if b.y >= y {
            let t = ((y - a.y) / (b.y - a.y).max(1e-6)).clamp(0.0, 1.0);
            return (a + (b - a) * t, ring(k - 1) + (ring(k) - ring(k - 1)) * t);
        }
    }
    let k = trunk.len() - 1;
    (nodes[trunk[k]].p, ring(k))
}

/// The flare: rings from under the ground to where it's the trunk again, its radius the trunk's grown by the species'
/// flare at the ground and falling off with height, ridged toward each root.
fn flare(sp: &Species, req: &Request, nodes: &[Node], trunk: &[usize], roots: &[Root], m: &mut Mesh) {
    let r_dbh = nodes[trunk[0]].r;
    let (sides, rings) = if req.prune <= 0.0 { (40, 16) } else if req.prune < 0.05 { (18, 8) } else { (10, 5) };
    // the flare falls to 1/e of itself in 0.15 m + 0.6 of the trunk's radius (0.34 m on a 0.32 m radius), ASSUMED,
    // and ends where it's under 3% of that
    let h = 0.15 + 0.6 * r_dbh;
    let top = 3.5 * h;
    let fall = |y: f32| (((-y.max(0.0) / h).exp() - (-top / h).exp()) / (1.0 - (-top / h).exp())).max(0.0);
    // a ridge toward each root, the furrows between at 0.3 of a ridge (ASSUMED), averaging 1 round the trunk; the
    // ridges are sharper low down, where they become the roots
    let ridge = |a: f32, y: f32| {
        let width = (0.22 + 0.25 * (y.max(0.0) / top)) * TAU / roots.len() as f32;
        let s: f32 = roots.iter().map(|r| {
            let d = (a - r.a + PI).rem_euclid(TAU) - PI;
            r.size * (-(d / width).powi(2)).exp()
        }).sum();
        0.3 + 0.7 * s.min(1.2)
    };
    // its top sits just inside the trunk's tube, whose faces are its rings' chords
    let st = ((r_dbh * 40.0).round() as usize + 3).clamp(3, 14);
    let inset = (PI / st as f32).cos();
    let rep = (TAU * r_dbh / BARK).round().max(1.0);
    let mut grid = Vec::with_capacity((rings + 1) * (sides + 1));
    let mut ys = Vec::with_capacity(rings + 1);
    for j in 0..=rings {
        // one ring under the ground, the rest closest where the flare turns fastest
        let y = if j == 0 { -DEPTH } else { top * ((j - 1) as f32 / (rings - 1) as f32).powf(1.6) };
        let (c, rt) = along_trunk(nodes, trunk, y);
        let mean = (0..90).map(|i| ridge(TAU * i as f32 / 90.0, y)).sum::<f32>() / 90.0;
        for s in 0..=sides {
            let a = TAU * s as f32 / sides as f32;
            let out = Vec3::new(a.cos(), 0.0, -a.sin());
            grid.push(c + out * rt * (inset + (sp.flare - inset) * fall(y) * ridge(a, y) / mean));
        }
        ys.push(y);
    }
    let at = |j: usize, s: usize| grid[j * (sides + 1) + s];
    let base = m.pos.len() as u32;
    for j in 0..=rings {
        for s in 0..=sides {
            let around = at(j, (s + 1) % sides) - at(j, (s + sides - 1) % sides);
            let up = at((j + 1).min(rings), s) - at(j.saturating_sub(1), s);
            let n = around.cross(up).normalize_or_zero();
            m.pos.push(at(j, s).to_array());
            m.nrm.push(if n == Vec3::ZERO { [0.0, 1.0, 0.0] } else { n.to_array() });
            m.uv.push([rep * s as f32 / sides as f32, ys[j] / BARK]);
            m.uv1.push([rep, r_dbh]);
        }
        if j > 0 {
            quads(m, base + ((j - 1) * (sides + 1)) as u32, base + (j * (sides + 1)) as u32, sides as u32);
        }
    }
}

/// The quads between two rings of `sides` + 1 vertices, wound as tube.rs winds them.
fn quads(m: &mut Mesh, lo: u32, hi: u32, sides: u32) {
    for s in 0..sides {
        let (a, b, c, d) = (lo + s, lo + s + 1, hi + s, hi + s + 1);
        m.idx.extend_from_slice(&[a, b, c, b, d, c]);
    }
}
