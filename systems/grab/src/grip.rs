//! The grip: how a hand lies on its hold, decided once at the grab and kept; how a thing in one hand turns with the
//! forearm; and how far an arm is past its joints' ranges.

use glam::{Quat, Vec3};
use raea_sys_body::arm::{ArmOptions, reach};
use raea_sys_body::limits::clamp_turn;
use raea_sys_body::rig::{Pose, Skeleton, hand_rest};
use raea_sys_body::shape::Thing;

use crate::hand::{facing, wrist_for};
use crate::take::{Part, Release, Take, smooth, span, yaw};
use crate::trunk::{clear, give_for, hand_points, room, spread_for, stoop};

/// how far past their ranges an arm's shoulder and elbow are in a pose, degrees
pub(crate) fn past_range(skel: &Skeleton, arm: [usize; 3], pose: &Pose) -> f32 {
    arm[..2].iter().map(|&joint| clamp_turn(skel, joint, pose.turns[joint]).1).sum()
}

/// How a hand lies on its hold at its grab, in the thing's own frame (the wrist's place and the hand's turn): the
/// fingers along the forearm as it arrives (found by solving the arm to the hold alone, from the body as it is
/// then), so the wrist is asked for the least bend; where that would tip the fingers into a solid (a cup on a table,
/// the forearm coming down to it) they are brought level, by as much as they would be pushed. A hold that is a way
/// of holding says where the fingers point.
pub(crate) fn lie_at_grab(skel: &Skeleton, base: &dyn Fn(f32) -> Pose, take: &Take, part: &Part) -> Option<(Vec3, Quat)> {
    let hold = part.hold;
    let thing = if part.grab <= take.grab + 1e-4 { take.thing } else { take.thing_at(skel, base, part.grab).0 };
    let own = base(part.grab);
    let w = skel.fk(&own);
    let (face, arm, rest) = (yaw(&w) * Vec3::Z, skel.arm(hold.side)?, hand_rest(skel, hold.side)?);
    let from = thing.world(hold.at) - w.pos[arm[0]];
    let found = Vec3::new(from.x, 0.0, from.z).normalize_or(face).lerp(facing(&thing, w.pos[0]), 0.4).normalize_or(face);
    let level = wrist_for(skel, &thing, &hold, hold.along.map_or(found, |a| thing.turn * a))?;
    let (at, turn) = if hold.along.is_some() {
        level
    } else {
        // the body as it will be at the grab: bent for a low thing, with the give its head and chest need there
        let bent = |give: f32| stoop(skel, &own, &w, face, level.0.y, 1.0, give, spread_for(skel, &w, &thing), &take.avoid);
        let hi = give_for(room(skel, &bent(0.0), &take.avoid));
        let pose = bent(hi);
        let probe = reach(skel, &pose, hold.side, thing.world(hold.at) + thing.turn * hold.normal * 0.04, ArmOptions { stiff: hi, ..Default::default() });
        let coming = |r: Option<raea_sys_body::arm::IkReport>| {
            r.map(|r| {
                let drawn = skel.fk(&r.pose);
                (drawn.pos[arm[2]] - drawn.pos[arm[1]]).normalize_or(face)
            })
        };
        let mut forearm = coming(probe).unwrap_or(face);
        // a thing that is then carried is held by a grip that also suits the carry: the fingers point between the
        // way the forearm comes at the grab and the way it will come once the thing is at its carry place (each
        // taken in the thing's own frame), so neither asks the wrist for the whole turn between them
        if take.slide.is_empty() && part.grab <= take.grab + 1e-4 {
            let then = take.thing_at(skel, base, take.carried).0;
            let later = coming(reach(skel, &base(take.carried), hold.side, then.world(hold.at) + then.turn * hold.normal * 0.04, ArmOptions::default()));
            if let Some(later) = later {
                forearm = (forearm + thing.turn * (then.turn.inverse() * later)).normalize_or(forearm);
            }
        }
        let lie = |keep: f32| wrist_for(skel, &thing, &hold, Vec3::new(forearm.x, forearm.y * keep, forearm.z).normalize_or(face));
        match lie(1.0) {
            Some(along) => lie(1.0 - smooth((clear(along.0, along.1, &hand_points(skel, hold.side, &rest, None), &take.avoid) - along.0).length() / 0.03)).unwrap_or(level),
            None => level,
        }
    };
    Some((thing.local(at), (thing.turn.inverse() * turn).normalize()))
}

/// How far a thing carried in one hand is turned about the vertical at `t`, radians: so the hand that holds it lies
/// along the forearm as the arm comes to it, up to a quarter turn. A grip doesn't change once taken, so without this a cup raised to the mouth would be
/// held by a wrist bent past what a wrist does; with it the cup turns in the hand's favour, as a person's does.
/// None while it stands, all once carried, none again by the time it is set down.
pub(crate) fn swivel(skel: &Skeleton, base: &dyn Fn(f32) -> Pose, take: &Take, part: &Part, t: f32, thing: &Thing) -> f32 {
    let down = match take.release {
        Some(Release::Put { at, down, .. }) => 1.0 - smooth(span(t, at, down)),
        _ => 1.0,
    };
    let weight = smooth(span(t, take.grab, take.carried)) * down;
    let (Some(arm), Some(rest), Some((_, lie))) = (skel.arm(part.hold.side), hand_rest(skel, part.hold.side), lie_at_grab(skel, base, take, part)) else { return 0.0 };
    if weight <= 0.0 {
        return 0.0;
    }
    let own = base(t);
    let level = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    // the arm solved to the thing's middle (not the hold, which the turn itself moves): the way the forearm comes
    let Some(probe) = reach(skel, &own, part.hold.side, thing.at, ArmOptions::default()) else { return 0.0 };
    let drawn = skel.fk(&probe.pose);
    let (fingers, forearm) = (level(thing.turn * lie * rest.along), level((drawn.pos[arm[2]] - drawn.pos[arm[1]]).normalize_or_zero()));
    // a hand or a forearm that points nearly straight up or down says little about which way round: less turn
    let sure = smooth(fingers.length().min(forearm.length()) / 0.3);
    // by the sine of the angle between them: up to a quarter turn, none where they are in line or dead against each
    // other, so it never flips
    let turned = (fingers.cross(forearm).y / (fingers.length() * forearm.length()).max(1e-6)).clamp(-1.0, 1.0).asin() * sure;
    turned * weight
}
