//! Taking a thing: reach, grab, lift, carry, then put it down or throw it. The whole action is a function of time
//! (docs/character.md, principle 9): any frame can be asked for, and a render and live play agree. The body's own
//! motion comes in as `base` (a clip, a walk, a stand); this only moves what the hands need, and says how much.
//!
//! The stages are musthafa45/Ik-Object-Holding's (MIT): hands eased onto the hold points, the thing drawn to a carry
//! place in front of the chest, both let go on a drop or a throw. There the easing is a per-frame lerp, which depends
//! on the frame rate and on every frame before; here each ease is a curve of time.

use glam::{Quat, Vec3};

use raea_sys_body::arm::{ArmOptions, forearm_twist, reach};
use crate::grip::{lie_at_grab, past_range, swivel};
use crate::trunk::{clear, give_for, hand_points, look, of_the_arm, own_solids, room, spread_for, stoop, torso};
use raea_sys_body::limits::clamp_turn;
use raea_sys_body::prior::shape;
use crate::hand::{Grip, Hold, Touch, curl};
use raea_sys_body::rig::hand_rest;
use raea_sys_body::rig::{Pose, Posed, Skeleton};
use raea_sys_body::shape::Thing;

#[derive(Clone, Copy, Debug)]
pub enum Release {
    /// set down: from `at` the thing is moved to `place`, standing there from `down` on, and the hands leave it
    Put { at: f32, down: f32, place: Vec3 },
    /// let go at `at` with a velocity, metres a second (none: a drop); it falls to `floor`. A throw is wound up:
    /// the thing drawn back against the throw and brought through, and the hands follow it out
    Throw { at: f32, velocity: Vec3, floor: f32 },
    /// the hands leave it where it is at `at` (a drawer pushed shut, a door opened)
    Leave { at: f32 },
}

#[derive(Clone, Debug)]
pub struct Take {
    /// the thing where it stands before it is taken
    pub thing: Thing,
    pub grip: Grip,
    /// when the hands leave the body's own motion, when they close on the thing, and when it has reached the carry
    pub start: f32,
    pub grab: f32,
    pub carried: f32,
    /// where the thing's centre rides, from the chest: x to the body's left, y up, z the way it faces
    pub carry: Vec3,
    pub release: Option<Release>,
    /// the scene's solids the hands keep out of (a table, a shelf)
    pub avoid: Vec<Thing>,
    /// what is done with the thing once it is carried: places for it to be at times, the hands following
    pub path: Vec<Key>,
    /// how heavy it is, kilograms (0: no matter)
    pub weight: f32,
    /// a thing that isn't carried but moved where it is (a drawer, a door, a lever): where it is at times after the
    /// grab, in the frame the body is posed in; the hands stay on it
    pub slide: Vec<(f32, Vec3, Quat)>,
    /// when a thing held in one hand is passed to the other
    pub pass: Option<f32>,
}

/// One hand's part in a take: its hold, when it leaves the body's own motion, when it is on, and when it lets go.
#[derive(Clone, Copy)]
pub(crate) struct Part {
    pub(crate) hold: Hold,
    pub(crate) start: f32,
    pub(crate) grab: f32,
    pub(crate) gone: Option<f32>,
}

/// Where a carried thing is at a time: off the chest as `carry` is, and tilted from how it is carried (the tilt in
/// the body's own axes: about x it tips toward the body or away).
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub t: f32,
    pub carry: Vec3,
    pub tilt: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Before,
    Reach,
    Lift,
    Carry,
    Release,
    After,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub pose: Pose,
    pub thing: Thing,
    pub phase: Phase,
    /// the worst a wrist ended from where its hold asks, metres
    pub miss: f32,
    /// the most any joint was turned from the body's own motion, degrees
    pub moved: f32,
    pub touch: [Option<Touch>; 2],
    /// a joint of an arm was held at its limit, so a palm may not be square on its hold
    pub limited: bool,
}

