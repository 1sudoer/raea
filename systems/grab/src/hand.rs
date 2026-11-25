//! Where hands go on a thing, and how they lie there. Hold points are found the way musthafa45/Ik-Object-Holding's
//! ColliderEdgeDetector finds them (MIT): from the thing's centre, out to the holder's left and right, a ray cast
//! back in meets the surface, and that is where each hand goes. Here the points are kept in the thing's own frame, so
//! they ride with it, and each carries the way the surface faces, which sets the palm. A thing narrower than a hand's
//! span is taken with one hand.

use glam::{Mat3, Quat, Vec3};

use raea_sys_body::rig::{Side, Skeleton, hand_rest};
use raea_sys_body::shape::{Solid, Thing};

/// where one hand holds a thing, in the thing's own frame
#[derive(Clone, Copy, Debug)]
pub struct Hold {
    pub side: Side,
    pub at: Vec3,
    /// the way the surface faces there, outward
    pub normal: Vec3,
    /// the way the fingers point, for a thing held one way (a handle, a grip); found from the arm when none
    pub along: Option<Vec3>,
    /// the hand's shape, as the hand prior's first numbers, for a hold that is a way of holding (a pinch, a fist
    /// round a handle); the fingers curl onto the surface when none
    pub shape: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug)]
pub enum Grip {
    One(Hold),
    Two(Hold, Hold),
}

impl Hold {
    /// the same hold for the other hand: across the thing, on its far side
    pub fn across(&self) -> Hold {
        let flip = |v: Vec3| v - self.normal * (2.0 * v.dot(self.normal));
        Hold { side: if self.side == Side::L { Side::R } else { Side::L }, at: flip(self.at), normal: -self.normal, along: self.along.map(flip), shape: self.shape }
    }
}

