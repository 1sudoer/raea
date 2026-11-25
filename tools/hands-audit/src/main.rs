//! The hands' audit: a set of takes run on a real body (a character's skeleton.json, with its skin samples) and
//! measured on that skin, 60 frames a second. Nothing here judges; it prints what happened, worst first within a
//! take, so a claim that a take is right can be checked against numbers taken from the body itself.
//!   cargo run -p raea-hands-audit -- CHARACTER/skeleton.json

use glam::{Quat, Vec3};
use raea_sys_body::*;
use raea_sys_grab::*;

/// one take's worst numbers
#[derive(Default)]
struct Worst {
    /// skin of the hands, and of the rest of the body, inside the thing, metres
    hand_in_thing: f32,
    body_in_thing: f32,
    /// any skin inside the scene's solids; hands and forearms inside the body's own trunk and thighs
    in_scene: f32,
    in_self: f32,
    /// while held: the fewest finger pads on the thing (of the hands holding), and the widest palm gap
    fewest_pads: usize,
    palm_gap: f32,
    miss: f32,
    /// degrees a second, and the biggest change of that in one frame (a pop), body joints then finger joints
    speed: [f32; 2],
    jump: [f32; 2],
    limited: usize,
    /// the most the trunk pitched forward, degrees, and the lowest the hips went, as a share of standing
    pitch: f32,
    hips: f32,
    where_worst: String,
}

/// whether a part of the skin (a sample's name) is a hand's
fn hands_part(part: &str) -> bool {
    part == "palm" || ["thumb", "index", "middle", "ring", "pinky"].iter().any(|f| part.starts_with(f))
}

