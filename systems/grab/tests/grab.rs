//! The measures docs/hands.feature names, on the stand-in body.

use glam::{Quat, Vec3};
use raea_sys_body::rig::Posed;
use raea_sys_body::*;
use raea_sys_grab::*;

/// a seeded random in [0, 1) (mulberry32, as sdk/body's tests use)
fn rng(mut seed: u32) -> impl FnMut() -> f32 {
    move || {
        seed = seed.wrapping_add(0x6d2b79f5);
        let mut t = (seed ^ (seed >> 15)).wrapping_mul(1 | seed);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f32) / 4294967296.0
    }
}

#[test]
fn mannequin_is_the_rig() {
    let s = mannequin();
    assert_eq!(s.joints.len(), 52);
    for (i, j) in s.joints.iter().enumerate() {
        assert!(j.parent.is_none_or(|p| p < i), "{} comes before its parent", j.name);
    }
    // the rest pose draws every joint where the skeleton has it
    let w = s.fk(&s.rest());
    for (i, j) in s.joints.iter().enumerate() {
        assert!((w.pos[i] - j.head).length() < 1e-5, "{}", j.name);
    }
}

#[test]
fn two_bone_hits_what_the_arm_reaches() {
    let s = mannequin();
    let mut r = rng(7);
    let mut worst = 0.0f32;
    for side in Side::BOTH {
        let arm = s.arm(side).unwrap();
        let shoulder = s.joints[arm[0]].head;
        let full = s.offset(arm[1]).length() + s.offset(arm[2]).length();
        for _ in 0..300 {
            // anywhere in front of the shoulder, from a third of the arm to nearly straight
            let dir = Vec3::new(r() * 2.0 - 1.0, r() * 2.0 - 1.0, r() * 0.9 + 0.1).normalize();
            let target = shoulder + dir * full * (0.33 + 0.65 * r());
            let out = two_bone(&s, &s.rest(), arm, target, ArmOptions::default());
            assert!(!out.out_of_reach);
            worst = worst.max(out.miss);
        }
    }
    assert!(worst < 1e-4, "worst miss {worst} m");
}

#[test]
fn two_bone_says_when_it_cannot_reach_and_changes_nothing_when_there() {
    let s = mannequin();
    let arm = s.arm(Side::L).unwrap();
    let full = s.offset(arm[1]).length() + s.offset(arm[2]).length();
    let far = s.joints[arm[0]].head + Vec3::Z * (full + 0.2);
    let out = two_bone(&s, &s.rest(), arm, far, ArmOptions::default());
    assert!(out.out_of_reach);
    assert!((out.miss - 0.2).abs() < 2e-3, "missed by the distance past: {}", out.miss);
    // the wrist already there: nothing turns
    let here = two_bone(&s, &s.rest(), arm, s.joints[arm[2]].head, ArmOptions::default());
    assert!(here.miss < 1e-5 && here.moved.iter().all(|m| m.1 < 0.05), "{:?}", here.moved);
}

#[test]
fn the_pole_sets_the_elbow_and_the_end_keeps_its_turn() {
    let s = mannequin();
    let arm = s.arm(Side::R).unwrap();
    let target = s.joints[arm[0]].head + Vec3::new(-0.05, -0.15, 0.35);
    let turn = Quat::from_rotation_y(0.6);
    for pole in [Vec3::NEG_Y, Vec3::NEG_X, Vec3::Y] {
        let out = two_bone(&s, &s.rest(), arm, target, ArmOptions { pole: Some(pole), end: Some(turn), ..Default::default() });
        let w = s.fk(&out.pose);
        assert!(out.miss < 1e-4);
        // the elbow is off the shoulder-wrist line on the pole's side
        let line = (w.pos[arm[2]] - w.pos[arm[0]]).normalize();
        let elbow = w.pos[arm[1]] - w.pos[arm[0]];
        let off = elbow - line * elbow.dot(line);
        let want = pole - line * pole.dot(line);
        assert!(off.normalize().dot(want.normalize()) > 0.999, "pole {pole}");
        assert!(w.rot[arm[2]].angle_between(turn) < 1e-3);
    }
}

