//! The body's own measures (docs/hands.feature): joint limits and the hand prior, on the stand-in body.

use glam::{Quat, Vec3};
use raea_sys_body::prior::{coefficients, oddness, shape};
use raea_sys_body::*;

#[test]
fn limits_read_and_hold_a_joint() {
    let s = mannequin();
    // the rest pose is inside every range
    assert!(violations(&s, &s.rest(), 0.01).is_empty(), "{:?}", violations(&s, &s.rest(), 0.01));
    for side in Side::BOTH {
        let [upper, elbow, hand] = s.arm(side).unwrap();
        // an elbow's fold reads as flex alone, the same on both sides; bent back it is past its range, and is brought to it
        let hinge = s.offset(elbow).cross(s.offset(hand)).normalize();
        let a = angles_of(&s, elbow, Quat::from_axis_angle(hinge, 1.0)).unwrap();
        assert!((a.flex - 57.3).abs() < 1.0 && a.side.abs() < 1.0 && a.twist.abs() < 1.0, "{a:?}");
        let back = Quat::from_axis_angle(hinge, -1.2);
        let mut p = s.rest();
        p.turns[elbow] = back;
        assert_eq!(violations(&s, &p, 0.5)[0].0, elbow);
        let (held, by) = clamp_turn(&s, elbow, back);
        assert!((angles_of(&s, elbow, held).unwrap().flex + 50.0).abs() < 0.5 && by > 15.0);
        // a turn inside its range comes back as it is
        let fine = Quat::from_axis_angle(Vec3::Z, 0.3 * side.sign());
        assert_eq!(clamp_turn(&s, upper, fine), (fine, 0.0));
        // a turn reads the same whichever sign its numbers carry (it read the long way round, 329 degrees for 31, and
        // was then held to a limit it was nowhere near)
        let small = Quat::from_axis_angle(Vec3::new(0.3, 0.5, 0.8).normalize(), 0.5);
        assert!((clamp_turn(&s, hand, -small).1 - clamp_turn(&s, hand, small).1).abs() < 0.01 && clamp_turn(&s, hand, small).1 < 10.0);
        let (a, b) = (angles_of(&s, hand, small).unwrap(), angles_of(&s, hand, -small).unwrap());
        assert!((a.flex - b.flex).abs() < 0.01 && (a.side - b.side).abs() < 0.01 && (a.twist - b.twist).abs() < 0.01, "{a:?} {b:?}");
        // a wrist's twist past 25 degrees is held there
        let twisted = Quat::from_axis_angle((s.joints[hand].tail - s.joints[hand].head).normalize(), 0.9 * side.sign());
        let (held, _) = clamp_turn(&s, hand, twisted);
        assert!((angles_of(&s, hand, held).unwrap().twist - 25.0).abs() < 0.5);
    }
}

#[test]
fn the_hand_prior_shapes_and_reads_hands() {
    let s = mannequin();
    for side in Side::BOTH {
        let rest = hand_rest(&s, side).unwrap();
        let hand = s.arm(side).unwrap()[2];
        // how far each fingertip is from its knuckle, on average: longest with the hand open, shortest in a fist
        let span = |pose: &Pose| {
            let w = s.fk(pose);
            ["index", "middle", "ring", "pinky"].iter().map(|f| {
                let [knuckle, _, tip] = s.finger(f, side).unwrap();
                (w.pos[tip] + w.rot[tip] * (s.joints[tip].tail - s.joints[tip].head) - w.pos[knuckle]).length()
            }).sum::<f32>() / 4.0
        };
        let shaped = |z: &[f32]| {
            let mut p = s.rest();
            shape(&s, &mut p.turns, side, z).unwrap();
            p
        };
        let (mean, open, shut) = (shaped(&[]), shaped(&[2.0]), shaped(&[-2.0]));
        // the first component opens and closes the hand, the average hand between: open at +2, a fist at -2
        assert!(span(&open) > span(&mean) && span(&mean) > span(&shut) + 0.004, "{} {} {}", span(&open), span(&mean), span(&shut));
        // and the fingers close toward the palm, not away from it
        let w = s.fk(&shut);
        let [knuckle, _, tip] = s.finger("middle", side).unwrap();
        assert!((w.pos[tip] - w.pos[knuckle]).dot(w.rot[hand] * rest.palm) > 0.01);
        // a shape reads back as the numbers that made it, with nothing left over
        let z = [0.8, -0.5, 0.3, 1.1, -0.7];
        let (back, left) = coefficients(&s, &shaped(&z), side).unwrap();
        for (a, b) in z.iter().zip(&back) {
            assert!((a - b).abs() < 0.05, "{z:?} read back as {:?}", &back[..5]);
        }
        assert!(left < 0.5, "{left} degrees left over");
        // the average hand is the least odd; the rig's rest hand reads about 2 (it isn't SOMA's hand); fingers bent
        // back read over twice that
        assert!(oddness(&s, &mean, side).unwrap() < 0.05);
        assert!(oddness(&s, &s.rest(), side).unwrap() < 3.0);
        let mut broken = s.rest();
        for f in ["index", "middle", "ring", "pinky"] {
            let j = s.finger(f, side).unwrap()[1];
            let axis = (s.joints[j].tail - s.joints[j].head).normalize().cross(rest.palm).normalize();
            broken.turns[j] = Quat::from_axis_angle(axis, -1.2);
        }
        let (odd, usual) = (oddness(&s, &broken, side).unwrap(), oddness(&s, &s.rest(), side).unwrap());
        assert!(odd > 2.0 * usual, "fingers bent back read {odd}, the rest hand {usual}");
    }
}

