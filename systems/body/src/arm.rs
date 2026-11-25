//! Arm and leg IK. `two_bone` puts a limb's end joint (wrist, ankle) on a target: the middle joint bends about the hinge it
//! already has, the limb swings the least that gets there, and `pole` chooses the swivel (where the elbow points).
//! `reach` adds the trunk: when the arm alone falls short, the clavicle, chest and spine lean the shoulder toward the
//! target, each by a capped share (sdk/body/src/reach.ts). Both report how far they missed and how far they turned
//! each joint, so a caller can tell a held contact from a pose that should never have been asked for
//! (docs/character.md, principle 2).

use glam::{Quat, Vec3};

use crate::rig::{Pose, Side, Skeleton};

#[derive(Clone, Copy, Debug, Default)]
pub struct ArmOptions {
    /// a direction (the body's frame) the middle joint should point toward, off the line to the target
    pub pole: Option<Vec3>,
    /// the end joint's world turn; by default the one it has
    pub end: Option<Quat>,
    /// metres short of straight where the limb starts to ease toward its full length instead of snapping straight
    /// (0: exact, and an elbow that locks the frame the target passes the arm's length)
    pub soft: f32,
    /// how much of the trunk's lean a reach gives up (0: it leans as far as it may; 1: the arm alone), for a body
    /// with something in front of it
    pub stiff: f32,
}

#[derive(Clone, Debug)]
pub struct IkReport {
    pub pose: Pose,
    /// how far the end joint ended from the target, metres
    pub miss: f32,
    /// how far each joint the solver touched was turned from the pose it was given, degrees
    pub moved: Vec<(usize, f32)>,
    /// the target is farther than the limb reaches straight, or nearer than it folds
    pub out_of_reach: bool,
}

/// the distance a limb of lengths adding to `full` is asked to span, eased so it never quite locks
fn soften(d: f32, full: f32, soft: f32) -> f32 {
    if soft <= 0.0 || d <= full - soft {
        return d;
    }
    let knee = full - soft;
    knee + soft * (1.0 - (-(d - knee) / soft).exp())
}

pub fn two_bone(skel: &Skeleton, p: &Pose, limb: [usize; 3], target: Vec3, o: ArmOptions) -> IkReport {
    let [a, b, c] = limb;
    let w = skel.fk(p);
    let (pa, pb, pc) = (w.pos[a], w.pos[b], w.pos[c]);
    let (la, lb) = ((pb - pa).length(), (pc - pb).length());
    let full = la + lb;
    let asked = (target - pa).length();
    let fold = (la - lb).abs() + 1e-4;
    let out_of_reach = asked > full || asked < fold;
    let d = soften(asked, full, o.soft).clamp(fold, full - 1e-5);

    // the hinge the middle joint already bends about; a straight limb takes it from the pole, or any side
    let (u, v) = ((pb - pa) / la, (pc - pb) / lb);
    let mut hinge = u.cross(v);
    if hinge.length_squared() < 1e-8 {
        hinge = o.pole.map_or(Vec3::ZERO, |pole| pole.cross(u));
    }
    let hinge = if hinge.length_squared() < 1e-8 { u.any_orthonormal_vector() } else { hinge.normalize() };
    // the bend between the two bones now, and the one that spans d (the law of cosines)
    let bend0 = u.dot(v).clamp(-1.0, 1.0).acos();
    let bend1 = std::f32::consts::PI - ((la * la + lb * lb - d * d) / (2.0 * la * lb)).clamp(-1.0, 1.0).acos();
    let flex = Quat::from_axis_angle(hinge, bend1 - bend0);
    let pc1 = pb + flex * (pc - pb);

    // the whole limb swung so its end lies toward the target
    let to = if asked > 1e-6 { (target - pa) / asked } else { (pc1 - pa).normalize() };
    let swing = Quat::from_rotation_arc((pc1 - pa).normalize(), to);
    let mut turn_a = swing;
    if let Some(pole) = o.pole {
        // about the line to the target, the middle joint brought toward the pole
        let elbow = swing * (pb - pa);
        let (e, q) = (elbow - to * elbow.dot(to), pole - to * pole.dot(to));
        if e.length_squared() > 1e-8 && q.length_squared() > 1e-8 {
            let (e, q) = (e.normalize(), q.normalize());
            let angle = e.cross(q).dot(to).atan2(e.dot(q));
            turn_a = Quat::from_axis_angle(to, angle) * swing;
        }
    }
    let ra = (turn_a * w.rot[a]).normalize();
    let rb = (turn_a * flex * w.rot[b]).normalize();
    // the middle joint is a hinge with an axis of its own, the one the rest pose's bend is about (an elbow's, a
    // knee's): the first bone is turned about itself until that axis is the one the limb is bent about (hinge_turn)
    let ra = (hinge_turn(skel, limb, ra, (turn_a * u).normalize(), (turn_a * u).cross(turn_a * flex * v)) * ra).normalize();
    let rc = o.end.unwrap_or(w.rot[c]);

    let mut pose = p.clone();
    pose.turns[a] = skel.local_for(&w, a, ra);
    pose.turns[b] = (ra.inverse() * rb).normalize();
    pose.turns[c] = (rb.inverse() * rc).normalize();
    let end = skel.fk(&pose).pos[c];
    let moved = limb.iter().map(|&i| (i, p.turns[i].angle_between(pose.turns[i]).to_degrees())).collect();
    IkReport { pose, miss: (end - target).length(), moved, out_of_reach }
}