#[test]
fn a_reach_past_the_arm_leans_the_trunk_and_says_so() {
    let s = mannequin();
    let arm = s.arm(Side::R).unwrap();
    let full = s.offset(arm[1]).length() + s.offset(arm[2]).length();
    let target = s.joints[arm[0]].head + Vec3::new(-0.1, -0.1, 1.0).normalize() * (full + 0.12);
    let out = reach(&s, &s.rest(), Side::R, target, ArmOptions::default()).unwrap();
    assert!(out.miss < 5e-3, "12 cm past the arm, missed by {}", out.miss);
    let chest = s.joint("spine_03").unwrap();
    assert!(out.moved.iter().any(|m| m.0 == chest && m.1 > 1.0), "the chest leant: {:?}", out.moved);
}

#[test]
fn forearm_twist_leaves_the_hand_where_it_was() {
    let s = mannequin();
    let [_, fore, hand] = s.arm(Side::L).unwrap();
    let mut p = s.rest();
    let axis = s.offset(hand).normalize();
    // a twist about the forearm's bone, on a bend that has none of its own
    p.turns[hand] = Quat::from_axis_angle(axis, 0.9) * Quat::from_axis_angle(axis.any_orthonormal_vector(), 0.2);
    let before = s.fk(&p);
    let out = forearm_twist(&s, &p, Side::L, 1.0);
    let after = s.fk(&out);
    assert!((before.pos[hand] - after.pos[hand]).length() < 1e-5);
    assert!(before.rot[hand].angle_between(after.rot[hand]) < 1e-3);
    // the wrist's own twist is gone, and the forearm has it
    assert!(arm::twist_about(out.turns[hand], axis).angle_between(Quat::IDENTITY) < 1e-3);
    assert!((out.turns[fore].angle_between(Quat::IDENTITY) - 0.9).abs() < 0.05);
}

#[test]
fn casts_meet_every_solid() {
    for solid in [
        Solid::Ball { r: 0.1 },
        Solid::Box { half: Vec3::new(0.2, 0.1, 0.15) },
        Solid::Capsule { half: 0.2, r: 0.05 },
        Solid::Cylinder { half: 0.06, r: 0.04 },
    ] {
        let thing = Thing { solid, at: Vec3::new(0.3, 1.0, 0.5), turn: Quat::from_rotation_z(0.4) };
        let mut r = rng(3);
        for _ in 0..50 {
            let dir = Vec3::new(r() - 0.5, r() - 0.5, r() - 0.5).normalize();
            let hit = thing.cast(thing.at + dir, -dir, 2.0).expect("a ray at the centre meets it");
            assert!(thing.distance(hit.at).abs() < 2e-4);
            // the surface faces back along the ray, more or less
            assert!(hit.normal.dot(dir) > 0.0);
        }
    }
}

#[test]
fn a_box_gets_two_hands_and_a_cup_one() {
    let holder = Vec3::new(0.0, 1.0, 0.0);
    let crate_ = Thing::new(Solid::Box { half: Vec3::new(0.18, 0.12, 0.14) }, Vec3::new(0.0, 1.0, 0.45));
    let Grip::Two(l, r) = find_holds(&crate_, holder, HoldOptions::default()) else { panic!("a box wider than a hand takes two") };
    // each on its own side's face, facing out
    assert!((l.at - Vec3::new(0.18, 0.0, 0.0)).length() < 1e-3 && l.normal.dot(Vec3::X) > 0.99);
    assert!((r.at - Vec3::new(-0.18, 0.0, 0.0)).length() < 1e-3 && r.normal.dot(Vec3::NEG_X) > 0.99);
    let cup = Thing::new(Solid::Cylinder { half: 0.05, r: 0.04 }, Vec3::new(-0.15, 1.0, 0.4));
    let Grip::One(h) = find_holds(&cup, holder, HoldOptions::default()) else { panic!("a cup takes one hand") };
    assert_eq!(h.side, Side::R);
    // turned, a thing's holds turn with it: found from wherever the holder stands
    let turned = Thing { turn: Quat::from_rotation_y(0.7), ..crate_ };
    for h in find_holds(&turned, holder, HoldOptions::default()).holds() {
        assert!(turned.distance(turned.world(h.at)).abs() < 2e-4);
    }
}

