//! The bark: each branch one tube (docs/foliage.feature FO1). A branch follows its thickest child at each node; the
//! other children start branches of their own, each from its parent's centre so it grows out of the parent's tube.
//! Rings are carried along a branch by parallel transport, so a tube doesn't twist; a branch's sides are fewer as it
//! is thinner (a twig is three sided). The bark hook reads:
//!   uv   round the branch, in whole repeats of BARK m of circumference (so its noise wraps), and along it in BARK m
//!   uv1  the repeats round (the noise's period) and the radius
//! No vertex colour: the engine multiplies a material by it, so a model drawn without the hook would go dark.

use parry3d::glamx::Vec3;

use crate::grow::{Node, children};
use crate::species::Species;
use crate::{Mesh, Request, across};

pub(crate) const BARK: f32 = 0.3;

/// Each branch's nodes in order, its parent's node first (none for the trunk).
pub(crate) fn branches(nodes: &[Node]) -> Vec<Vec<usize>> {
    let kids = children(nodes);
    let mut out = Vec::new();
    let mut starts = vec![(usize::MAX, 0usize)];
    while let Some((from, s)) = starts.pop() {
        let mut chain = if from == usize::MAX { vec![] } else { vec![from] };
        let mut at = s;
        loop {
            chain.push(at);
            let Some(&main) = kids[at].iter().max_by(|&&a, &&b| nodes[a].r.total_cmp(&nodes[b].r).then(b.cmp(&a))) else { break };
            for &k in kids[at].iter().filter(|&&k| k != main) {
                starts.push((at, k));
            }
            at = main;
        }
        out.push(chain);
    }
    out
}

pub(crate) fn bark(_sp: &Species, req: &Request, nodes: &[Node]) -> Mesh {
    let mut m = Mesh::default();
    for chain in branches(nodes) {
        // a pruned level ends each branch where it gets thinner than the prune (radii only thin along a branch)
        let keep = chain.iter().skip(1).take_while(|&&i| nodes[i].r >= req.prune).count() + 1;
        let chain = &chain[..keep.min(chain.len())];
        if chain.len() < 2 {
            continue;
        }
        // a side branch's first ring (at its parent) has its own first node's radius
        let first = if nodes[chain[0]].parent == usize::MAX || chain.len() < 2 { chain[0] } else { chain[1] };
        let r0 = nodes[first].r;
        let sides = ((r0 * 40.0).round() as usize + 3).clamp(3, 14);
        let rep = ((std::f32::consts::TAU * r0 / BARK).round()).max(1.0);
        let mut frame = Vec3::ZERO;
        let mut along = 0.0;
        for (k, &i) in chain.iter().enumerate() {
            let p = nodes[i].p;
            let prev = if k > 0 { nodes[chain[k - 1]].p } else { p };
            let next = if k + 1 < chain.len() { nodes[chain[k + 1]].p } else { p };
            let t = (next - prev).normalize_or_zero();
            let t = if t == Vec3::ZERO { Vec3::Y } else { t };
            frame = if k == 0 { across(t) } else { let f = (frame - t * frame.dot(t)).normalize_or_zero(); if f == Vec3::ZERO { across(t) } else { f } };
            let side = t.cross(frame);
            if k > 0 {
                along += (p - prev).length();
            }
            let r = if k == 0 && chain.len() > 1 && nodes[chain[0]].r > nodes[chain[1]].r { nodes[chain[1]].r } else { nodes[i].r };
            let base = m.pos.len() as u32;
            for s in 0..=sides {
                let a = std::f32::consts::TAU * s as f32 / sides as f32;
                let n = frame * a.cos() + side * a.sin();
                let q = p + n * r;
                m.pos.push(q.to_array());
                m.nrm.push(n.to_array());
                m.uv.push([rep * s as f32 / sides as f32, along / BARK]);
                m.uv1.push([rep, r]);
            }
            if k > 0 {
                let prev_ring = base - (sides as u32 + 1);
                for s in 0..sides as u32 {
                    let (a, b, c, d) = (prev_ring + s, prev_ring + s + 1, base + s, base + s + 1);
                    m.idx.extend_from_slice(&[a, b, c, b, d, c]);
                }
            }
        }
    }
    m
}
