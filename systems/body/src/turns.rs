//! A motion given as turns, posed on a SOMA body. Kimodo (and anything else that speaks SOMA's skeleton in turns)
//! says how each joint is turned in the world from SOMA's T-pose. A raea body made from SOMA's rig (rig "soma")
//! has those joints under SOMA's names, bound in SOMA's bind pose, so nothing is fitted: a joint's world turn from
//! bind is the motion's turn after the turn from bind to the T-pose, a joint the motion doesn't name rides its
//! parent, and each limb segment's four twist joints take SOMA's shares of the turn about the segment (arm.rs).

use std::collections::HashMap;

use glam::{Quat, Vec3};

use crate::arm::{twist_about, twist_joints};
use crate::rig::{Pose, Skeleton};

/// SOMA's limb segments that carry twist joints: the joint each hangs from, the joint at its far end, and whether
/// it is an upper segment (it undoes its own joint's twist) or a lower one (it takes up its far joint's)
const SEGMENTS: [(&str, &str, bool); 4] = [("Arm", "ForeArm", true), ("ForeArm", "Hand", false), ("Leg", "Shin", true), ("Shin", "Foot", false)];

/// the signed angle of a turn about a unit axis: its twist, radians
fn angle_about(q: Quat, axis: Vec3) -> f32 {
    let t = twist_about(q, axis);
    2.0 * Vec3::new(t.x, t.y, t.z).dot(axis).atan2(t.w)
}

/// Sets a SOMA body's twist joints (`{segment}Twist1` to 4) in `world`, every joint's world turn from bind, from
/// the turns of the joints at each segment's two ends. A body without twist joints is left as it is.
pub fn set_twists(skel: &Skeleton, world: &mut [Quat]) {
    for side in ["Left", "Right"] {
        for (seg, far, upper) in SEGMENTS {
            let (Some(s), Some(n)) = (skel.joint(&format!("{side}{seg}")), skel.joint(&format!("{side}{far}"))) else { continue };
            let axis = (world[s] * (skel.joints[n].head - skel.joints[s].head)).normalize_or_zero();
            if axis == Vec3::ZERO {
                continue;
            }
            let before = skel.joints[s].parent.map_or(Quat::IDENTITY, |p| world[p]);
            let (start, end) = if upper { (angle_about(world[s] * before.inverse(), axis), 0.0) } else { (0.0, angle_about(world[n] * world[s].inverse(), axis)) };
            for (k, turn) in twist_joints(axis, start, end, upper).into_iter().enumerate() {
                if let Some(t) = skel.joint(&format!("{side}{seg}Twist{}", k + 1)) {
                    world[t] = (turn * world[s]).normalize();
                }
            }
        }
    }
}

/// The pose of a SOMA body from a motion's turns: `names` and `turns` are the motion's joints and each one's world
/// turn from the T-pose; `tpose` is each of the body's joints' turn from bind to the T-pose, by name (a joint
/// without one is taken as bound in the T-pose); `root` is where the hips are. Ends, eyes and the jaw are not
/// turned by a motion's body joints and ride their parents.
pub fn from_turns(skel: &Skeleton, tpose: &HashMap<String, Quat>, names: &[String], turns: &[Quat], root: Vec3) -> Pose {
    let given: HashMap<&str, Quat> = names.iter().map(String::as_str).zip(turns.iter().copied()).collect();
    let mut world = vec![Quat::IDENTITY; skel.joints.len()];
    for (i, joint) in skel.joints.iter().enumerate() {
        let rides = joint.name.ends_with("End") || joint.name.ends_with("Eye") || joint.name == "Jaw";
        world[i] = match given.get(joint.name.as_str()).filter(|_| !rides) {
            Some(turn) => (*turn * tpose.get(&joint.name).copied().unwrap_or(Quat::IDENTITY)).normalize(),
            None => joint.parent.map_or(Quat::IDENTITY, |p| world[p]),
        };
    }
    set_twists(skel, &mut world);
    Pose { root, turns: locals(skel, &world) }
}

/// each joint's turn against its parent, from every joint's world turn
fn locals(skel: &Skeleton, world: &[Quat]) -> Vec<Quat> {
    skel.joints.iter().enumerate().map(|(i, j)| j.parent.map_or(world[i], |p| (world[p].inverse() * world[i]).normalize())).collect()
}

/// A pose of a SOMA body with its twist joints set from the joints at each limb segment's ends: for a pose that
/// came from a clip or a solver, which turn the named joints only.
pub fn with_twists(skel: &Skeleton, pose: &Pose) -> Pose {
    let mut world = skel.fk(pose).rot;
    set_twists(skel, &mut world);
    Pose { root: pose.root, turns: locals(skel, &world) }
}