#[test]
fn the_palm_lies_on_its_hold() {
    let s = mannequin();
    let thing = Thing::new(Solid::Box { half: Vec3::new(0.18, 0.12, 0.14) }, Vec3::new(0.0, 1.1, 0.4));
    for hold in find_holds(&thing, Vec3::new(0.0, 1.0, 0.0), HoldOptions::default()).holds() {
        let (wrist, turn) = wrist_for(&s, &thing, &hold, Vec3::Z).unwrap();
        let rest = hand_rest(&s, hold.side).unwrap();
        // the palm's skin is on the surface, facing into it, fingers forward
        assert!((wrist + turn * rest.mid - thing.world(hold.at)).length() < 1e-4);
        assert!((turn * rest.palm).dot(thing.turn * hold.normal) < -0.999);
        assert!((turn * rest.along).dot(Vec3::Z) > 0.99);
    }
}

fn stand(s: &Skeleton) -> impl Fn(f32) -> Pose + '_ {
    |_| s.rest()
}

#[test]
fn a_box_is_taken_carried_and_thrown() {
    let s = mannequin();
    let thing = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(0.0, 1.05, 0.42));
    let grip = find_holds(&thing, s.rest().root, HoldOptions::default());
    let mut take = Take::new(thing, grip, 0.5);
    take.release = Some(Release::Throw { at: 2.5, velocity: Vec3::new(0.0, 2.0, 3.0), floor: 0.0 });
    let base = stand(&s);

    // before: nothing is touched
    let f = perform(&s, &base, &take, 0.2);
    assert_eq!(f.phase, Phase::Before);
    assert_eq!(f.pose, s.rest());
    assert_eq!(f.thing, thing);

    // from the grab to the release both palms stay on their holds
    let mut t = take.grab;
    while t < 2.5 {
        let f = perform(&s, &base, &take, t);
        assert!(f.miss < 5e-3, "t {t}: a wrist {} m off its hold", f.miss);
        for hold in grip.holds() {
            let rest = hand_rest(&s, hold.side).unwrap();
            let w = s.fk(&f.pose);
            let hand = s.arm(hold.side).unwrap()[2];
            let palm = w.pos[hand] + w.rot[hand] * rest.mid;
            assert!((palm - f.thing.world(hold.at)).length() < 6e-3, "t {t}: palm off the hold");
        }
        t += 1.0 / 30.0;
    }
    // carried: in front of the chest, level, and the fingers are on it and not in it
    let f = perform(&s, &base, &take, 2.0);
    assert_eq!(f.phase, Phase::Carry);
    assert!(f.thing.turn.angle_between(thing.turn) < 1e-3);
    for touch in f.touch {
        let touch = touch.unwrap();
        assert!(touch.on.iter().filter(|on| **on).count() >= 3, "{touch:?}");
        assert!(touch.inside < 0.012, "{touch:?}");
    }
    // thrown: it flies forward, lands on the floor, and the body is back to its own motion
    let f = perform(&s, &base, &take, 2.7);
    assert!(f.thing.at.z > perform(&s, &base, &take, 2.5).thing.at.z + 0.4);
    let f = perform(&s, &base, &take, 6.0);
    assert_eq!(f.phase, Phase::After);
    assert!((f.thing.at.y - f.thing.solid.bound()).abs() < 1e-3);
    assert_eq!(f.pose, s.rest());
}