/// the trunk joints that lean a shoulder, nearest the arm first: the share of the lean each takes a pass, and the
/// most it may turn from the pose it was given, degrees
const LEAN: [(&str, f32, f32); 3] = [("clavicle", 0.35, 18.0), ("spine_03", 0.5, 30.0), ("spine_02", 0.5, 25.0)];

/// A hand's wrist put on a target the arm alone may not get to.
pub fn reach(skel: &Skeleton, p: &Pose, s: Side, target: Vec3, o: ArmOptions) -> Option<IkReport> {
    let arm = skel.arm(s)?;
    let within = 0.002;
    let mut out = two_bone(skel, p, arm, target, o);
    let lean: Vec<(usize, f32, f32)> = LEAN
        .iter()
        .filter_map(|(n, share, cap)| {
            let name = if *n == "clavicle" { format!("clavicle_{}", s.sx()) } else { n.to_string() };
            Some((skel.joint(&name)?, *share, *cap * (1.0 - o.stiff.clamp(0.0, 1.0))))
        })
        .collect();
    // (whenever the arm ends short, not only past its full length: the arm eases toward straight before that, and a
    // lean that waited for the full length came on all at once)
    if out.miss <= within {
        return Some(out);
    }
    let hand = o.end.unwrap_or(skel.fk(p).rot[arm[2]]);
    let o = ArmOptions { end: Some(hand), ..o };
    let arm_len = skel.offset(arm[1]).length() + skel.offset(arm[2]).length();
    let mut turns = p.turns.clone();
    for _ in 0..8 {
        if out.miss <= within {
            break;
        }
        for &(i, share, cap) in &lean {
            let w = skel.fk(&Pose { root: p.root, turns: turns.clone() });
            let sh = w.pos[arm[0]];
            // where the shoulder must be for the arm, nearly straight, to touch the target
            let want = target - (target - sh).normalize_or_zero() * arm_len * 0.97;
            if (want - sh).length() < 1e-4 {
                break;
            }
            let turn = Quat::IDENTITY.slerp(Quat::from_rotation_arc((sh - w.pos[i]).normalize(), (want - w.pos[i]).normalize()), share);
            let local = skel.local_for(&w, i, turn * w.rot[i]);
            // held to its cap, measured from the pose it came with
            let from = p.turns[i];
            let off = from.angle_between(local).to_degrees();
            turns[i] = if off > cap { from.slerp(local, cap / off) } else { local };
        }
        out = two_bone(skel, &Pose { root: p.root, turns: turns.clone() }, arm, target, o);
        turns = out.pose.turns.clone();
    }
    out.moved = lean.iter().map(|l| l.0).chain(arm).map(|i| (i, p.turns[i].angle_between(out.pose.turns[i]).to_degrees())).collect();
    Some(out)
}