const GRAVITY: f32 = 9.81;
/// how long the hands take to go back to the body's own motion after letting go, seconds
const AWAY: f32 = 0.6;
/// how long the fingers take to shut, ending at the grab, seconds
const SHUT: f32 = 0.25;
/// how long a throw's wind-up is, ending at the release, seconds
const WIND: f32 = 0.4;
/// how open the hand comes in, in the hand prior's first component (its standard deviations)
const OPEN: f32 = 1.5;

pub(crate) fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

pub(crate) fn span(t: f32, a: f32, b: f32) -> f32 {
    if b <= a { (t >= b) as u8 as f32 } else { ((t - a) / (b - a)).clamp(0.0, 1.0) }
}

/// the turn about the vertical that the body faces by
pub(crate) fn yaw(w: &Posed) -> Quat {
    let f = w.rot[0] * Vec3::Z;
    Quat::from_rotation_y(f.x.atan2(f.z))
}

impl Take {
    /// A take with the usual timing: reach for 0.6 s from `start`, lift for 0.5 s, carried in front of the belly (a
    /// one-handed thing, off to its hand's side).
    pub fn new(thing: Thing, grip: Grip, start: f32) -> Self {
        let carry = match grip {
            Grip::One(h) => Vec3::new(0.2 * h.side.sign(), -0.18, 0.3),
            // against the belly, the elbows bent: nearer and higher the smaller it is
            Grip::Two(..) => Vec3::new(0.0, -0.1, 0.2 + thing.solid.bound() * 0.5),
        };
        Take { thing, grip, start, grab: start + 0.6, carried: start + 1.1, carry, release: None, avoid: Vec::new(), path: Vec::new(), weight: 0.0, slide: Vec::new(), pass: None }
    }

    /// The take of a thing that weighs `kg`: held lower and closer, lifted slower, the trunk leaning back against it.
    pub fn weighing(mut self, kg: f32) -> Self {
        self.weight = kg.max(0.0);
        self.carry -= Vec3::new(0.0, (0.01 * self.weight).min(0.12), (0.004 * self.weight).min(0.05));
        self.carried = self.grab + (self.carried - self.grab) * (1.0 + self.weight / 8.0);
        self
    }

    /// The take of a thing in one hand that is passed to the other at `at`: brought to the middle, taken by the
    /// other hand on its far side, and carried on that side.
    pub fn passing(mut self, at: f32) -> Self {
        if matches!(self.grip, Grip::One(_)) {
            self.pass = Some(at);
            let (middle, other) = (Vec3::new(0.0, self.carry.y, self.carry.z), Vec3::new(-self.carry.x, self.carry.y, self.carry.z));
            self.path.push(Key { t: at - 0.15, carry: middle, tilt: Quat::IDENTITY });
            self.path.push(Key { t: at + 0.3, carry: middle, tilt: Quat::IDENTITY });
            self.path.push(Key { t: at + 0.9, carry: other, tilt: Quat::IDENTITY });
        }
        self
    }

    /// each hand's part: both hands of a two-handed take together; a pass's second hand from half a second before
    fn parts(&self) -> Vec<Part> {
        let end = self.let_go();
        match (self.grip, self.pass) {
            (Grip::One(h), Some(pass)) => vec![
                Part { hold: h, start: self.start, grab: self.grab, gone: Some(pass + 0.1) },
                Part { hold: h.across(), start: pass - 0.5, grab: pass, gone: end },
            ],
            _ => self.grip.holds().map(|hold| Part { hold, start: self.start, grab: self.grab, gone: end }).collect(),
        }
    }

    /// where the carried thing is off the chest at `t`, and its tilt: the carry place, then the path's keys, eased
    /// from each to the next
    fn held_at(&self, t: f32) -> (Vec3, Quat) {
        let mut from = Key { t: self.carried, carry: self.carry, tilt: Quat::IDENTITY };
        for key in &self.path {
            if t < key.t {
                let u = smooth(span(t, from.t, key.t));
                return (from.carry.lerp(key.carry, u), from.tilt.slerp(key.tilt, u));
            }
            from = *key;
        }
        (from.carry, from.tilt)
    }

