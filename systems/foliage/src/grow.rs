//! A tree's skeleton by space colonization (Runions, Lane and Prusinkiewicz 2007; docs/foliage.feature FO1).
//!   1. the crown's envelope: from the clear trunk to the top, a profile widest at `widest`, `square` its
//!      superellipse exponent, made lumpy by a few low waves round it; attraction points fill it, most in its outer
//!      shell, where light reaches
//!   2. a trunk rises, leaning a little, into the crown
//!   3. colonization: each point pulls the node nearest it within reach; each pulled node grows one step toward the
//!      mean of its pulls (bent by the species' tropism), and a point any node comes within kill of is used up.
//!      Each point keeps its nearest node and only the new nodes are checked against it, so a step costs the points
//!      times the new nodes near them, not the whole tree
//!   4. twigs sprout from the tips; thin shoots droop by the species' droop; the chains are smoothed
//!   5. radii by the pipe model, scaled so the trunk has the species' radius for its height

use std::collections::HashMap;

use parry3d::glamx::Vec3;

use crate::species::Species;
use crate::{Request, Rng};

/// A node of the skeleton: where, its parent (usize::MAX for the root), its radius, whether it's a twig.
#[derive(Clone, Debug)]
pub struct Node {
    pub p: Vec3,
    pub parent: usize,
    pub r: f32,
    pub twig: bool,
}

pub(crate) const NONE: usize = usize::MAX;
const MAX_NODES: usize = 80_000;
const MAX_STEPS: usize = 500;

/// The crown's envelope: its radius at a height and bearing.
pub(crate) struct Envelope {
    pub base: f32,
    pub top: f32,
    pub rad: f32,
    widest: f32,
    square: f32,
    low: f32,
    waves: [(f32, f32, f32); 3],
}

impl Envelope {
    pub(crate) fn new(sp: &Species, req: &Request, rng: &mut Rng) -> Envelope {
        let waves = [1.0f32, 2.0, 3.0].map(|k| (k, rng.range(0.0, std::f32::consts::TAU), sp.lumps * rng.range(0.4, 1.0) / k.sqrt()));
        Envelope { base: req.clear, top: req.height, rad: req.width / 2.0, widest: sp.widest, square: sp.square, low: sp.low, waves }
    }
    /// The crown's radius at height y on bearing a (radians); 0 outside its heights.
    pub(crate) fn r(&self, y: f32, a: f32) -> f32 {
        let t = (y - self.base) / (self.top - self.base);
        if !(0.0..=1.0).contains(&t) {
            return 0.0;
        }
        let s = if t < self.widest { (self.widest - t) / self.widest } else { (t - self.widest) / (1.0 - self.widest) };
        let e = if t < self.widest { self.low } else { self.square };
        let prof = (1.0 - s.powf(e)).max(0.0).powf(1.0 / e);
        let lump: f32 = self.waves.iter().map(|&(k, ph, amp)| amp * (k * a + ph + 2.0 * t).cos()).sum();
        self.rad * prof * (1.0 + lump)
    }
    pub(crate) fn centre(&self) -> Vec3 {
        Vec3::new(0.0, self.base + (self.top - self.base) * self.widest, 0.0)
    }
}

type Cell = (i32, i32, i32);

fn cell(p: Vec3, s: f32) -> Cell {
    ((p.x / s).floor() as i32, (p.y / s).floor() as i32, (p.z / s).floor() as i32)
}