impl Grip {
    pub fn holds(&self) -> impl Iterator<Item = Hold> {
        let (a, b) = match *self {
            Grip::One(h) => (Some(h), None),
            Grip::Two(l, r) => (Some(l), Some(r)),
        };
        a.into_iter().chain(b)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HoldOptions {
    /// the hand that takes a thing small enough for one
    pub hand: Side,
    /// the widest thing one hand takes, metres across
    pub span: f32,
    /// how far below the thing's middle the hands go, metres (a heavy box is held low)
    pub lower: f32,
}

impl Default for HoldOptions {
    fn default() -> Self {
        HoldOptions { hand: Side::R, span: 0.11, lower: 0.0 }
    }
}

/// the level way from a holder to a thing (the way the holder faces to take it)
pub fn facing(thing: &Thing, holder: Vec3) -> Vec3 {
    let f = thing.at - holder;
    Vec3::new(f.x, 0.0, f.z).try_normalize().unwrap_or(Vec3::Z)
}

/// The holds a holder standing at `holder` takes on a thing: one on its left and one on its right, or one hand's.
pub fn find_holds(thing: &Thing, holder: Vec3, o: HoldOptions) -> Grip {
    // a rod lying down (a capsule or cylinder longer than it is wide, its length near level, thin enough for one
    // hand) is taken over the top, the fingers round it across its length: not by its ends
    if let Solid::Capsule { half, r } | Solid::Cylinder { half, r } = thing.solid {
        let length = thing.turn * Vec3::Y;
        if half > r * 1.5 && 2.0 * r <= o.span && length.y.abs() < 0.5 {
            let across = Vec3::Y.cross(length).normalize_or(Vec3::X);
            // the fingers point away from the holder, across the rod
            let away = if across.dot(facing(thing, holder)) >= 0.0 { across } else { -across };
            let top = thing.cast(thing.at + Vec3::Y * (thing.solid.bound() + 0.1), Vec3::NEG_Y, thing.solid.bound() + 0.1).map_or(thing.at + Vec3::Y * r, |h| h.at);
            let turn = thing.turn.inverse();
            return Grip::One(Hold { side: o.hand, at: thing.local(top), normal: turn * Vec3::Y, along: Some(turn * away), shape: None });
        }
    }
    // the body's left is its up crossed with its facing (y up, facing +z, x left)
    let left = Vec3::Y.cross(facing(thing, holder));
    let far = thing.solid.bound() + 0.1;
    let hold = |side: Side| {
        let out = left * side.sign();
        let from = thing.at + out * far - Vec3::Y * o.lower;
        // no surface on that line (the lowered ray passes under a ball): the thing's edge that way, as the original does
        let (at, normal) = thing.cast(from, -out, far).map_or((thing.at + out * thing.solid.bound(), out), |h| (h.at, h.normal));
        Hold { side, at: thing.local(at), normal: thing.turn.inverse() * normal, along: None, shape: None }
    };
    let (l, r) = (hold(Side::L), hold(Side::R));
    if (l.at - r.at).length() <= o.span { Grip::One(if o.hand == Side::L { l } else { r }) } else { Grip::Two(l, r) }
}

/// The wrist's place and the hand's turn that lay the palm on a hold: the palm against the surface, the fingers
/// pointing the way the holder faces, as near as the surface lets them.
pub fn wrist_for(skel: &Skeleton, thing: &Thing, hold: &Hold, facing: Vec3) -> Option<(Vec3, Quat)> {
    let rest = hand_rest(skel, hold.side)?;
    let (at, n) = (thing.world(hold.at), (thing.turn * hold.normal).normalize());
    // the fingers point the way asked, as near as the surface lets them. An asking nearly straight into the surface
    // says little, and less the straighter it is: there the fingers turn toward upright (or, on a level surface,
    // toward the holder's left), by as much as the asking falls short, so the way they point never flips
    let (asked, up) = (facing.reject_from_normalized(n), Vec3::Y.reject_from_normalized(n));
    let spare = if up.length() > 0.3 { up.normalize() } else { Vec3::X.reject_from_normalized(n).normalize_or(Vec3::X) };
    let along = (asked + spare * (1.0 - asked.length()).max(0.0) * 0.6).normalize_or(spare);
    let frame = |a: Vec3, p: Vec3| Mat3::from_cols(a, p, a.cross(p));
    let turn = Quat::from_mat3(&(frame(along, -n) * frame(rest.along, rest.palm).transpose())).normalize();
    Some((at - turn * rest.mid, turn))
}

/// What the fingers of one hand found on a thing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Touch {
    /// thumb, index, middle, ring, pinky: on the thing
    pub on: [bool; 5],
    /// how deep the worst finger is inside it at the pose it came with, metres (0: none is)
    pub inside: f32,
}

/// each joint's most bend from rest, degrees: fingers, then the thumb
const BEND: [[f32; 3]; 2] = [[80.0, 100.0, 70.0], [45.0, 55.0, 70.0]];
/// and how far each opens back past rest (the rest hand is a little curled, so a finger laid on a flat face opens)
const OPEN: [[f32; 3]; 2] = [[-40.0, -25.0, -20.0], [-30.0, -20.0, -20.0]];
/// how far a finger that meets nothing still closes, as a share of its most: a loose hand, not a fist
const LOOSE: f32 = 0.35;

/// The fingers of a hand curled onto a thing: each joint, knuckle first, bends toward the palm until its bone meets
/// the surface, so the finger wraps whatever shape is there. The hand is taken to be at `wrist` (its place and turn:
/// where its hold has it), not where the arm has got it to, so the shape the fingers close to is the hold's and
/// doesn't change as the hand comes in. `close` (0 to 1) is how far along that closing is, for the frames a hand
/// takes to shut. The pose's own finger turns are what it closes from.
pub fn curl(skel: &Skeleton, turns: &mut [Quat], wrist: (Vec3, Quat), s: Side, thing: &Thing, close: f32) -> Option<Touch> {
    let hand = skel.arm(s)?[2];
    let rest = hand_rest(skel, s)?;
    let mut touch = Touch::default();
    for (n, name) in raea_sys_body::rig::FINGERS.iter().enumerate() {
        let js = skel.finger(name, s)?;
        let (thumb, thick) = (n == 0, if n == 0 { 0.010 } else { 0.008 });
        // the axis each joint bends about, at rest: the one that carries its bone toward the palm's side (the thumb's,
        // toward the space in front of the palm, where it meets the fingers)
        let axis = |k: usize| {
            let j = &skel.joints[js[k]];
            let bone = (j.tail - j.head).normalize();
            let toward = if thumb { (skel.joints[hand].head + rest.mid + rest.palm * 0.05 - j.head).normalize() } else { rest.palm };
            bone.cross(toward).normalize_or(Vec3::X)
        };
        let own = [turns[js[0]], turns[js[1]], turns[js[2]]];
        let mut base = own;
        // each bone's own skin, where the body has samples of it
        let skin = js.map(|j| skel.skin_of(j).collect::<Vec<_>>());
        // the least clearance of bone k's skin with the finger bent by `bend` (radians a joint), each sample where its
        // joints carry it by their weights, as it is drawn (the ball of the thumb is half the hand's: read as riding
        // the thumb's bone alone it stood clear of a box it was 9 mm inside). A joint not this finger's rides the hand.
        let clear = |base: &[Quat; 3], bend: [f32; 3], k: usize| {
            let mut at = [wrist; 3];
            for i in 0..3 {
                let (j, (pp, pr)) = (&skel.joints[js[i]], if i == 0 { wrist } else { at[i - 1] });
                // (the knuckle from the hand's joint, whatever lies between: SOMA has a bone in the palm there)
                let p = pp + pr * if i == 0 { j.head - skel.joints[hand].head } else { skel.offset(js[i]) };
                at[i] = (p, (pr * base[i] * Quat::from_axis_angle(axis(i), bend[i])).normalize());
            }
            let place = |j: usize| js.iter().position(|&c| c == j).map_or((wrist.0 + wrist.1 * (skel.joints[j].head - skel.joints[hand].head), wrist.1), |i| at[i]);
            let j = &skel.joints[js[k]];
            let bare = [0.45, 0.75, 1.0].map(|along| thing.distance(at[k].0 + at[k].1 * ((j.tail - j.head) * along)) - thick);
            let own = skin[k].iter().map(|x| thing.distance(skel.carried(x, place)));
            if skin[k].is_empty() { bare.into_iter().fold(f32::MAX, f32::min) } else { own.fold(f32::MAX, f32::min) }
        };
        // each joint starts opened and bends until its bone, or any bone past it, meets the surface (the last step
        // halved down to the contact), so the finger wraps whatever is there; the joints after it wait opened. (On
        // its own bone alone, a knuckle bent 60 degrees onto a box's face carried the next two bones 35 mm in.)
        let free = |base: &[Quat; 3], bend: [f32; 3], k: usize| (k..3).map(|i| clear(base, bend, i)).fold(f32::MAX, f32::min);
        let open = OPEN[thumb as usize].map(f32::to_radians);
        // a thumb that is in the thing even opened is spread out from the palm (turned about the hand's length, the
        // less of the two ways, up to 60 degrees) until it is clear: round a cup, not through it
        if thumb {
            let worst = |b: &[Quat; 3]| (0..3).map(|k| clear(b, open, k)).fold(f32::MAX, f32::min);
            if worst(&base) < 0.0 {
                let spread = |by: f32| {
                    let mut b = own;
                    b[0] = (own[0] * Quat::from_axis_angle(rest.along, by.to_radians())).normalize();
                    b
                };
                if let Some(by) = (1..=12).flat_map(|a| [a as f32 * 5.0, a as f32 * -5.0]).find(|by| worst(&spread(*by)) >= 0.0) {
                    base = spread(by);
                }
            }
        }
        let mut bend = open;
        for k in 0..3 {
            let most = BEND[thumb as usize][k].to_radians();
            let start = free(&base, bend, k);
            if start < 0.0 {
                // in the thing even opened: nothing a curl can fix; said, not hidden
                touch.inside = touch.inside.max(-start);
                touch.on[n] = true;
                continue;
            }
            let step = 3.0f32.to_radians();
            let (mut lo, mut hit) = (open[k], None);
            // the bend at which the bone comes nearest, for a finger that never meets the thing
            let mut nearest = (start, open[k]);
            while lo < most {
                let hi = (lo + step).min(most);
                bend[k] = hi;
                let gap = free(&base, bend, k);
                if gap < 0.0 {
                    hit = Some(hi);
                    break;
                }
                if gap < nearest.0 {
                    nearest = (gap, hi);
                }
                lo = hi;
            }
            match hit {
                Some(mut hi) => {
                    for _ in 0..8 {
                        let mid = (lo + hi) / 2.0;
                        bend[k] = mid;
                        if free(&base, bend, k) < 0.0 { hi = mid } else { lo = mid }
                    }
                    bend[k] = lo;
                    touch.on[n] = true;
                }
                // a finger that never meets it: where it comes nearest when that is near (within 3 cm), easing to a
                // loose curl the farther off it stays, so a finger at the thing's edge doesn't snap between the two
                None => {
                    let near = 1.0 - (nearest.0 / 0.03).clamp(0.0, 1.0);
                    bend[k] = most * LOOSE + (nearest.1 - most * LOOSE) * near * near * (3.0 - 2.0 * near);
                }
            }
        }
        for k in 0..3 {
            // (from the pose's own turn, so a spread thumb eases out as the hand shuts)
            let shut = (base[k] * Quat::from_axis_angle(axis(k), bend[k])).normalize();
            turns[js[k]] = own[k].slerp(shut, close.clamp(0.0, 1.0));
        }
    }
    Some(touch)
}