    /// where the thing rides at a moment of the body's motion: off the chest, turned only with the body's facing (and
    /// the path's tilt), so what stood upright is carried upright
    fn riding(&self, skel: &Skeleton, base: &dyn Fn(f32) -> Pose, t: f32) -> Thing {
        let (now, then) = (skel.fk(&base(t)), skel.fk(&base(self.grab)));
        let chest = skel.joint("spine_03").unwrap_or(0);
        let (y, y0) = (yaw(&now), yaw(&then));
        let (carry, tilt) = self.held_at(t);
        self.thing.moved(now.pos[chest] + y * carry, (y * tilt * y0.inverse() * self.thing.turn).normalize())
    }

    /// where the thing is at `t`, and whether the hands are on it
    pub(crate) fn thing_at(&self, skel: &Skeleton, base: &dyn Fn(f32) -> Pose, t: f32) -> (Thing, Phase) {
        if t < self.start {
            return (self.thing, Phase::Before);
        }
        if t < self.grab {
            return (self.thing, Phase::Reach);
        }
        let carried = |t: f32| {
            // a thing moved where it is: from where it stood through its places, eased
            if !self.slide.is_empty() {
                let mut from = (self.grab, self.thing.at, self.thing.turn);
                for key in &self.slide {
                    if t < key.0 {
                        let u = smooth(span(t, from.0, key.0));
                        return self.thing.moved(from.1.lerp(key.1, u), from.2.slerp(key.2, u));
                    }
                    from = *key;
                }
                return self.thing.moved(from.1, from.2);
            }
            let (ride, u) = (self.riding(skel, base, t), smooth(span(t, self.grab, self.carried)));
            // lifted in a low arc, so it leaves what it stood on before it travels; and over whatever is in its
            // way, by how deep its corners would be in it
            let rise = (std::f32::consts::PI * span(t, self.grab, self.carried)).sin();
            let mut thing = self.thing.moved(self.thing.at.lerp(ride.at, u) + Vec3::Y * 0.04 * rise, self.thing.turn.slerp(ride.turn, u));
            let deep = thing.corners().iter().flat_map(|c| self.avoid.iter().map(|s| 0.01 - s.distance(*c))).fold(0.0f32, f32::max);
            thing.at.y += deep * (rise * 4.0).min(1.0);
            // a throw is wound up: drawn back against the throw over its last moments, and brought through
            if let Some(Release::Throw { at, velocity, .. }) = self.release {
                let wind = (std::f32::consts::PI * span(t, at - WIND, at)).sin() * (velocity.length() / 3.0).min(1.0);
                thing.at -= velocity.normalize_or_zero() * 0.18 * wind;
            }
            // and out of the body, the way it faces, by how deep it would be in it: its corners and the wrists that
            // hold it (a hand's length behind each hold) against the trunk and thighs, and the body's own skin,
            // where it has samples, against the thing (a thing asked to be carried in the belly is carried against it)
            let body = skel.fk(&base(t));
            let (ahead, solids) = (yaw(&body) * Vec3::Z, own_solids(skel, &body));
            // (its skin as the body will be for a thing this low: squatting, its knees sat back from the thing)
            let mut around = self.avoid.clone();
            around.push(thing);
            let low = skel.fk(&stoop(skel, &base(t), &body, ahead, thing.at.y, 1.0, 0.0, spread_for(skel, &body, &thing), &around));
            let skin: Vec<Vec3> = skel.skin(&low).into_iter().zip(&skel.surface).filter(|(_, x)| !of_the_arm(&x.part)).map(|(p, _)| p).collect();
            let stood = thing.at;
            for _ in 0..10 {
                // (no further than 25 cm: past that the arms couldn't hold it)
                if (thing.at - stood).length() > 0.25 {
                    break;
                }
                let wrists = self.grip.holds().map(|h| thing.world(h.at) - ahead * 0.08);
                let boxed = thing.corners().into_iter().chain(wrists).flat_map(|c| solids.iter().map(move |s| 0.01 - s.distance(c))).fold(0.0f32, f32::max);
                let inside = skin.iter().map(|p| 0.004 - thing.distance(*p)).fold(boxed, f32::max);
                if inside <= 0.0 {
                    break;
                }
                thing.at += ahead * (inside + 0.005) * u;
            }
            thing
        };
        match self.release {
            Some(Release::Put { at, down, place }) if t >= at => {
                let from = carried(at);
                let u = smooth(span(t, at, down));
                // set down level with where it stood: its own turn about the vertical kept
                let arc = Vec3::Y * 0.04 * (std::f32::consts::PI * span(t, at, down)).sin();
                let thing = from.moved(from.at.lerp(place, u) + arc, from.turn);
                (thing, if t < down { Phase::Release } else { Phase::After })
            }
            Some(Release::Throw { at, velocity, floor }) if t >= at => {
                let from = carried(at);
                let rest = floor + from.solid.bound();
                // the fall, stopped where it lands
                let fall = |dt: f32| from.at + velocity * dt - Vec3::Y * (0.5 * GRAVITY * dt * dt);
                let land = (velocity.y + (velocity.y * velocity.y + 2.0 * GRAVITY * (from.at.y - rest).max(0.0)).sqrt()) / GRAVITY;
                (from.moved(fall((t - at).min(land)), from.turn), Phase::After)
            }
            Some(Release::Leave { at }) if t >= at => (carried(at), Phase::After),
            _ => (carried(t), if t < self.carried { Phase::Lift } else { Phase::Carry }),
        }
    }