#[test]
fn a_box_on_the_floor_is_taken_by_bending_with_the_feet_planted() {
    let s = mannequin();
    let thing = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(0.0, 0.1, 0.38));
    let take = Take::new(thing, find_holds(&thing, s.rest().root, HoldOptions::default()), 0.0);
    let base = stand(&s);
    let rest = s.fk(&s.rest());
    let feet = ["foot_l", "foot_r"].map(|n| s.joint(n).unwrap());
    let (mut t, mut lowest) = (0.0, f32::MAX);
    while t < 2.0 {
        let f = perform(&s, &base, &take, t);
        let w = s.fk(&f.pose);
        for foot in feet {
            assert!((w.pos[foot] - rest.pos[foot]).length() < 5e-3, "t {t}: a foot moved");
            assert!(w.rot[foot].dot(rest.rot[foot]).abs() > 0.9999, "t {t}: a foot turned");
        }
        lowest = lowest.min(f.pose.root.y);
        if t >= take.grab {
            assert!(f.miss < 0.02, "t {t}: a wrist {} m off its hold on the floor", f.miss);
        }
        t += 1.0 / 30.0;
    }
    // the hips came down for it, and are back up with the box carried
    assert!(lowest < s.rest().root.y - 0.3, "the hips came down to {lowest}");
    assert!(perform(&s, &base, &take, 1.9).pose.root.y > s.rest().root.y - 0.02);
}

#[test]
fn hands_stay_out_of_a_table() {
    let s = mannequin();
    let top = 0.95;
    let table = Thing::new(Solid::Box { half: Vec3::new(0.6, top / 2.0, 0.34) }, Vec3::new(0.0, top / 2.0, 0.66));
    let cup = Thing::new(Solid::Cylinder { half: 0.05, r: 0.035 }, Vec3::new(-0.2, top + 0.05, 0.42));
    let mut take = Take::new(cup, find_holds(&cup, s.rest().root, HoldOptions::default()), 0.0);
    take.release = Some(Release::Put { at: 1.7, down: 2.3, place: Vec3::new(0.0, top + 0.05, 0.44) });
    take.avoid = vec![table];
    let base = stand(&s);
    let hand = s.arm(Side::R).unwrap()[2];
    let rest = hand_rest(&s, Side::R).unwrap();
    let mut t = 0.0;
    while t < 3.0 {
        let f = perform(&s, &base, &take, t);
        let w = s.fk(&f.pose);
        for p in [Vec3::ZERO, rest.mid, rest.along * 0.17] {
            let d = table.distance(w.pos[hand] + w.rot[hand] * p);
            assert!(d > 0.0, "t {t}: the hand is {} m into the table", -d);
        }
        if t >= take.grab && t < 2.3 {
            assert!(f.miss < 8e-3, "t {t}: keeping clear cost the hold {} m", f.miss);
        }
        t += 1.0 / 30.0;
    }
}

#[test]
fn the_body_is_told_where_to_stand() {
    let thing = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(2.0, 1.0, 3.0));
    let (at, yaw) = stand_for(&thing, Vec3::new(2.0, 1.0, 0.0));
    // a forearm's length off the face nearest it, facing the thing
    assert!((at - Vec3::new(2.0, 1.0, 3.0 - 0.12 - 0.3)).length() < 2e-3, "{at}");
    assert!(yaw.abs() < 1e-3);
    // and standing there the thing is in reach of both hands
    let s = mannequin();
    let mut p = s.rest();
    p.root += at - Vec3::new(0.0, 1.0, 0.0);
    let here = move |_: f32| p.clone();
    let take = Take::new(thing, find_holds(&thing, at, HoldOptions::default()), 0.0);
    assert!(perform(&s, &here, &take, 1.0).miss < 5e-3);
}

#[test]
fn a_cup_is_put_down_somewhere_else_upright_while_the_body_turns() {
    let s = mannequin();
    let cup = Thing::new(Solid::Cylinder { half: 0.05, r: 0.035 }, Vec3::new(-0.2, 1.0, 0.38));
    let grip = find_holds(&cup, s.rest().root, HoldOptions::default());
    let mut take = Take::new(cup, grip, 0.0);
    let place = Vec3::new(0.25, 1.0, 0.36);
    take.release = Some(Release::Put { at: 2.0, down: 2.6, place });
    // the body turns a quarter to its left between 1 s and 2 s, and back
    let base = |t: f32| {
        let mut p = s.rest();
        let u = ((t - 1.0).clamp(0.0, 1.0) * std::f32::consts::PI).sin();
        p.turns[0] = Quat::from_rotation_y(0.6 * u);
        p
    };
    let mut t = 0.0;
    while t < 3.5 {
        let f = perform(&s, &base, &take, t);
        // upright the whole way (it is in the hand, so it tips a degree or two where the wrist is at its limit)
        let tilt = (f.thing.turn * Vec3::Y).angle_between(Vec3::Y).to_degrees();
        assert!(tilt < 3.0, "t {t}: the cup tilts {tilt} degrees");
        t += 1.0 / 30.0;
    }
    let f = perform(&s, &base, &take, 3.4);
    assert_eq!(f.phase, Phase::After);
    assert!((f.thing.at - place).length() < 1e-4);
    assert!(f.moved < 0.01, "the hand is back: {}", f.moved);
}