fn audit(skel: &Skeleton, name: &str, base: &dyn Fn(f32) -> Pose, take: &Take, until: f32) {
    let arm_part = |part: &str| hands_part(part) || part == "foreArm";
    let fingers_from = skel.joint("thumb_01_l").unwrap_or(usize::MAX);
    let chest = skel.joint("spine_03").unwrap_or(0);
    let stand = skel.fk(&base(0.0)).pos[0].y;
    let mut w = Worst { fewest_pads: usize::MAX, hips: 1.0, ..Default::default() };
    let (mut last, mut rate): (Option<Pose>, Vec<f32>) = (None, vec![0.0; skel.joints.len()]);
    let frames = (until * 60.0) as usize;
    for i in 0..=frames {
        let t = i as f32 / 60.0;
        let f = perform(skel, base, take, t);
        let at = skel.fk(&f.pose);
        let skin = skel.skin(&at);
        let own = own_solids(skel, &at);
        let held = matches!(f.phase, Phase::Lift | Phase::Carry | Phase::Release);
        let (mut pads, mut gap) = (0usize, f32::MAX);
        let note = |value: f32, slot: &mut f32, what: &str, w_where: &mut String| {
            if value > *slot {
                *slot = value;
                if value > 0.005 {
                    *w_where = format!("{what} at {t:.2} s");
                }
            }
        };
        let mut touching = std::collections::HashSet::new();
        for (sample, p) in skel.surface.iter().zip(&skin) {
            let d = f.thing.distance(*p);
            if f.phase != Phase::After {
                if hands_part(&sample.part) {
                    note(-d, &mut w.hand_in_thing, &format!("{} in the thing", sample.part), &mut w.where_worst);
                } else {
                    note(-d, &mut w.body_in_thing, &format!("{} in the thing", sample.part), &mut w.where_worst);
                }
            }
            for solid in &take.avoid {
                note(-solid.distance(*p), &mut w.in_scene, &format!("{} in the scene", sample.part), &mut w.where_worst);
            }
            if arm_part(&sample.part) {
                for solid in &own {
                    note(-solid.distance(*p), &mut w.in_self, &format!("{} in the body", sample.part), &mut w.where_worst);
                }
            }
            if held && d < 0.006 && sample.part.ends_with('3') {
                touching.insert((sample.part.clone(), sample.joints[0].0));
            }
            // (the palm's skin and the ball of the thumb, which lies on a face with it)
            if held && (sample.part == "palm" || sample.part == "thumb1") {
                gap = gap.min(d);
            }
        }
        // FINGERS=1: each part of the hands, how near its skin is to the thing a third of a second into the hold
        if std::env::var("FINGERS").is_ok() && held && (t - (take.grab + 0.3)).abs() < 0.009 {
            let mut by: std::collections::BTreeMap<String, f32> = Default::default();
            for (sample, p) in skel.surface.iter().zip(&skin) {
                if hands_part(&sample.part) {
                    let e = by.entry(format!("{}{}", sample.part, if p.x > at.pos[0].x { "L" } else { "R" })).or_insert(f32::MAX);
                    *e = e.min(f.thing.distance(*p));
                }
            }
            eprintln!("{name} at {t:.2}: nearest skin of each part to the thing, mm: {}", by.iter().map(|(k, v)| format!("{k} {:.0}", v * 1000.0)).collect::<Vec<_>>().join(", "));
        }
        pads += touching.len();
        if held && t >= take.grab + 0.05 {
            w.fewest_pads = w.fewest_pads.min(pads);
            w.palm_gap = w.palm_gap.max(gap);
            w.miss = w.miss.max(f.miss);
        }
        if let Ok(span) = std::env::var("WATCH") {
            // WATCH="mug|1.82|1.92": the right arm's angles and the hand's world turn over those seconds of that take
            let mut words = span.split('|');
            let (take_name, from, to) = (words.next().unwrap_or(""), words.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0), words.next().and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0));
            if name.starts_with(take_name) && t >= from && t <= to {
                let arm = skel.arm(Side::R).unwrap();
                let angles = arm.map(|j| angles_of(skel, j, f.pose.turns[j]).map(|a| (a.flex.round(), a.side.round(), a.twist.round())));
                let deepest = skel.surface.iter().zip(&skin).map(|(x, p)| (-f.thing.distance(*p), x.part.as_str())).fold((f32::MIN, ""), |a, b| if b.0 > a.0 { b } else { a });
                eprintln!("  {t:.3} {:?} miss {:.3} thing {:.2?} deepest {} {:.0} mm hips {:.2}", f.phase, f.miss, f.thing.at.to_array(), deepest.1, deepest.0 * 1000.0, at.pos[0].y);
                eprintln!("  {t:.3} {:?} limited {} hand world {:.2?} thing fwd {:.2?} wrist {:.3?} elbow {:.3?}", angles, f.limited, at.rot[arm[2]].to_array(), (f.thing.turn * Vec3::Z).to_array(), at.pos[arm[2]].to_array(), at.pos[arm[1]].to_array());
            }
        }
        w.limited += f.limited as usize;
        let up = (at.pos[chest] - at.pos[0]).normalize();
        w.pitch = w.pitch.max(up.angle_between(Vec3::Y).to_degrees());
        w.hips = w.hips.min(at.pos[0].y / stand);
        if let Some(before) = &last {
            for (j, (a, b)) in f.pose.turns.iter().zip(&before.turns).enumerate() {
                let speed = a.angle_between(*b).to_degrees() * 60.0;
                let k = (j >= fingers_from) as usize;
                w.speed[k] = w.speed[k].max(speed);
                if i > 1 {
                    if (speed - rate[j]).abs() > std::env::var("POPS").ok().and_then(|x| x.parse::<f32>().ok()).unwrap_or(f32::MAX) {
                        eprintln!("  pop {name}: {} at {t:.3} s: {:.0} to {speed:.0} deg/s ({:?})", skel.joints[j].name, rate[j], f.phase);
                    }
                    w.jump[k] = w.jump[k].max((speed - rate[j]).abs());
                }
                rate[j] = speed;
            }
        }
        last = Some(f.pose);
    }
    println!(
        "{name:<22} skin in thing: hand {:>4.0} mm, body {:>4.0} mm | in scene {:>4.0} mm | in self {:>4.0} mm | pads on {:>2}, palm gap {:>3.0} mm, hold off {:>3.0} mm | body {:>4.0} deg/s (jump {:>4.0}), fingers {:>4.0} (jump {:>4.0}) | limited {:>3} frames | pitch {:>2.0} deg, hips {:>3.0}% | {}",
        w.hand_in_thing * 1000.0, w.body_in_thing * 1000.0, w.in_scene * 1000.0, w.in_self * 1000.0,
        if w.fewest_pads == usize::MAX { 0 } else { w.fewest_pads }, w.palm_gap.min(9.99) * 1000.0, w.miss * 1000.0,
        w.speed[0], w.jump[0], w.speed[1], w.jump[1], w.limited, w.pitch, w.hips * 100.0, w.where_worst
    );
}