    /// when the hands let go, if they do
    fn let_go(&self) -> Option<f32> {
        match self.release? {
            Release::Put { down, .. } => Some(down),
            Release::Throw { at, .. } | Release::Leave { at } => Some(at),
        }
    }
}

/// The body and the thing at `t`. `base` is the body's own motion: its pose at any time.
pub fn perform(skel: &Skeleton, base: &dyn Fn(f32) -> Pose, take: &Take, t: f32) -> Frame {
    let own = base(t);
    let (mut thing, phase) = take.thing_at(skel, base, t);
    // a thing carried in one hand turns about the vertical with the forearm (swivel), as it was when let go
    if let (Grip::One(_), true, None, Some(part)) = (take.grip, take.slide.is_empty(), take.pass, take.parts().first()) {
        let till = part.gone.map_or(t, |g| t.min(g));
        let then = if till < t { take.thing_at(skel, base, till).0 } else { thing };
        thing.turn = (Quat::from_rotation_y(swivel(skel, base, take, part, till, &then)) * thing.turn).normalize();
    }
    let mut frame = Frame { pose: own.clone(), thing, phase, miss: 0.0, moved: 0.0, touch: [None, None], limited: false };
    // each hand's part: how far along its reach it is (`into`), and how far gone once it has let go (`away`)
    let parts: Vec<(Part, f32, f32)> = take
        .parts()
        .into_iter()
        .map(|p| (p, smooth(span(t, p.start, p.grab)), p.gone.map_or(0.0, |g| smooth(span(t, g, g + AWAY)))))
        .filter(|(p, _, away)| t >= p.start && *away < 1.0)
        .collect();
    if parts.is_empty() {
        return frame;
    }
    // how much of the body is the take's: the most any hand is on
    let busy = parts.iter().map(|(_, into, away)| into * (1.0 - away)).fold(0.0, f32::max);
    let on = parts.iter().map(|(_, _, away)| 1.0 - away).fold(0.0, f32::max);
    let w0 = skel.fk(&own);
    let face = yaw(&w0) * Vec3::Z;
    // a hand that has let go eases off the place the thing was let go at, not after a thing in flight; and a throw's
    // hands follow the thing out a little
    let follow = match take.release {
        Some(Release::Throw { velocity, .. }) => velocity.normalize_or_zero() * 0.2 * (velocity.length() / 3.0).min(1.0),
        _ => Vec3::ZERO,
    };
    // each hand's lie on its hold is the one it has at its grab (lie_at_grab), kept in the thing's own frame: a hand
    // doesn't shift on what it holds
    let holds: Vec<_> = parts
        .iter()
        .filter_map(|&(part, into, away)| {
            let held = match part.gone {
                Some(g) if t >= g => take.thing_at(skel, base, g).0.moved(take.thing_at(skel, base, g).0.at, thing.turn),
                _ => thing,
            };
            let (at, turn) = lie_at_grab(skel, base, take, &part)?;
            Some((part, into, away, held, (held.world(at), (held.turn * turn).normalize())))
        })
        .collect();
    // a thing lower than the arms reach: the body bends and squats for it, as far as the hands have got
    let low = holds.iter().map(|h| h.4.0.y).fold(f32::MAX, f32::min);
    // a low thing close in front comes up between the knees: they open for it
    let spread = spread_for(skel, &w0, &thing) * busy;
    // what the knees keep out of: the scene's solids, and the thing itself while it stands (a squat sits back from a
    // box on the floor instead of putting its shins through it)
    let mut around = take.avoid.clone();
    if matches!(phase, Phase::Reach | Phase::Lift) {
        around.push(thing);
    }
    // solved with the bend the body has room for: where bending or leaning would put the head or chest into a
    // solid, less of both and more squat, a step at a time
    // (`lower`: how much further down the body goes than the hands' height alone asks; `short`, how far the worst
    // wrist ended from where it was sent)
    let attempt = |give: f32, lower: f32, frame: &mut Frame| {
    let mut pose = stoop(skel, &own, &w0, face, low - lower, busy, give, spread, &around);
    let mut short = 0.0f32;
    frame.miss = 0.0;
    // a heavy thing: the trunk leans back against it once it is off what it stood on
    if let (true, Some(waist)) = (take.weight > 0.0, skel.joint("spine_01")) {
        let back = (0.012 * take.weight).min(0.2) * smooth(span(t, take.grab, take.carried)) * on;
        pose.turns[waist] = (pose.turns[waist] * Quat::from_rotation_x(-back)).normalize();
    }
    // (each hand, then every hand but the last again with the trunk left where it is: a reach leans the chest, and
    // the second hand's lean carried the first 6 cm off its hold, its fingers still shaped for the hold)
    for (n, &(part, into, away, held, level)) in holds.iter().chain(&holds[..holds.len().saturating_sub(1)]).enumerate() {
        let (hold, again) = (part.hold, n >= holds.len());
        let (Some(arm), Some(rest)) = (skel.arm(hold.side), hand_rest(skel, hold.side)) else { continue };
        let normal = held.turn * hold.normal;
        let (at, turn) = level;
        // the wrist's path: from where the body has it, in onto the hold from off the surface and over what is in the
        // way; and when letting go, back off the surface and out to where the body has it
        let (u, off) = if away > 0.0 { (1.0 - away, (normal * 0.08 + follow) * smooth(away * 4.0)) } else { (into, normal * 0.1 * (1.0 - into)) };
        let arc = Vec3::Y * 0.06 * (std::f32::consts::PI * u).sin();
        let end = w0.rot[arm[2]].slerp(turn, smooth(span(u, 0.0, 0.8)));
        // the hand opens as it comes in, as a hand does before it takes something (the prior's open hand), then
        // the fingers shut over the last moments of the reach, and open again as the hand leaves
        let mut open = pose.turns.clone();
        if !again && shape(skel, &mut open, hold.side, &[OPEN]).is_some() {
            let wide = smooth(span(t, part.start, part.grab - SHUT));
            for (mine, open) in pose.turns.iter_mut().zip(&open) {
                *mine = mine.slerp(*open, wide);
            }
        }
        let close = if again { 0.0 } else { smooth(span(t, part.grab - SHUT, part.grab)) * (1.0 - smooth(span(away, 0.0, 0.5))) };
        match hold.shape {
            // a way of holding brings its hand's shape
            // for the fingers that don't meet the thing; the ones that do are curled onto it as on any hold, so none
            // sinks in
            Some(z) => {
                let (mut fitted, mut shaped) = (pose.turns.clone(), pose.turns.clone());
                let touch = curl(skel, &mut fitted, (at, turn), hold.side, &held, 1.0);
                if let (Some(touch), Some(())) = (touch, shape(skel, &mut shaped, hold.side, &z)) {
                    for (n, name) in raea_sys_body::rig::FINGERS.iter().enumerate() {
                        for j in skel.finger(name, hold.side).into_iter().flatten().filter(|_| !touch.on[n]) {
                            fitted[j] = shaped[j];
                        }
                    }
                }
                frame.touch[hold.side as usize] = touch;
                for (mine, fitted) in pose.turns.iter_mut().zip(&fitted) {
                    *mine = mine.slerp(*fitted, close);
                }
            }
            None if close > 0.0 => frame.touch[hold.side as usize] = curl(skel, &mut pose.turns, (at, turn), hold.side, &held, close),
            None => {}
        }
        // (the hand's skin, its fingers as they are this frame, keeps 2 cm off solids while it travels; on its hold,
        // 4 mm. Read with the fingers at rest, a hand opened for a box on a table put its little finger 34 mm in.)
        let mut skin = hand_points(skel, hold.side, &rest, Some(&skel.fk(&pose)));
        skin.1 = 0.004 + (skin.1 - 0.004).max(0.0) * (1.0 - u);
        let target = clear(w0.pos[arm[2]].lerp(at + off, u) + arc, end, &skin, &take.avoid);
        // elbows out and down, the way arms hang; where that puts the shoulder or the elbow past its range, the
        // elbow is swung about the line from shoulder to hand, 10 degrees at a time, to the nearest swing that
        // doesn't (the wrist is then held to its own range, and a frame that had to says so)
        let pole = yaw(&w0) * Vec3::new(0.7 * hold.side.sign(), -1.0, -0.4);
        let line = (target - skel.fk(&pose).pos[arm[0]]).normalize_or(face);
        // each swing's cost: how far past their ranges the shoulder and elbow are (degrees), and how deep the elbow
        // and forearm are in the scene's solids and the body's own trunk (a degree a millimetre); the swing taken is
        // the mean of them all, weighted steeply toward the cheapest and gently toward none, so it moves smoothly
        // from frame to frame where a first-that-passes choice jumped
        let solve_at = |swing: f32| {
            let swung = Quat::from_axis_angle(line, swing.to_radians()) * pole;
            let solved = reach(skel, &pose, hold.side, target, ArmOptions { pole: Some(swung), end: Some(end), soft: 0.02, stiff: if again { 1.0 } else { give } })?;
            Some(forearm_twist(skel, &solved.pose, hold.side, 1.0))
        };
        let (mut sum, mut weights, mut least) = (0.0f32, 0.0f32, f32::MAX);
        let mut costs = Vec::with_capacity(13);
        for step in -6..=6 {
            let swing = step as f32 * 15.0;
            let Some(tried) = solve_at(swing) else { continue };
            // (the wrist's too: an elbow lifted out to the side can spare a wrist a bend it can't make)
            let mut cost = past_range(skel, arm, &tried) + clamp_turn(skel, arm[2], tried.turns[arm[2]]).1;
            let drawn = skel.fk(&tried);
            let (elbow, wrist) = (drawn.pos[arm[1]], drawn.pos[arm[2]]);
            let depth = |solid: &Thing, margin: f32| (margin - solid.distance(elbow)).max(margin - 0.01 - solid.distance(elbow.lerp(wrist, 0.5))).max(0.0);
            cost += take.avoid.iter().map(|s| depth(s, 0.04)).sum::<f32>() * 1000.0;
            cost += torso(skel, &drawn).map_or(0.0, |body| depth(&body, 0.03)) * 1000.0;
            least = least.min(cost);
            costs.push((swing, cost));
        }
        for (swing, cost) in costs {
            let w = (-(cost - least) / 12.0).exp() * (-(swing / 60.0).powi(2)).exp();
            sum += swing * w;
            weights += w;
        }
        let Some(solved) = solve_at(if weights > 0.0 { sum / weights } else { 0.0 }) else { continue };
        let past = past_range(skel, arm, &solved);
        pose = solved;
        short = short.max((skel.fk(&pose).pos[arm[2]] - target).length());
        let (wrist, by) = clamp_turn(skel, arm[2], pose.turns[arm[2]]);
        pose.turns[arm[2]] = wrist;
        frame.limited |= past >= 0.5 || by > 0.5;
        if t >= part.grab && away <= 0.0 {
            frame.miss = frame.miss.max((skel.fk(&pose).pos[arm[2]] - at).length());
        }
    }
    // the eyes go to the thing as the hands do, and mostly leave it once it is carried
    let watching = busy * (1.0 - 0.7 * smooth(span(t, take.grab, take.carried)));
    (look(skel, &pose, thing.at, watching * (1.0 - 0.5 * give)), short)
    };
    // the body follows the hands: where the arms fall short of where they are sent (a thing low and out in front),
    // the body goes down by that much more, twice over, the same every frame so it stays smooth
    let solve = |give: f32, frame: &mut Frame| {
        let first = attempt(give, 0.0, frame).1 * 1.3;
        let second = attempt(give, first, frame).1 * 1.3;
        attempt(give, first + second, frame).0
    };
    // bent as far as the head and chest have room for: the nearer a solid they would come bending freely, the more
    // of the bend is given up for squat
    let mut pose = solve(0.0, &mut frame);
    let give = give_for(room(skel, &pose, &take.avoid));
    if give > 0.0 {
        pose = solve(give, &mut frame);
    }
    // a thing held in one hand is where that hand has it: if the wrist was held at its limit, the thing turns with
    // the hand it is in (a cup tips less than asked) and nothing of the hand passes into it
    if let ([(part, _, away, _, (at, turn))], true) = (&holds[..], take.slide.is_empty()) {
        if let (true, Some(arm)) = (t >= part.grab && *away <= 0.0, skel.arm(part.hold.side)) {
            let drawn = skel.fk(&pose);
            let off = (drawn.rot[arm[2]] * turn.inverse()).normalize();
            frame.thing = thing.moved(drawn.pos[arm[2]] + off * (thing.at - *at), (off * thing.turn).normalize());
        }
    }
    // the first and the last of a hand's part are a blend of the joints, so its arm leaves and ends exactly as the
    // body has it: each hand's own arm by its own part, the rest of the body by the hand most on
    let mut most = 0.0f32;
    // a joint of a side's arm or hand (its legs aren't)
    let of_arm = |name: &str, side: &str| name.ends_with(side) && !["thigh", "calf", "foot", "ball"].iter().any(|leg| name.starts_with(leg));
    let sides: Vec<String> = holds.iter().map(|h| format!("_{}", h.0.hold.side.sx())).collect();
    for (&(part, _, away, ..), side) in holds.iter().zip(&sides) {
        let w = smooth(span(t, part.start, part.start + 0.15)) * (1.0 - smooth(span(away, 0.45, 1.0)));
        most = most.max(w);
        for (i, joint) in skel.joints.iter().enumerate() {
            if of_arm(&raea_sys_body::rig::role_name(&joint.name), side) {
                pose.turns[i] = own.turns[i].slerp(pose.turns[i], w);
            }
        }
    }
    pose.root = own.root.lerp(pose.root, most);
    for (i, joint) in skel.joints.iter().enumerate() {
        if !sides.iter().any(|side| of_arm(&raea_sys_body::rig::role_name(&joint.name), side)) {
            pose.turns[i] = own.turns[i].slerp(pose.turns[i], most);
        }
    }
    frame.moved = pose.turns.iter().zip(&own.turns).map(|(a, b)| a.angle_between(*b).to_degrees()).fold(0.0, f32::max);
    frame.pose = pose;
    frame
}