#[test]
fn any_frame_can_be_asked_for() {
    let s = mannequin();
    let thing = Thing::new(Solid::Ball { r: 0.12 }, Vec3::new(0.05, 1.1, 0.4));
    let take = Take::new(thing, find_holds(&thing, s.rest().root, HoldOptions::default()), 0.0);
    let base = stand(&s);
    // asked in any order, a frame is the same frame
    let a = perform(&s, &base, &take, 0.9);
    let _ = perform(&s, &base, &take, 0.1);
    let b = perform(&s, &base, &take, 0.9);
    assert_eq!(a.pose, b.pose);
    assert_eq!(a.thing, b.thing);
    // and no joint jumps between frames, a 60th of a second apart: the body's, and the fingers as they shut
    let (mut last, mut body, mut fingers, mut t) = (perform(&s, &base, &take, 0.0).pose, 0.0f32, 0.0f32, 0.0);
    let hands = s.joint("thumb_01_l").unwrap();
    while t < 1.6 {
        t += 1.0 / 60.0;
        let now = perform(&s, &base, &take, t).pose;
        for (i, (a, b)) in now.turns.iter().zip(&last.turns).enumerate() {
            let d = a.angle_between(*b).to_degrees();
            if i < hands { body = body.max(d) } else { fingers = fingers.max(d) }
        }
        last = now;
    }
    assert!(body < 6.0, "a joint of the body turned {body} degrees in one frame");
    assert!(fingers < 20.0, "a finger joint turned {fingers} degrees in one frame");
}

#[test]
fn a_take_asked_for_as_json_is_the_same_frame() {
    let s = mannequin();
    let joints: Vec<serde_json::Value> = s
        .joints
        .iter()
        .map(|j| serde_json::json!({"name": j.name, "parent": j.parent.map_or(-1, |p| p as i64), "head": j.head.to_array(), "tail": j.tail.to_array()}))
        .collect();
    let rest = s.rest();
    let turns: Vec<[f32; 4]> = rest.turns.iter().map(|q| q.to_array()).collect();
    let ask = |t: f32| {
        take_json(&serde_json::json!({
            "joints": joints, "t": t, "start": 0.5,
            "base": [{"t": t, "root": rest.root.to_array(), "turns": turns}],
            "thing": {"box": [0.16, 0.1, 0.12], "at": [0.0, 1.05, 0.42]},
            "throw": {"at": 2.5, "velocity": [0.0, 2.0, 3.0]},
        }))
        .unwrap()
    };
    let thing = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(0.0, 1.05, 0.42));
    let mut take = Take::new(thing, find_holds(&thing, rest.root, HoldOptions::default()), 0.5);
    take.release = Some(Release::Throw { at: 2.5, velocity: Vec3::new(0.0, 2.0, 3.0), floor: 0.0 });
    for t in [0.0, 0.8, 1.4, 2.0, 2.7] {
        let (json, direct) = (ask(t), perform(&s, &|_| s.rest(), &take, t));
        assert_eq!(json["phase"], format!("{:?}", direct.phase));
        assert_eq!(json["hands"], 2);
        let at = &json["thing"]["at"];
        assert!((Vec3::new(at[0].as_f64().unwrap() as f32, at[1].as_f64().unwrap() as f32, at[2].as_f64().unwrap() as f32) - direct.thing.at).length() < 1e-5);
        for (a, b) in json["pose"]["turns"].as_array().unwrap().iter().zip(&direct.pose.turns) {
            let q = Quat::from_xyzw(a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32, a[2].as_f64().unwrap() as f32, a[3].as_f64().unwrap() as f32);
            assert!(q.dot(*b).abs() > 0.999999, "t {t}: {q} against {b}");
        }
    }
    assert!(take_json(&serde_json::json!({"joints": joints, "t": 0, "start": 0, "base": [], "thing": {"at": [0, 0, 0]}})).is_err());
}