fn main() {
    let path = std::env::args().nth(1).expect("a character's skeleton.json");
    let skel = Skeleton::from_json(&std::fs::read_to_string(&path).expect("the skeleton read")).expect("a skeleton");
    println!("{} joints, {} skin samples; every number is the worst over the take, 60 frames a second", skel.joints.len(), skel.surface.len());
    // the body's own motion: standing, arms hanging (the rest pose holds them out), as near the sides as keeps the
    // hands' skin out of the thighs (at a fixed 0.12 SOMA's fingers hung 31 mm inside them before any take began)
    let hang = |out: f32| {
        let mut stand = skel.rest();
        for side in Side::BOTH {
            if let Some([upper, fore, _]) = skel.arm(side) {
                let down = |i: usize, to: Vec3| Quat::from_rotation_arc((skel.joints[i].tail - skel.joints[i].head).normalize(), to.normalize());
                stand.turns[upper] = down(upper, Vec3::new(out * side.sign(), -1.0, 0.0));
                stand.turns[fore] = (stand.turns[upper].inverse() * down(fore, Vec3::new(0.05 * side.sign(), -1.0, 0.12))).normalize();
            }
        }
        stand
    };
    let inside = |pose: &Pose| {
        let at = skel.fk(pose);
        let own = own_solids(&skel, &at);
        skel.surface.iter().zip(skel.skin(&at)).filter(|(x, _)| hands_part(&x.part) || x.part == "foreArm").any(|(_, p)| own.iter().any(|s| s.distance(p) < 0.005))
    };
    let stand = (3..=12).map(|k| hang(k as f32 * 0.04)).find(|p| !inside(p)).unwrap_or_else(|| hang(0.48));
    let base = move |_: f32| stand.clone();
    let root = base(0.0).root;
    let top = 0.95;
    let table = Thing::new(Solid::Box { half: Vec3::new(0.6, top / 2.0, 0.34) }, Vec3::new(0.0, top / 2.0, 0.72));
    let found = |thing: &Thing| find_holds(thing, root, HoldOptions::default());

    let crate_ = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(0.0, top + 0.1, 0.5));
    let mut t = Take::new(crate_, found(&crate_), 0.4);
    t.avoid = vec![table];
    t.release = Some(Release::Throw { at: 2.3, velocity: Vec3::new(0.0, 1.6, 2.6), floor: 0.0 });
    audit(&skel, "box, taken and thrown", &base, &t, 3.2);

    let cup = Thing::new(Solid::Cylinder { half: 0.05, r: 0.035 }, Vec3::new(-0.2, top + 0.05, 0.46));
    let mut t = Take::new(cup, found(&cup), 0.4);
    t.avoid = vec![table];
    t.release = Some(Release::Put { at: 1.9, down: 2.5, place: Vec3::new(0.02, top + 0.05, 0.48) });
    audit(&skel, "cup, moved", &base, &t, 3.3);

    // a box on the floor where a body would stand for it (a forearm's length in front of the feet), lifted onto a
    // table that is a top with room under it. (The first audits had the box on the body's toes with a solid table
    // 38 cm ahead: 17 cm of floor for a 24 cm box, a take no body could make.)
    let low = Thing::new(Solid::Box { half: Vec3::new(0.16, 0.1, 0.12) }, Vec3::new(0.0, 0.1, 0.44));
    let slab = Thing::new(Solid::Box { half: Vec3::new(0.6, 0.02, 0.3) }, Vec3::new(0.0, top - 0.02, 0.8));
    let mut t = Take::new(low, found(&low), 0.4);
    (t.grab, t.carried, t.avoid) = (1.2, 2.0, vec![slab]);
    t.release = Some(Release::Put { at: 2.2, down: 2.9, place: Vec3::new(0.0, top + 0.1, 0.62) });
    audit(&skel, "box, floor to table", &base, &t, 3.6);

    let hold = Hold { side: Side::R, at: Vec3::new(-0.035, 0.0, 0.0), normal: Vec3::NEG_X, along: None, shape: Some([-1.2, 0.0, 0.0]) };
    let mut t = Take::new(cup, Grip::One(hold), 0.3);
    (t.grab, t.carried, t.avoid) = (0.9, 1.4, vec![table]);
    t.path = vec![Key { t: 2.0, carry: Vec3::new(-0.02, 0.36, 0.24), tilt: Quat::from_rotation_x(-0.5) }];
    t.release = Some(Release::Put { at: 2.4, down: 3.1, place: cup.at });
    audit(&skel, "mug, drunk from", &base, &t, 3.8);

    let mut t = Take::new(cup, found(&cup), 0.3).passing(2.0);
    t.avoid = vec![table];
    audit(&skel, "cup, hand to hand", &base, &t, 3.5);

    let mut t = Take::new(crate_, found(&crate_), 0.4).weighing(14.0);
    t.avoid = vec![table];
    audit(&skel, "box, 14 kg", &base, &t, 3.5);
}