#[test]
fn soma_names_map_onto_the_rig() {
    for (soma, raea) in [("Hips", "pelvis"), ("Chest", "spine_03"), ("LeftForeArm", "lowerarm_l"), ("RightShin", "calf_r"), ("LeftToeBase", "ball_l"), ("LeftHandThumb1", "thumb_01_l"), ("RightHandIndex2", "index_01_r"), ("LeftHandPinky4", "pinky_03_l")] {
        assert_eq!(raea_joint(soma).as_deref(), Some(raea));
    }
    // what the rig has no joint for
    for soma in ["Root", "Neck2", "Jaw", "LeftEye", "LeftHandIndex1", "RightHandMiddleEnd", "LeftToeEnd"] {
        assert_eq!(raea_joint(soma), None, "{soma}");
    }
}

#[test]
fn a_soma_body_of_other_proportions_is_followed_bone_for_bone() {
    let s = mannequin();
    // a "SOMA" body: the rig's own joints under SOMA's names, 1.3 times the size and in centimetres, posed
    let soma_name = |raea: &str| {
        ["Hips", "Spine1", "Spine2", "Chest", "Neck1", "Head"].iter().map(|n| n.to_string()).chain(["Left", "Right"].iter().flat_map(|side| {
            ["Shoulder", "Arm", "ForeArm", "Hand", "Leg", "Shin", "Foot", "ToeBase"].iter().map(move |n| format!("{side}{n}")).chain(["Thumb1", "Thumb2", "Thumb3"].iter().map(move |n| format!("{side}Hand{n}"))).chain(["Index", "Middle", "Ring", "Pinky"].iter().flat_map(move |f| [2, 3, 4].map(move |k| format!("{side}Hand{f}{k}"))))
        })).find(|n| raea_joint(n).as_deref() == Some(raea))
    };
    let mut r = {
        let mut seed = 5u32;
        move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / 16777216.0
        }
    };
    let mut seen = s.rest();
    seen.root += Vec3::new(0.4, -0.1, 1.0);
    for (i, turn) in seen.turns.iter_mut().enumerate() {
        if s.joints[i].parent.is_some() && !s.joints[i].name.contains("_0") {
            *turn = Quat::from_axis_angle(Vec3::new(r() - 0.5, r() - 0.5, r() - 0.5).normalize(), r() * 0.7);
        }
    }
    let w = s.fk(&seen);
    let (mut names, mut at) = (Vec::new(), Vec::new());
    for (i, joint) in s.joints.iter().enumerate() {
        names.push(soma_name(&joint.name).unwrap());
        at.push(w.pos[i] * 130.0);
    }
    let pose = from_soma(&s, &names, &at, None).unwrap();
    let got = s.fk(&pose);
    // every bone of the body points as the SOMA body's does, and it stands where that body does (to scale)
    for (i, joint) in s.joints.iter().enumerate() {
        let Some(parent) = joint.parent else { continue };
        let (ours, theirs) = ((got.pos[i] - got.pos[parent]).normalize(), (w.pos[i] - w.pos[parent]).normalize());
        assert!(ours.angle_between(theirs).to_degrees() < 2.0, "{} points {} degrees off", joint.name, ours.angle_between(theirs).to_degrees());
    }
    assert!((got.pos[0] - w.pos[0]).length() < 0.01);
    // and its elbows and knees are not bent sideways
    for name in ["lowerarm_l", "lowerarm_r", "calf_l", "calf_r"] {
        let j = s.joint(name).unwrap();
        let side = angles_of(&s, j, pose.turns[j]).unwrap().side;
        assert!(side.abs() < 25.0, "{name} bends {side} degrees sideways");
    }
}

