//! Joint limits, as sdk/body has them (axes.ts, limits.ts): each joint's turn read as anatomical angles from the
//! rig's rest pose (flex: its main bend; side: the bend across it; twist: about its bone), and the range of each,
//! degrees. The ranges are sdk/body's, set from anatomy and checked there against 26,288 captured frames. Written
//! for the left; the right is mirrored, so the same numbers mean the same motion on either side.

use glam::{Quat, Vec3};

use crate::arm::twist_about;
use crate::rig::{Pose, Skeleton};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angles {
    pub flex: f32,
    pub side: f32,
    pub twist: f32,
}

/// a joint's role (its name without the side), the ranges of its flex, side and twist, where its bone's tip goes as
/// it flexes (None: an elbow, which folds in the plane its rest bend makes) and as it bends sideways
type Row = (
    &'static str,
    [f32; 2],
    [f32; 2],
    [f32; 2],
    Option<Vec3>,
    Vec3,
);
const UP: Vec3 = Vec3::Y;
const AHEAD: Vec3 = Vec3::Z;
const OUT: Vec3 = Vec3::X;
const LIMITS: [Row; 13] = [
    (
        "spine_01",
        [-25.0, 45.0],
        [-20.0, 20.0],
        [-15.0, 15.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "spine_02",
        [-5.0, 100.0],
        [-30.0, 30.0],
        [-20.0, 20.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "spine_03",
        [-65.0, 25.0],
        [-30.0, 30.0],
        [-20.0, 20.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "neck_01",
        [-60.0, 55.0],
        [-40.0, 40.0],
        [-45.0, 45.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "head",
        [-45.0, 45.0],
        [-30.0, 30.0],
        [-45.0, 45.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "clavicle",
        [-45.0, 25.0],
        [-20.0, 75.0],
        [-20.0, 15.0],
        Some(AHEAD),
        UP,
    ),
    (
        "upperarm",
        [-65.0, 150.0],
        [-70.0, 140.0],
        [-95.0, 80.0],
        Some(AHEAD),
        UP,
    ),
    (
        "lowerarm",
        [-50.0, 110.0],
        [-20.0, 8.0],
        [-100.0, 40.0],
        None,
        UP,
    ),
    (
        "hand",
        [-75.0, 60.0],
        [-35.0, 45.0],
        [-5.0, 25.0],
        Some(Vec3::NEG_Y),
        OUT,
    ),
    (
        "thigh",
        [-35.0, 125.0],
        [-30.0, 65.0],
        [-60.0, 30.0],
        Some(AHEAD),
        OUT,
    ),
    (
        "calf",
        [-8.0, 150.0],
        [-20.0, 8.0],
        [-15.0, 15.0],
        Some(Vec3::NEG_Z),
        OUT,
    ),
    (
        "foot",
        [-30.0, 55.0],
        [-40.0, 25.0],
        [-25.0, 35.0],
        Some(UP),
        OUT,
    ),
    (
        "ball",
        [-35.0, 65.0],
        [-25.0, 25.0],
        [-30.0, 40.0],
        Some(UP),
        OUT,
    ),
];

/// The same for a SOMA body, read from SOMA's rest pose (its bind: arms about 50 degrees under level, elbows nearly
/// straight): re-measured over the capture library moved to SOMA (8,848 frames; the body SDK's table). SOMA's rest
/// elbow says little of how it folds, so the way the hand goes as it folds is the captures' own.
#[rustfmt::skip]
const SOMA_LIMITS: [Row; 13] = [
    ("spine_01", [-25.0, 55.0], [-20.0, 20.0], [-15.0, 15.0], Some(AHEAD), OUT),
    ("spine_02", [-30.0, 100.0], [-30.0, 30.0], [-20.0, 20.0], Some(AHEAD), OUT),
    ("spine_03", [-65.0, 50.0], [-30.0, 30.0], [-20.0, 20.0], Some(AHEAD), OUT),
    ("neck_01", [-60.0, 55.0], [-40.0, 40.0], [-45.0, 45.0], Some(AHEAD), OUT),
    ("head", [-45.0, 45.0], [-30.0, 30.0], [-45.0, 45.0], Some(AHEAD), OUT),
    ("clavicle", [-45.0, 35.0], [-20.0, 75.0], [-40.0, 15.0], Some(AHEAD), UP),
    ("upperarm", [-80.0, 150.0], [-70.0, 140.0], [-105.0, 80.0], Some(AHEAD), UP),
    ("lowerarm", [-30.0, 115.0], [-25.0, 12.0], [-100.0, 70.0], Some(Vec3::new(-0.316, 0.287, 0.902)), UP),
    ("hand", [-75.0, 60.0], [-50.0, 45.0], [-15.0, 25.0], Some(Vec3::NEG_Y), OUT),
    ("thigh", [-50.0, 125.0], [-30.0, 65.0], [-60.0, 30.0], Some(AHEAD), OUT),
    ("calf", [-17.0, 150.0], [-20.0, 8.0], [-15.0, 15.0], Some(Vec3::NEG_Z), OUT),
    ("foot", [-42.0, 55.0], [-52.0, 25.0], [-25.0, 35.0], Some(UP), OUT),
    ("ball", [-35.0, 88.0], [-55.0, 25.0], [-42.0, 40.0], Some(UP), OUT),
];

/// a joint's axes at rest: its bone, the axes a turn about which is its flexion and its side bend, and the hand its
/// twist is read by (+1 on the left and the midline)
struct Axes {
    bone: Vec3,
    flex: Vec3,
    side: Vec3,
    hand: f32,
    row: &'static Row,
}

fn axes(skel: &Skeleton, i: usize) -> Option<Axes> {
    let j = &skel.joints[i];
    let named = crate::rig::role_name(&j.name);
    let right = named.ends_with("_r");
    let role = named.trim_end_matches("_l").trim_end_matches("_r");
    // (a SOMA joint's name is not its role)
    let row = if named == j.name { &LIMITS } else { &SOMA_LIMITS }.iter().find(|r| r.0 == role)?;
    let mirror = |v: Vec3| if right { Vec3::new(-v.x, v.y, v.z) } else { v };
    let dir = |k: usize| (skel.joints[k].tail - skel.joints[k].head).normalize();
    let bone = dir(i);
    let off = |v: Vec3, n: Vec3| v - n * v.dot(n);
    // the fold's tip direction: away from the parent's bone, in the plane the rest bend makes
    let toward = match row.4 {
        Some(d) => mirror(d),
        None => off(-dir(j.parent?), bone),
    };
    let flex = bone.cross(toward).try_normalize()?;
    let side = off(off(bone.cross(mirror(row.5)), flex), bone).try_normalize()?;
    Some(Axes {
        bone,
        flex,
        side,
        hand: if right { -1.0 } else { 1.0 },
        row,
    })
}

/// a joint's turn as anatomical angles, degrees (None for a joint with no limits: the root, the fingers)
pub fn angles_of(skel: &Skeleton, i: usize, q: Quat) -> Option<Angles> {
    let ax = axes(skel, i)?;
    Some(read(&ax, q))
}

fn read(ax: &Axes, q: Quat) -> Angles {
    let twist = twist_about(q, ax.bone);
    // the short way round: a turn and its negative are one turn, and only one of them reads under half a turn
    let swing = q * twist.inverse();
    let swing = if swing.w < 0.0 { -swing } else { swing }.to_scaled_axis();
    let about = 2.0
        * Vec3::new(twist.x, twist.y, twist.z)
            .dot(ax.bone)
            .atan2(twist.w);
    Angles {
        flex: swing.dot(ax.flex).to_degrees(),
        side: swing.dot(ax.side).to_degrees(),
        twist: (about * ax.hand).to_degrees(),
    }
}

fn clamped(ax: &Axes, a: Angles) -> Angles {
    let (mut flex, mut side) = (
        a.flex.clamp(ax.row.1[0], ax.row.1[1]),
        a.side.clamp(ax.row.2[0], ax.row.2[1]),
    );
    // the swing as a whole to 175 degrees, past which a swing's angles stop meaning one turn
    let whole = flex.hypot(side);
    if whole > 175.0 {
        (flex, side) = (flex * 175.0 / whole, side * 175.0 / whole);
    }
    Angles {
        flex,
        side,
        twist: a.twist.clamp(ax.row.3[0], ax.row.3[1]),
    }
}

/// A joint's turn brought inside its limit (each angle clamped), and how far that turned it, degrees.
pub fn clamp_turn(skel: &Skeleton, i: usize, q: Quat) -> (Quat, f32) {
    let Some(ax) = axes(skel, i) else {
        return (q, 0.0);
    };
    let (a, c) = (read(&ax, q), clamped(&ax, read(&ax, q)));
    if a == c {
        return (q, 0.0);
    }
    let swing =
        Quat::from_scaled_axis(ax.flex * c.flex.to_radians() + ax.side * c.side.to_radians());
    let turn =
        (swing * Quat::from_axis_angle(ax.bone, (c.twist * ax.hand).to_radians())).normalize();
    (turn, q.angle_between(turn).to_degrees())
}

/// Every joint of a pose outside its limit by more than `tolerance` degrees: its index and by how much, worst first.
pub fn violations(skel: &Skeleton, p: &Pose, tolerance: f32) -> Vec<(usize, f32)> {
    let mut out: Vec<(usize, f32)> = (0..skel.joints.len())
        .filter_map(|i| {
            let ax = axes(skel, i)?;
            let (a, c) = (read(&ax, p.turns[i]), clamped(&ax, read(&ax, p.turns[i])));
            let by = (a.flex - c.flex)
                .hypot(a.side - c.side)
                .max((a.twist - c.twist).abs());
            (by > tolerance).then_some((i, by))
        })
        .collect();
    out.sort_by_key(|v| std::cmp::Reverse((v.1 * 1000.0) as i64));
    out
}