pub(crate) fn grow(sp: &Species, req: &Request, rng: &mut Rng) -> (Vec<Node>, Envelope) {
    let env = Envelope::new(sp, req, rng);
    // a bigger crown takes bigger steps, so a street lime and a park plane cost about the same
    let step = sp.step * (env.rad / 5.0).clamp(0.6, 1.6);
    let (reach, kill) = (sp.reach * step, sp.kill * step);

    // 1. attraction points, in the crown's lobes and their bumps (lobes.rs)
    let volume = std::f32::consts::PI * env.rad * env.rad * (env.top - env.base) * 0.55;
    let want = ((sp.points * volume * (1.0 - (1.0 - sp.shell).powi(3))) as usize).clamp(400, 24_000);
    let crown = crate::lobes::lobes(sp, &env, rng);
    let mut pts: Vec<(Vec3, usize, f32)> = crate::lobes::points(sp, &env, &crown, want, rng).into_iter().map(|p| (p, NONE, f32::INFINITY)).collect();

    // 2. the trunk, on up the crown as a leader as far as the species carries one, wandering a little; then the big
    //    limbs a forking species splits into there, spread round and leaning out (colonization grows from all of it,
    //    so scaffold branches leave the leader at every height)
    let lean = Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)) * 0.035;
    let mut nodes = vec![Node { p: Vec3::ZERO, parent: NONE, r: 0.0, twig: false }];
    let into = env.base + (env.top - env.base) * sp.leader.max(0.12);
    let mut wander = Vec3::ZERO;
    while nodes.last().unwrap().p.y < into {
        wander = (wander + Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)) * 0.04) * 0.85;
        let d = (Vec3::Y + lean + wander).normalize();
        let last = nodes.len() - 1;
        nodes.push(Node { p: nodes[last].p + d * step, parent: last, r: 0.0, twig: false });
    }
    let fork = nodes.len() - 1;
    let spin = rng.range(0.0, std::f32::consts::TAU);
    for k in 0..sp.forks {
        let a = spin + std::f32::consts::TAU * (k as f32 + rng.range(-0.2, 0.2)) / sp.forks as f32;
        let tilt = rng.range(0.45, 0.8);
        let mut d = Vec3::new(a.cos() * tilt.sin(), tilt.cos(), a.sin() * tilt.sin());
        let mut at = fork;
        let len = (env.top - env.base) * rng.range(0.22, 0.32);
        for _ in 0..(len / step) as usize {
            d = (d + Vec3::Y * 0.04 + rng.unit() * 0.05).normalize();
            nodes.push(Node { p: nodes[at].p + d * step, parent: at, r: 0.0, twig: false });
            at = nodes.len() - 1;
        }
    }
    // each big lobe's scaffold limb, from the frame so far
    let frame: Vec<Vec3> = nodes.iter().map(|n| n.p).collect();
    for (p, parent) in crate::lobes::scaffolds(sp, &crown, &frame, step, rng) {
        nodes.push(Node { p, parent, r: 0.0, twig: false });
    }

    // 3. colonization
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for i in 1..nodes.len() {
        kids[nodes[i].parent].push(i);
    }
    let mut fresh: Vec<usize> = (0..nodes.len()).collect();
    let mut leader = fork;
    for _ in 0..MAX_STEPS {
        // each point's nearest node, from the nodes new since the last step
        let mut grid: HashMap<Cell, Vec<usize>> = HashMap::new();
        for &i in &fresh {
            grid.entry(cell(nodes[i].p, reach)).or_default().push(i);
        }
        for (p, near, dist) in pts.iter_mut() {
            let c = cell(*p, reach);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for &i in grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).into_iter().flatten() {
                            let d = (nodes[i].p - *p).length();
                            if d < *dist && d < reach {
                                (*near, *dist) = (i, d);
                            }
                        }
                    }
                }
            }
        }
        pts.retain(|&(_, _, d)| d > kill);
        if pts.is_empty() || nodes.len() >= MAX_NODES {
            break;
        }
        let mut pull: HashMap<usize, Vec3> = HashMap::new();
        for &(p, near, _) in &pts {
            if near != NONE {
                *pull.entry(near).or_insert(Vec3::ZERO) += (p - nodes[near].p).normalize();
            }
        }
        let mut order: Vec<_> = pull.into_iter().collect();
        order.sort_by_key(|&(i, _)| i);
        fresh.clear();
        // nothing in reach yet: the trunk climbs on
        if order.is_empty() && nodes[leader].p.y < env.top {
            let p = nodes[leader].p + (Vec3::Y + lean).normalize() * step;
            nodes.push(Node { p, parent: leader, r: 0.0, twig: false });
            kids.push(Vec::new());
            kids[leader].push(nodes.len() - 1);
            leader = nodes.len() - 1;
            fresh.push(leader);
            continue;
        }
        for (i, sum) in order {
            if sum.length() < 1e-3 {
                continue;
            }
            let d = (sum.normalize() + Vec3::Y * sp.up * 0.35 + rng.unit() * sp.crook * 0.5).normalize();
            let p = nodes[i].p + d * step;
            if kids[i].iter().any(|&k| (nodes[k].p - p).length() < step * 0.15) {
                continue;
            }
            nodes.push(Node { p, parent: i, r: 0.0, twig: false });
            kids.push(Vec::new());
            kids[i].push(nodes.len() - 1);
            fresh.push(nodes.len() - 1);
        }
        if fresh.is_empty() {
            break;
        }
    }

    // 4. twigs from the tips
    let tips: Vec<usize> = (1..nodes.len()).filter(|&i| kids[i].is_empty()).collect();
    for &t in &tips {
        let n = (sp.twigs + rng.f()) as usize;
        let dir = (nodes[t].p - nodes[nodes[t].parent].p).normalize_or_zero();
        for _ in 0..n {
            let mut at = t;
            let mut d = (dir * 0.6 + rng.unit() * 0.7 + Vec3::Y * sp.up * 0.3).normalize();
            for _ in 0..3 {
                let p = nodes[at].p + d * (sp.twig_len / 3.0) * rng.range(0.7, 1.2);
                nodes.push(Node { p, parent: at, r: 0.0, twig: true });
                kids.push(Vec::new());
                kids[at].push(nodes.len() - 1);
                at = nodes.len() - 1;
                d = (d + rng.unit() * 0.25).normalize();
            }
        }
    }

    radii(sp, req, &mut nodes, &kids);
    droop(sp, &mut nodes);
    smooth(&mut nodes, &kids, if sp.crook < 0.15 { 2 } else { 1 });
    (nodes, env)
}