#[test]
fn a_pose_is_recovered_from_where_its_joints_were_seen() {
    let s = mannequin();
    let mut r = rng(11);
    let mut worst = 0.0f32;
    for _ in 0..20 {
        // a pose of the body: every joint turned up to 50 degrees, the body moved
        let mut seen = s.rest();
        seen.root += Vec3::new(r() - 0.5, r() * 0.2, r() - 0.5);
        for turn in seen.turns.iter_mut() {
            *turn = Quat::from_axis_angle(Vec3::new(r() - 0.5, r() - 0.5, r() - 0.5).normalize(), r() * 0.87);
        }
        let marks = marks_of(&s, &seen);
        let (pose, miss) = fit(&s, &s.rest(), &marks, 10);
        worst = worst.max(miss);
        // every joint is where it was seen
        let (a, b) = (s.fk(&pose), s.fk(&seen));
        for i in 0..s.joints.len() {
            assert!((a.pos[i] - b.pos[i]).length() < 4e-3, "{} is {} m off", s.joints[i].name, (a.pos[i] - b.pos[i]).length());
        }
    }
    assert!(worst < 2e-3, "root-mean-square miss {worst} m");
    // marks that are where the pose already has them change nothing
    let rest = s.rest();
    let (same, miss) = fit(&s, &rest, &marks_of(&s, &rest), 3);
    assert!(miss < 1e-5);
    for (a, b) in same.turns.iter().zip(&rest.turns) {
        assert!(a.dot(*b).abs() > 0.999999);
    }
}

#[test]
fn the_nearest_thing_in_sight_is_the_one_taken() {
    let (from, eye) = (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.7, 0.0));
    let ball = |x: f32, z: f32| Thing::new(Solid::Ball { r: 0.1 }, Vec3::new(x, 1.0, z));
    let things = [ball(0.0, 1.5), ball(0.3, 0.6), ball(0.0, 5.0)];
    assert_eq!(pick(&things, from, eye, 2.0, &[]), Some(1));
    // a wall between the body and the nearer one: the farther is taken; and nothing past the radius
    let wall = Thing::new(Solid::Box { half: Vec3::new(0.2, 1.5, 0.02) }, Vec3::new(0.35, 1.0, 0.35));
    assert_eq!(pick(&things, from, eye, 2.0, &[wall]), Some(0));
    assert_eq!(pick(&things[2..], from, eye, 2.0, &[]), None);
}

#[test]
fn the_head_turns_to_the_thing_and_a_wrist_stops_at_its_limit() {
    let s = mannequin();
    let head = s.joint("head").unwrap();
    let at = Vec3::new(0.6, 1.0, 0.5);
    let turned = look(&s, &s.rest(), at, 1.0);
    let (w0, w) = (s.fk(&s.rest()), s.fk(&turned));
    let toward = |w: &Posed| (w.rot[head] * Vec3::Z).dot((at - w.pos[head]).normalize());
    assert!(toward(&w) > toward(&w0) + 0.3, "the head faces the thing more: {} from {}", toward(&w), toward(&w0));
    assert!(w.rot[head].angle_between(w0.rot[head]).to_degrees() < LOOK_MOST + 0.5, "a glance, not a bow");
    assert_eq!(look(&s, &s.rest(), at, 0.0), s.rest());
    // SOMA's twist joints: a forearm's follow its hand by rising shares; an upper arm's undo the shoulder's twist
    let fore = twist_joints(Vec3::X, 0.0, 1.0, false);
    assert!((fore[3].to_axis_angle().1 - 0.95).abs() < 1e-4 && (fore[0].to_axis_angle().1 - 0.05).abs() < 1e-4);
    let upper = twist_joints(Vec3::X, 1.0, 0.0, true);
    assert!(upper[0].angle_between(Quat::from_rotation_x(-0.95)) < 1e-3);
}
