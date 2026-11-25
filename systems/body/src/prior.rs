//! What a hand looks like: SOMA-X's hand articulation prior (NVlabs/SOMA-X, Apache-2.0; data/README.md), a
//! principal-component model of real hand poses, carried onto raea's fifteen finger joints. It gives a hand a
//! shape from a few numbers (`shape`: none of them is the average relaxed hand, the first opens and closes it) and
//! says how far a hand's pose is from the hands people make (`oddness`). SOMA's poses are turns from its flat bind
//! hand in the wrist's axes; here each finger bone is pointed where SOMA's would point, in raea's hand's frame, from
//! raea's own rest hand, which is a little curled.

use std::sync::OnceLock;

use glam::{Mat3, Quat, Vec3};

use crate::rig::{FINGERS, Pose, Side, Skeleton, hand_rest};

const JOINTS: usize = 15;
const DIMS: usize = JOINTS * 3;
pub const COMPONENTS: usize = 32;

struct Table {
    mean: Vec<f32>,
    sd: Vec<f32>,
    /// COMPONENTS rows of DIMS
    components: Vec<f32>,
    back: Vec<f32>,
    dirs: Vec<Vec3>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let bytes = include_bytes!("../data/soma_hand_prior.bin");
        let mut all = Vec::with_capacity(bytes.len() / 4);
        for four in bytes.as_chunks::<4>().0 {
            all.push(f32::from_le_bytes(*four));
        }
        let (mean, rest) = all.split_at(DIMS);
        let (sd, rest) = rest.split_at(COMPONENTS);
        let (components, rest) = rest.split_at(COMPONENTS * DIMS);
        let (back, dirs) = rest.split_at(COMPONENTS * DIMS);
        Table { mean: mean.into(), sd: sd.into(), components: components.into(), back: back.into(), dirs: dirs.chunks_exact(3).map(|d| Vec3::new(d[0], d[1], d[2]).normalize()).collect() }
    })
}

/// A hand of the rig set against SOMA's: its finger joints in the prior's order, the turn from SOMA's wrist axes
/// to the hand's rest frame, and for each joint the turn from its rest bone to where SOMA's bind bone points.
struct Fitted {
    joints: [usize; JOINTS],
    frame: Quat,
    to_bind: [Quat; JOINTS],
}

/// the left hand's (the prior is the left's; a right hand is read mirrored)
fn fitted(skel: &Skeleton, s: Side) -> Option<Fitted> {
    let rest = hand_rest(skel, s)?;
    let mirror = |v: Vec3| if s == Side::R { Vec3::new(-v.x, v.y, v.z) } else { v };
    // SOMA's left hand at bind: fingers along +x, the palm facing +z
    let (along, palm) = (mirror(rest.along), mirror(rest.palm));
    let frame = Quat::from_mat3(&(Mat3::from_cols(along, palm, along.cross(palm)) * Mat3::from_cols(Vec3::X, Vec3::Z, Vec3::X.cross(Vec3::Z)).transpose())).normalize();
    let mut joints = [0; JOINTS];
    let mut to_bind = [Quat::IDENTITY; JOINTS];
    for (f, name) in FINGERS.iter().enumerate() {
        for (k, j) in skel.finger(name, s)?.into_iter().enumerate() {
            let bone = mirror((skel.joints[j].tail - skel.joints[j].head).normalize());
            joints[f * 3 + k] = j;
            to_bind[f * 3 + k] = Quat::from_rotation_arc(bone, frame * table().dirs[f * 3 + k]);
        }
    }
    Some(Fitted { joints, frame, to_bind })
}

/// a turn as the left's sees it, and back: mirrored across the body's middle
fn mirrored(q: Quat, s: Side) -> Quat {
    if s == Side::R { Quat::from_xyzw(q.x, -q.y, -q.z, q.w) } else { q }
}

/// A hand's finger turns for coefficients `z` (each in its own standard deviations; missing ones are 0): written into
/// `turns` for the side's fifteen finger joints.
pub fn shape(skel: &Skeleton, turns: &mut [Quat], s: Side, z: &[f32]) -> Option<()> {
    let (t, hand) = (table(), fitted(skel, s)?);
    let mut pose = t.mean.clone();
    for (i, zi) in z.iter().take(COMPONENTS).enumerate() {
        for (p, c) in pose.iter_mut().zip(&t.components[i * DIMS..(i + 1) * DIMS]) {
            *p += zi * t.sd[i] * c;
        }
    }
    for f in 0..5 {
        // down the finger: SOMA's turns so far, and the bone turned with them from where its bind bone points
        let (mut soma, mut above) = (Quat::IDENTITY, Quat::IDENTITY);
        for k in 0..3 {
            let i = f * 3 + k;
            let turn = Quat::from_scaled_axis(Vec3::new(pose[i * 3], pose[i * 3 + 1], pose[i * 3 + 2]));
            soma = (soma * (hand.frame * turn * hand.frame.inverse())).normalize();
            let world = (soma * hand.to_bind[i]).normalize();
            turns[hand.joints[i]] = mirrored((above.inverse() * world).normalize(), s);
            above = world;
        }
    }
    Some(())
}

/// A hand's pose read as the prior's coefficients, and what of it they don't account for (degrees, root mean square
/// over the joints).
pub fn coefficients(skel: &Skeleton, pose: &Pose, s: Side) -> Option<(Vec<f32>, f32)> {
    let (t, hand) = (table(), fitted(skel, s)?);
    let mut seen = vec![0.0f32; DIMS];
    for f in 0..5 {
        let (mut soma, mut above) = (Quat::IDENTITY, Quat::IDENTITY);
        for k in 0..3 {
            let i = f * 3 + k;
            let world = (above * mirrored(pose.turns[hand.joints[i]], s)).normalize();
            let so_far = (world * hand.to_bind[i].inverse()).normalize();
            let turn = hand.frame.inverse() * (soma.inverse() * so_far) * hand.frame;
            let v = turn.to_scaled_axis();
            seen[i * 3..i * 3 + 3].copy_from_slice(&[v.x, v.y, v.z]);
            (soma, above) = (so_far, world);
        }
    }
    let off: Vec<f32> = seen.iter().zip(&t.mean).map(|(a, b)| a - b).collect();
    let z: Vec<f32> = (0..COMPONENTS).map(|i| t.back[i * DIMS..(i + 1) * DIMS].iter().zip(&off).map(|(a, b)| a * b).sum()).collect();
    let mut left = off;
    for (i, zi) in z.iter().enumerate() {
        for (l, c) in left.iter_mut().zip(&t.components[i * DIMS..(i + 1) * DIMS]) {
            *l -= zi * t.sd[i] * c;
        }
    }
    let rest = (left.iter().map(|v| v * v).sum::<f32>() / JOINTS as f32).sqrt().to_degrees();
    Some((z, rest))
}

/// How far a hand's pose is from the hands people make (the coefficients' root mean square, plus what they leave
/// over, a deviation for each 10 degrees): 0 for the average hand. It is a comparison, not a verdict: the rig's own
/// rest hand reads about 2, since its bones aren't SOMA's, and fingers bent back at the middle joint read over 4.
pub fn oddness(skel: &Skeleton, pose: &Pose, s: Side) -> Option<f32> {
    let (z, rest) = coefficients(skel, pose, s)?;
    Some((z.iter().map(|v| v * v).sum::<f32>() / COMPONENTS as f32).sqrt() + rest / 10.0)
}