/// The pipe model: a parent's radius^n is the sum of its children's. A model has far fewer tips than a real tree, so
/// one exponent can't give both a real twig and a real trunk: n is fitted per tree so the tips have the species' twig
/// radius and the trunk's foot its radius for the tree's height (clamped to 1.5-3.5, the trunk kept exact).
fn radii(sp: &Species, req: &Request, nodes: &mut [Node], kids: &[Vec<usize>]) {
    let mut a = vec![0f32; nodes.len()];
    for i in (0..nodes.len()).rev() {
        a[i] = if kids[i].is_empty() { 1.0 } else { kids[i].iter().map(|&k| a[k]).sum() };
    }
    let trunk = sp.trunk * req.height;
    let n = (a[0].ln() / (trunk / sp.tip).ln().max(1e-3)).clamp(1.5, 3.5);
    let k = trunk / a[0].powf(1.0 / n);
    for (node, a) in nodes.iter_mut().zip(&a) {
        node.r = k * a.powf(1.0 / n);
    }
}

/// Thin shoots hang: each segment turns down by droop x its length x how thin it is, carried out to its children.
fn droop(sp: &Species, nodes: &mut [Node]) {
    if sp.droop <= 0.0 {
        return;
    }
    let tip_r = nodes.iter().map(|n| n.r).fold(f32::INFINITY, f32::min);
    let orig: Vec<Vec3> = nodes.iter().map(|n| n.p).collect();
    for i in 1..nodes.len() {
        let par = nodes[i].parent;
        let off = orig[i] - orig[par];
        let len = off.length();
        if len < 1e-6 {
            continue;
        }
        let thin = (tip_r / nodes[i].r).powf(0.6);
        let turn = (sp.droop * len * thin).min(1.2);
        let d = (off / len - Vec3::Y * turn.tan()).normalize();
        nodes[i].p = nodes[par].p + d * len;
    }
}

/// Two passes of a chain's middles toward their neighbours (branch points, tips and the root held).
fn smooth(nodes: &mut [Node], kids: &[Vec<usize>], passes: usize) {
    for _ in 0..passes {
        let was: Vec<Vec3> = nodes.iter().map(|n| n.p).collect();
        for i in 1..nodes.len() {
            if let [k] = kids[i][..] {
                nodes[i].p = was[i] * 0.5 + (was[nodes[i].parent] + was[k]) * 0.25;
            }
        }
    }
}

/// Each node's children.
pub(crate) fn children(nodes: &[Node]) -> Vec<Vec<usize>> {
    let mut kids = vec![Vec::new(); nodes.len()];
    for (i, n) in nodes.iter().enumerate().skip(1) {
        kids[n.parent].push(i);
    }
    kids
}