/// A wrist's twist shared with the forearm, where pronation happens, so the skin doesn't wring at the wrist. The hand
/// ends where it was. `share` is how much of the twist the forearm takes (1: all of it, sdk/body's library fix).
/// The split is SOMA-X's swing-twist extractor (NVlabs/SOMA-X, Apache-2.0, `local_x_swing_twist`, half-angle form).
pub fn forearm_twist(skel: &Skeleton, p: &Pose, s: Side, share: f32) -> Pose {
    let Some([_, fore, hand]) = skel.arm(s) else { return p.clone() };
    let axis = skel.offset(hand).normalize_or_zero();
    let twist = twist_about(p.turns[hand], axis);
    let part = Quat::IDENTITY.slerp(twist, share);
    let mut out = p.clone();
    out.turns[fore] = (p.turns[fore] * part).normalize();
    out.turns[hand] = (part.inverse() * p.turns[hand]).normalize();
    out
}

/// the part of a turn that is about `axis` (the twist of its swing-twist split), the short way round
pub fn twist_about(q: Quat, axis: Vec3) -> Quat {
    let along = Vec3::new(q.x, q.y, q.z).dot(axis);
    let t = Quat::from_xyzw(axis.x * along, axis.y * along, axis.z * along, q.w);
    if t.length_squared() < 1e-12 {
        return Quat::IDENTITY;
    }
    let t = t.normalize();
    if t.w < 0.0 { -t } else { t }
}

/// How SOMA-X spreads a segment's twist over four twist joints along it, nearest the start joint first
/// (SOMA_procedural_transforms.json): for a rig that has twist joints. raea's game_engine rig has none.
pub const TWIST_SHARES: [f32; 4] = [0.05, 1.0 / 3.0, 2.0 / 3.0, 0.95];

/// A segment's four twist joints' turns about its bone `axis`, as SOMA-X's procedural rig evaluates them: `start` is
/// the twist (radians) of the joint the segment hangs from, `end` the twist of the joint at its far end. A forearm
/// or shin follows its end joint by rising shares. An upper arm or thigh (`upper`) undoes its own joint's twist
/// near the shoulder or hip, so the skin there doesn't wring, and follows the next joint toward the elbow or knee.
pub fn twist_joints(axis: Vec3, start: f32, end: f32, upper: bool) -> [Quat; 4] {
    TWIST_SHARES.map(|share| Quat::from_axis_angle(axis, if upper { -(1.0 - share) * start + share * end } else { share * end }))
}

/// The turn of a limb's first bone about itself (`along`, as posed) that makes the hinge its rest pose bends about
/// the one the limb is bent about now (`bent`: the first bone crossed with the second, as posed), as an upper arm
/// turns in its socket: so the middle joint flexes and never bends sideways. `ra` is the first bone's world turn. A
/// limb at rest nearly straight has no such axis to read, and a limb posed nearly straight no bend to match: less
/// turn the nearer either is.
fn hinge_turn(skel: &Skeleton, limb: [usize; 3], ra: Quat, along: Vec3, bent: Vec3) -> Quat {
    let own = skel.offset(limb[1]).normalize_or_zero().cross(skel.offset(limb[2]).normalize_or_zero());
    let sure = (own.length() / 0.2).min(1.0) * (bent.length() / 0.2).min(1.0);
    let (has, wants) = ((ra * own).reject_from_normalized(along), bent.reject_from_normalized(along));
    if sure <= 0.0 || has.length() < 1e-4 || wants.length() < 1e-4 {
        return Quat::IDENTITY;
    }
    Quat::from_axis_angle(along, has.cross(wants).dot(along).atan2(has.dot(wants)) * sure * sure * (3.0 - 2.0 * sure))
}

/// A pose with a limb's first bone turned about itself so its middle joint flexes about its own axis, every joint
/// staying where it is: for a pose that came from places alone (a fit), which leaves that turn open.
pub fn settle_hinge(skel: &Skeleton, p: &Pose, limb: [usize; 3]) -> Pose {
    let [a, b, c] = limb;
    let w = skel.fk(p);
    let (u, v) = ((w.pos[b] - w.pos[a]).normalize_or_zero(), (w.pos[c] - w.pos[b]).normalize_or_zero());
    let ra = (hinge_turn(skel, limb, w.rot[a], u, u.cross(v)) * w.rot[a]).normalize();
    let mut pose = p.clone();
    pose.turns[a] = skel.local_for(&w, a, ra);
    pose.turns[b] = (ra.inverse() * w.rot[b]).normalize();
    pose
}