#[test]
fn a_soma_body_takes_a_motions_turns_and_spreads_twist_along_its_limbs() {
    use raea_sys_body::rig::Joint;
    // one arm of a SOMA body: a chest, an upper arm and forearm along +x, each with four twist joints, and a hand
    let j = |name: &str, parent: Option<usize>, x: f32| Joint { name: name.into(), parent, head: Vec3::new(x, 1.4, 0.0), tail: Vec3::new(x + 0.05, 1.4, 0.0) };
    let mut joints = vec![j("Chest", None, 0.0), j("LeftArm", Some(0), 0.2), j("LeftForeArm", Some(1), 0.5), j("LeftHand", Some(2), 0.75)];
    for k in 0..4 {
        joints.push(j(&format!("LeftArmTwist{}", k + 1), Some(1), 0.2 + 0.06 * k as f32));
        joints.push(j(&format!("LeftForeArmTwist{}", k + 1), Some(2), 0.5 + 0.05 * k as f32));
    }
    let s = Skeleton::new(joints);
    let names: Vec<String> = ["Chest", "LeftArm", "LeftForeArm", "LeftHand"].map(String::from).to_vec();
    let about_x = |q: Quat| 2.0 * q.x.atan2(q.w);
    // the hand turned 1.2 about the forearm: the forearm's twist joints take 5 per cent to 95 per cent of it
    let turn = Quat::from_axis_angle(Vec3::X, 1.2);
    let p = from_turns(&s, &Default::default(), &names, &[Quat::IDENTITY, Quat::IDENTITY, Quat::IDENTITY, turn], Vec3::new(0.0, 1.4, 0.0));
    let w = s.fk(&p);
    for (k, share) in [0.05, 1.0 / 3.0, 2.0 / 3.0, 0.95].into_iter().enumerate() {
        let t = s.joint(&format!("LeftForeArmTwist{}", k + 1)).unwrap();
        assert!((about_x(w.rot[t]) - 1.2 * share).abs() < 1e-3, "forearm twist {k}: {}", about_x(w.rot[t]));
        assert!(about_x(w.rot[s.joint(&format!("LeftArmTwist{}", k + 1)).unwrap()]).abs() < 1e-3);
    }
    assert!((about_x(w.rot[3]) - 1.2).abs() < 1e-3 && w.pos[3].distance(Vec3::new(0.75, 1.4, 0.0)) < 1e-5);
    // the whole arm turned 0.8 about itself at the shoulder: the skin near the shoulder stays with the chest
    let p = from_turns(&s, &Default::default(), &names, &[Quat::IDENTITY, Quat::from_axis_angle(Vec3::X, 0.8), Quat::from_axis_angle(Vec3::X, 0.8), Quat::from_axis_angle(Vec3::X, 0.8)], Vec3::ZERO);
    let w = s.fk(&p);
    for (k, share) in [0.05, 1.0 / 3.0, 2.0 / 3.0, 0.95].into_iter().enumerate() {
        let t = s.joint(&format!("LeftArmTwist{}", k + 1)).unwrap();
        assert!((about_x(w.rot[t]) - 0.8 * share).abs() < 1e-3, "upper twist {k}: {}", about_x(w.rot[t]));
    }
    // a joint's turn to the T-pose comes before the motion's turn, and a joint the motion doesn't name rides its parent
    let tpose = std::collections::HashMap::from([("LeftArm".to_string(), Quat::from_axis_angle(Vec3::Z, 0.5))]);
    let p = from_turns(&s, &tpose, &names[..2].to_vec(), &[Quat::IDENTITY, Quat::from_axis_angle(Vec3::Y, 0.3)], Vec3::new(0.0, 0.1, 0.0));
    let w = s.fk(&p);
    let want = Quat::from_axis_angle(Vec3::Y, 0.3) * Quat::from_axis_angle(Vec3::Z, 0.5);
    assert!(w.rot[1].dot(want).abs() > 0.99999 && w.rot[2].dot(want).abs() > 0.99999 && p.root == Vec3::new(0.0, 0.1, 0.0));
}
