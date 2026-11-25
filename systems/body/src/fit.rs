//! A pose fitted to points: joint turns recovered from where joints (or marks on the body) were seen, so motion that
//! arrives as positions (a capture, a tracker, another rig's joints, a model's output) plays on raea's rig. This is
//! the analytical pass of SOMA-X's pose inversion (NVlabs/SOMA-X, Apache-2.0, soma/fitting/pose_inversion.py), on
//! marks instead of mesh vertices: joints are refitted parents first, each turned to best align everything it
//! carries with where that was seen, the root moved onto its own marks, for a few passes. The alignment is
//! found by turning toward the answer from the turn the joint has (Mueller et al., "A robust method to extract the
//! rotational part of deformations"), so a joint the marks say nothing about (a twist about a bone) keeps its turn.

use glam::{Mat3, Quat, Vec3};

use crate::rig::{Pose, Skeleton};

/// A point of the body seen somewhere: on `joint`, `offset` from its head in the rest pose's axes, seen at `at`.
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub joint: usize,
    pub offset: Vec3,
    pub at: Vec3,
    pub weight: f32,
}

/// the turn that best carries the vectors `from` onto `to` (weights in `w`): the rotation nearest their covariance
fn align(pairs: &[(Vec3, Vec3, f32)]) -> Quat {
    let mut cov = Mat3::ZERO;
    for &(a, b, w) in pairs {
        cov += Mat3::from_cols(b * a.x, b * a.y, b * a.z) * w;
    }
    let mut q = Quat::IDENTITY;
    for _ in 0..100 {
        let r = Mat3::from_quat(q);
        let omega = (r.x_axis.cross(cov.x_axis) + r.y_axis.cross(cov.y_axis) + r.z_axis.cross(cov.z_axis))
            / ((r.x_axis.dot(cov.x_axis) + r.y_axis.dot(cov.y_axis) + r.z_axis.dot(cov.z_axis)).abs() + 1e-9);
        let angle = omega.length();
        if angle < 1e-7 {
            break;
        }
        q = (Quat::from_axis_angle(omega / angle, angle) * q).normalize();
    }
    q
}

/// The pose that puts the marks where they were seen, starting from `from`, and the marks' root-mean-square miss.
pub fn fit(skel: &Skeleton, from: &Pose, marks: &[Mark], passes: usize) -> (Pose, f32) {
    let n = skel.joints.len();
    // how many joints down from `j` a mark's joint is (0: on it), if `j` carries it
    let below = |j: usize, mark: usize| {
        let (mut i, mut depth) = (Some(mark), 0);
        while let Some(k) = i {
            if k == j {
                return Some(depth);
            }
            (i, depth) = (skel.joints[k].parent, depth + 1);
        }
        None
    };
    let seen = |pose: &Pose| -> Vec<Vec3> {
        let w = skel.fk(pose);
        marks.iter().map(|m| w.pos[m.joint] + w.rot[m.joint] * m.offset).collect()
    };
    let mut pose = from.clone();
    for _ in 0..passes {
        // the root is put where its own marks were seen (any marks, when it has none)
        let now = seen(&pose);
        let own = |i: &usize| marks[*i].joint == 0;
        let by: Vec<usize> = if (0..marks.len()).any(|i| own(&i)) { (0..marks.len()).filter(own).collect() } else { (0..marks.len()).collect() };
        let total: f32 = by.iter().map(|&i| marks[i].weight).sum();
        if total > 0.0 {
            pose.root += by.iter().map(|&i| (marks[i].at - now[i]) * marks[i].weight).sum::<Vec3>() / total;
        }
        for j in 0..n {
            let (w, now) = (skel.fk(&pose), seen(&pose));
            // what a joint's turn alone decides counts in full: marks on it, and where its children's heads are. What
            // lies further down also depends on the joints between, so it counts little: enough to settle a twist
            // about a single bone, not enough to bend this joint for a fault further down
            let pairs: Vec<(Vec3, Vec3, f32)> = (0..marks.len())
                .filter_map(|i| {
                    let depth = below(j, marks[i].joint)?;
                    let rigid = depth == 0 || (depth == 1 && marks[i].offset.length_squared() < 1e-10);
                    Some((now[i] - w.pos[j], marks[i].at - w.pos[j], marks[i].weight * if rigid { 1.0 } else { 1e-5 }))
                })
                .collect();
            if pairs.iter().all(|p| p.0.length_squared() < 1e-10) {
                continue;
            }
            pose.turns[j] = skel.local_for(&w, j, align(&pairs) * w.rot[j]);
        }
    }
    let now = seen(&pose);
    let miss = (marks.iter().zip(&now).map(|(m, p)| (m.at - *p).length_squared()).sum::<f32>() / marks.len().max(1) as f32).sqrt();
    (pose, miss)
}

/// Marks for every joint's head and bone end, as a pose draws them: what a capture of joint positions gives.
pub fn marks_of(skel: &Skeleton, seen: &Pose) -> Vec<Mark> {
    let w = skel.fk(seen);
    let mut out = Vec::with_capacity(skel.joints.len() * 2);
    for (i, j) in skel.joints.iter().enumerate() {
        out.push(Mark { joint: i, offset: Vec3::ZERO, at: w.pos[i], weight: 1.0 });
        out.push(Mark { joint: i, offset: j.tail - j.head, at: w.pos[i] + w.rot[i] * (j.tail - j.head), weight: 1.0 });
    }
    out
}
