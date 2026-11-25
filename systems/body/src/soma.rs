//! Motion on SOMA's skeleton played on raea's rig. SOMA (NVlabs/SOMA-X, Apache-2.0) is the 78-joint skeleton
//! NVIDIA's motion models and datasets speak (Kimodo, GEM-X, BONES-SEED...); raea's rig is MakeHuman's game_engine,
//! 52 joints. The bridge is by direction, not by turn: each of raea's bones is pointed the way SOMA's matching bone
//! points, at raea's own lengths, so the two bodies' proportions and bind poses don't have to agree; the pose is
//! then fitted to those places (fit.rs). Positions alone leave a limb's turn about itself open: the first bone of
//! each arm and leg is turned so its elbow or knee flexes about its own axis (arm.rs).

use std::collections::HashMap;

use glam::{Mat3, Quat, Vec3};

use crate::arm::settle_hinge;
use crate::fit::{Mark, fit};
use crate::rig::{Pose, Side, Skeleton};

/// where the middle of SOMA's eyes is from its head joint in its bind pose (somaskel's T-pose), centimetres
const SOMA_EYES: Vec3 = Vec3::new(0.0, 5.371, 7.573);

/// raea's joint for a SOMA joint: the trunk and head by name, limbs and fingers by side. A SOMA finger's 2, 3, 4 are
/// raea's 01, 02, 03 (its 1 is the metacarpal, which raea's hand has none of); the thumb's are 1, 2, 3.
pub fn raea_joint(soma: &str) -> Option<String> {
    crate::rig::rename(soma, false)
}

/// the SOMA joint that ends a raea joint's bone, where raea has no joint of its own there
fn bone_end(raea: &str) -> Option<String> {
    let side = if raea.ends_with("_l") { "Left" } else { "Right" };
    match raea.trim_end_matches("_l").trim_end_matches("_r") {
        "head" => Some("HeadEnd".into()),
        "ball" => Some(format!("{side}ToeEnd")),
        // (the 30-joint SOMA has no fingers: the hand points at its middle finger's end)
        "hand" => Some(format!("{side}HandMiddleEnd")),
        tip => tip.strip_suffix("_03").map(|finger| format!("{side}Hand{}{}End", finger[..1].to_uppercase(), &finger[1..])),
    }
}

/// The pose that has raea's bones pointing as a SOMA body's do. `names` and `at` are SOMA's joints and where each is
/// (any unit, y up); `eyes` is where the point between raea's eyes is at rest, when the body says (it gives the
/// head its turn about the neck). The body stands where SOMA's hips are, scaled by the two bodies' leg lengths.
pub fn from_soma(skel: &Skeleton, names: &[String], at: &[Vec3], eyes: Option<Vec3>) -> Option<Pose> {
    let soma: HashMap<&str, Vec3> = names.iter().map(String::as_str).zip(at.iter().copied()).collect();
    let seen: HashMap<usize, Vec3> = names.iter().zip(at).filter_map(|(n, p)| Some((skel.joint(&raea_joint(n)?)?, *p))).collect();
    let pelvis = skel.joint("pelvis")?;
    // the scale between the bodies: raea's leg (hip to knee to ankle) over SOMA's
    let leg = |a: Vec3, b: Vec3, c: Vec3| (b - a).length() + (c - b).length();
    let [thigh, calf, foot] = ["thigh_l", "calf_l", "foot_l"].map(|n| skel.joint(n));
    let (thigh, calf, foot) = (thigh?, calf?, foot?);
    let ours = leg(skel.joints[thigh].head, skel.joints[calf].head, skel.joints[foot].head);
    let scale = ours / leg(*seen.get(&thigh)?, *seen.get(&calf)?, *seen.get(&foot)?).max(1e-6);
    // each of raea's joints placed from its parent's, the way SOMA's matching joints lie, at raea's own length
    let mut place: Vec<Option<Vec3>> = vec![None; skel.joints.len()];
    place[pelvis] = Some(*seen.get(&pelvis)? * scale);
    let mut marks = vec![Mark { joint: pelvis, offset: Vec3::ZERO, at: place[pelvis]?, weight: 1.0 }];
    for (i, joint) in skel.joints.iter().enumerate() {
        let Some(parent) = joint.parent else { continue };
        let (Some(from), Some(a), Some(b)) = (place[parent], seen.get(&parent), seen.get(&i)) else { continue };
        let to = from + (*b - *a).normalize_or_zero() * skel.offset(i).length();
        place[i] = Some(to);
        marks.push(Mark { joint: i, offset: Vec3::ZERO, at: to, weight: 1.0 });
        // and a bone's end, where SOMA has a joint for it and raea has none
        if let Some(end) = bone_end(&joint.name).and_then(|n| soma.get(n.as_str())) {
            let bone = joint.tail - joint.head;
            marks.push(Mark { joint: i, offset: bone, at: to + (*end - *b).normalize_or_zero() * bone.length(), weight: 1.0 });
        }
    }
    // the head's turn, from SOMA's head and eyes, which are rigid with each other: the turn that takes them from
    // where SOMA's bind pose has them (eyes level across, their middle 35 degrees above straight ahead of the head
    // joint) to where they are. raea's head, its eyes and its top are carried by that same turn, so the two rigs'
    // head joints sitting at different places in the skull doesn't tip the head
    if let (Some(head), Some(l), Some(r)) = (skel.joint("head"), soma.get("LeftEye"), soma.get("RightEye")) {
        if let (Some(to), Some(at)) = (place[head], seen.get(&head)) {
            let frame = |across: Vec3, ahead: Vec3| {
                let x = across.normalize_or(Vec3::X);
                let z = ahead.reject_from_normalized(x).normalize_or(Vec3::Z);
                Mat3::from_cols(x, z, x.cross(z))
            };
            let turn = Quat::from_mat3(&(frame(*l - *r, (*l + *r) / 2.0 - *at) * frame(Vec3::X, SOMA_EYES).transpose())).normalize();
            let top = skel.joints[head].tail - skel.joints[head].head;
            marks.retain(|m| m.joint != head || m.offset == Vec3::ZERO);
            marks.push(Mark { joint: head, offset: top, at: to + turn * top, weight: 1.0 });
            let ahead = eyes.map_or(Vec3::new(0.0, 0.04, 0.09), |e| e - skel.joints[head].head);
            marks.push(Mark { joint: head, offset: ahead, at: to + turn * ahead, weight: 1.0 });
        }
    }
    let mut pose = fit(skel, &skel.rest(), &marks, 6).0;
    for side in Side::BOTH {
        let x = side.sx();
        for limb in [["upperarm", "lowerarm", "hand"], ["thigh", "calf", "foot"]] {
            if let [Some(a), Some(b), Some(c)] = limb.map(|n| skel.joint(&format!("{n}_{x}"))) {
                pose = settle_hinge(skel, &pose, [a, b, c]);
            }
        }
    }
    Some(pose)
}
