//! The rig, as sdk/body/src/rig.ts has it: MakeHuman's game_engine skeleton (docs/skeleton.feature), poses and
//! forward kinematics. The body's frame: y up, facing +z, x its left. Every joint's rest turn is none, so a joint's
//! turn is read in the body's axes as they are at rest.

use std::collections::HashMap;

use glam::{Quat, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    L,
    R,
}

impl Side {
    pub const BOTH: [Side; 2] = [Side::L, Side::R];
    pub fn sx(self) -> &'static str {
        if self == Side::L { "l" } else { "r" }
    }
    /// +1 on the body's left (+x), -1 on its right
    pub fn sign(self) -> f32 {
        if self == Side::L { 1.0 } else { -1.0 }
    }
}

pub const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    /// the parent's index; None for the root (the pelvis). Parents come before their children
    pub parent: Option<usize>,
    /// where the joint and its bone's end are at rest, metres, the body's frame
    pub head: Vec3,
    pub tail: Vec3,
}

/// A point of the skin: the part of the body it is on ("palm", "index3", "thigh"...), the joints that carry it with
/// their weights, where it is at rest, and the way the skin faces there.
#[derive(Clone, Debug)]
pub struct Sample {
    pub part: String,
    pub joints: Vec<(usize, f32)>,
    pub at: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    /// samples of the body's skin at rest (skeleton.json's `surface`; none on the stand-in body)
    pub surface: Vec<Sample>,
    index: HashMap<String, usize>,
}

/// A pose: the root's place, and each joint's turn from rest against its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub root: Vec3,
    pub turns: Vec<Quat>,
}

/// A pose drawn: each joint's place and turn in the body's frame.
#[derive(Clone, Debug)]
pub struct Posed {
    pub pos: Vec<Vec3>,
    pub rot: Vec<Quat>,
}

impl Skeleton {
    pub fn new(joints: Vec<Joint>) -> Self {
        let index = joints.iter().enumerate().map(|(i, j)| (j.name.clone(), i)).collect();
        Skeleton { joints, surface: Vec::new(), index }
    }

    /// A character's skeleton.json (projects/psx/export.py): `joints` of name, parent (-1 for the root), head, tail.
    pub fn from_json(text: &str) -> Result<Self, String> {
        Self::from_value(&serde_json::from_str(text).map_err(|e| format!("skeleton.json: {e}"))?)
    }

    pub fn from_value(v: &serde_json::Value) -> Result<Self, String> {
        let list = v["joints"].as_array().ok_or("skeleton.json: no joints")?;
        let vec3 = |a: &serde_json::Value| -> Option<Vec3> {
            let a = a.as_array()?;
            Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
        };
        let mut joints = Vec::with_capacity(list.len());
        for (i, j) in list.iter().enumerate() {
            let name = j["name"].as_str().ok_or_else(|| format!("skeleton.json: joint {i} has no name"))?;
            let parent = j["parent"].as_i64().ok_or_else(|| format!("skeleton.json: {name} has no parent"))?;
            if parent >= i as i64 {
                return Err(format!("skeleton.json: {name}'s parent comes after it"));
            }
            let head = vec3(&j["head"]).ok_or_else(|| format!("skeleton.json: {name} has no head"))?;
            let tail = vec3(&j["tail"]).unwrap_or(head);
            joints.push(Joint { name: name.to_string(), parent: usize::try_from(parent).ok(), head, tail });
        }
        let mut skel = Skeleton::new(joints);
        // the skin's samples: each an offset from its first joint's head, carried by up to four joints
        for x in v["surface"].as_array().into_iter().flatten() {
            let js: Vec<usize> = x["j"].as_array().into_iter().flatten().filter_map(|j| j.as_u64().map(|j| j as usize)).collect();
            let ws = x["w"].as_array().into_iter().flatten().filter_map(|w| w.as_f64().map(|w| w as f32));
            let (Some(&first), Some(o)) = (js.first(), vec3(&x["o"])) else { continue };
            if js.iter().any(|&j| j >= skel.joints.len()) {
                continue;
            }
            skel.surface.push(Sample { part: x["part"].as_str().unwrap_or("").to_string(), joints: js.iter().copied().zip(ws).collect(), at: skel.joints[first].head + o, normal: vec3(&x["n"]).unwrap_or(Vec3::Y) });
        }
        Ok(skel)
    }

    /// A joint by its name, or by its role: the roles are spelt as the game_engine rig named its joints ("thigh_l",
    /// "spine_03", "index_02_r"), and on a SOMA body a role finds SOMA's joint for it (docs/character.feature CH1).
    /// A SOMA name finds the joint on a body named the old way too, as the TypeScript SDK now names joints by SOMA.
    pub fn joint(&self, name: &str) -> Option<usize> {
        self.index
            .get(name)
            .copied()
            .or_else(|| self.index.get(soma_name(name)?.as_str()).copied())
            .or_else(|| self.index.get(rename(name, false)?.as_str()).copied())
    }

    fn three(&self, names: [String; 3]) -> Option<[usize; 3]> {
        Some([self.joint(&names[0])?, self.joint(&names[1])?, self.joint(&names[2])?])
    }

    /// shoulder, elbow, wrist
    pub fn arm(&self, s: Side) -> Option<[usize; 3]> {
        self.three(["upperarm", "lowerarm", "hand"].map(|n| format!("{n}_{}", s.sx())))
    }

    /// a finger's three joints, knuckle first (the thumb's first is its metacarpal)
    pub fn finger(&self, f: &str, s: Side) -> Option<[usize; 3]> {
        self.three([1, 2, 3].map(|k| format!("{f}_0{k}_{}", s.sx())))
    }

    pub fn rest(&self) -> Pose {
        Pose { root: self.joints.first().map_or(Vec3::ZERO, |j| j.head), turns: vec![Quat::IDENTITY; self.joints.len()] }
    }

    /// a joint's place at rest against its parent's
    pub fn offset(&self, i: usize) -> Vec3 {
        match self.joints[i].parent {
            Some(p) => self.joints[i].head - self.joints[p].head,
            None => Vec3::ZERO,
        }
    }

    /// Forward kinematics: each joint's place and turn.
    pub fn fk(&self, p: &Pose) -> Posed {
        let n = self.joints.len();
        let (mut pos, mut rot) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let t = p.turns.get(i).copied().unwrap_or(Quat::IDENTITY);
            match self.joints[i].parent {
                None => {
                    rot.push(t);
                    pos.push(p.root);
                }
                Some(par) => {
                    let (pr, pp): (Quat, Vec3) = (rot[par], pos[par]);
                    rot.push((pr * t).normalize());
                    pos.push(pp + pr * self.offset(i));
                }
            }
        }
        Posed { pos, rot }
    }

    /// the samples of the skin a joint carries first (a bone's own skin)
    pub fn skin_of(&self, joint: usize) -> impl Iterator<Item = &Sample> {
        self.surface.iter().filter(move |x| x.joints.first().is_some_and(|j| j.0 == joint))
    }

    /// Where every sample of the skin is in a pose drawn: each carried by its joints, by their weights.
    pub fn skin(&self, at: &Posed) -> Vec<Vec3> {
        self.surface.iter().map(|s| self.carried(s, |j| (at.pos[j], at.rot[j]))).collect()
    }

    /// Where one sample of the skin is, each of its joints at the place and turn `at` gives for it.
    pub fn carried(&self, s: &Sample, at: impl Fn(usize) -> (Vec3, Quat)) -> Vec3 {
        let sum: Vec3 = s.joints.iter().map(|&(j, w)| { let (p, q) = at(j); (p + q * (s.at - self.joints[j].head)) * w }).sum();
        sum / s.joints.iter().map(|j| j.1).sum::<f32>().max(1e-6)
    }

    /// the local turn that gives joint `i` the world turn `world`, against its drawn parent
    pub fn local_for(&self, w: &Posed, i: usize, world: Quat) -> Quat {
        let pr = self.joints[i].parent.map_or(Quat::IDENTITY, |p| w.rot[p]);
        (pr.inverse() * world).normalize()
    }
}

/// A hand's frame at rest, in the body's axes: along (wrist to the middle knuckle), the way the palm faces, and where
/// the palm's skin is from the wrist.
#[derive(Clone, Copy, Debug)]
pub struct HandRest {
    pub along: Vec3,
    pub palm: Vec3,
    pub mid: Vec3,
}

pub fn hand_rest(skel: &Skeleton, s: Side) -> Option<HandRest> {
    let hand = skel.arm(s)?[2];
    let knuckle = |f: &str| Some(skel.joints[skel.finger(f, s)?[0]].head);
    let h = skel.joints[hand].head;
    let reach = knuckle("middle")? - h;
    let along = reach.normalize();
    let line = knuckle("index")? - knuckle("pinky")?;
    let across = (line - along * line.dot(along)).normalize();
    let palm = if s == Side::L { along.cross(across) } else { across.cross(along) };
    // the palm's middle: a little over half way to the knuckles, out to the skin (the body's own, where it has
    // samples: the highest of the palm's skin within 2.5 cm of that middle; else 12 mm)
    let middle = reach * 0.55;
    // and the ball of the thumb, which is the palm's too though the thumb's first bone carries it: a face the palm
    // lies on meets it first (left out, it sat 6 to 9 mm inside a held box with the thumb as far open as it goes)
    let ball = skel.skin_of(skel.finger("thumb", s)?[0]).filter(|x| x.normal.dot(palm) > 0.5);
    let skin = skel
        .skin_of(hand)
        // (skin the hand mostly carries itself: the web to the thumb stands 13 mm proud of the palm and goes with the thumb)
        .filter(|x| x.part == "palm" && x.normal.dot(palm) > 0.5 && x.joints.first().is_some_and(|j| j.1 >= 0.8))
        .filter(|x| (x.at - h - middle).reject_from_normalized(palm).length() < 0.025)
        .chain(ball)
        .map(|x| x.at - h)
        .map(|o| o.dot(palm))
        .fold(f32::MIN, f32::max);
    Some(HandRest { along, palm, mid: middle + palm * if skin > 0.0 { skin } else { 0.012 } })
}

/// the trunk's joints, and a limb's without their side: each role (the game_engine rig's name) and SOMA's name
pub const TRUNK: [(&str, &str); 6] = [("pelvis", "Hips"), ("spine_01", "Spine1"), ("spine_02", "Spine2"), ("spine_03", "Chest"), ("neck_01", "Neck1"), ("head", "Head")];
pub const LIMB: [(&str, &str); 8] = [("clavicle", "Shoulder"), ("upperarm", "Arm"), ("lowerarm", "ForeArm"), ("hand", "Hand"), ("thigh", "Leg"), ("calf", "Shin"), ("foot", "Foot"), ("ball", "ToeBase")];

/// A joint's name on the other rig: a role (the game_engine rig's name: "thigh_l", "index_02_r") to SOMA's name
/// (`to_soma`), or SOMA's to the role. The trunk by name, limbs and fingers by side. A finger's 01, 02, 03 are SOMA's
/// 2, 3, 4 (its 1 is the bone in the palm, which a role has none for); the thumb's are 1, 2, 3.
pub fn rename(name: &str, to_soma: bool) -> Option<String> {
    let pick = |t: &[(&'static str, &'static str)], key: &str| t.iter().find(|p| (if to_soma { p.0 } else { p.1 }) == key).map(|p| if to_soma { p.1 } else { p.0 });
    if let Some(n) = pick(&TRUNK, name) {
        return Some(n.to_string());
    }
    let strip = |side: &str| if to_soma { name.strip_suffix(side) } else { name.strip_prefix(side) };
    let (base, left) = match if to_soma { (strip("_l"), strip("_r")) } else { (strip("Left"), strip("Right")) } {
        (Some(base), _) => (base, true),
        (_, Some(base)) => (base, false),
        _ => return None,
    };
    let sided = |n: &str| if to_soma { format!("{}{n}", if left { "Left" } else { "Right" }) } else { format!("{n}_{}", if left { "l" } else { "r" }) };
    if let Some(n) = pick(&LIMB, base) {
        return Some(sided(n));
    }
    let fingers = ["thumb", "index", "middle", "ring", "pinky"];
    if to_soma {
        let (finger, k) = base.split_once("_0")?;
        let k = k.parse::<usize>().ok()? + usize::from(finger != "thumb");
        return fingers.contains(&finger).then(|| sided(&format!("Hand{}{}{k}", finger[..1].to_uppercase(), &finger[1..])));
    }
    let finger = base.strip_prefix("Hand")?;
    let (finger, n) = finger.split_at(finger.len().checked_sub(1)?);
    let (finger, n) = (finger.to_lowercase(), n.parse::<usize>().ok()?);
    let k = if finger == "thumb" { n } else { n.checked_sub(1)? };
    (fingers.contains(&finger.as_str()) && (1..=3).contains(&k)).then(|| sided(&format!("{finger}_0{k}")))
}

/// SOMA's name for a joint named by its role
pub fn soma_name(role: &str) -> Option<String> {
    rename(role, true)
}

/// a joint's role (as `Skeleton::joint` takes one) from its name on either rig
pub fn role_name(name: &str) -> String {
    crate::soma::raea_joint(name).unwrap_or_else(|| name.to_string())
}

/// A stand-in body of the rig's joints at an adult's proportions (1.81 m), so the solvers run with no character on
/// disk: tests, examples, and a scene that hasn't loaded its body yet. A real character brings its own skeleton.json.
pub fn mannequin() -> Skeleton {
    // name, parent, head, tail; the left side only (x > 0), the right is its mirror
    const MID: [(&str, &str, [f32; 3], [f32; 3]); 6] = [
        ("pelvis", "", [0.0, 1.00, 0.01], [0.0, 1.09, -0.03]),
        ("spine_01", "pelvis", [0.0, 1.09, -0.03], [0.0, 1.16, -0.02]),
        ("spine_02", "spine_01", [0.0, 1.16, -0.02], [0.0, 1.22, -0.04]),
        ("spine_03", "spine_02", [0.0, 1.22, -0.04], [0.0, 1.36, -0.06]),
        ("neck_01", "spine_03", [0.0, 1.57, 0.01], [0.0, 1.66, 0.04]),
        ("head", "neck_01", [0.0, 1.66, 0.04], [0.0, 1.82, 0.04]),
    ];
    const LIMB: [(&str, &str, [f32; 3], [f32; 3]); 8] = [
        ("clavicle", "spine_03", [0.024, 1.490, 0.030], [0.190, 1.463, 0.013]),
        ("upperarm", "clavicle", [0.190, 1.463, 0.013], [0.401, 1.235, 0.022]),
        ("lowerarm", "upperarm", [0.401, 1.235, 0.022], [0.534, 1.109, 0.213]),
        ("hand", "lowerarm", [0.534, 1.109, 0.213], [0.545, 1.093, 0.241]),
        ("thigh", "pelvis", [0.120, 0.975, -0.004], [0.160, 0.579, 0.030]),
        ("calf", "thigh", [0.160, 0.579, 0.030], [0.192, 0.093, 0.004]),
        ("foot", "calf", [0.192, 0.093, 0.004], [0.196, 0.029, 0.138]),
        ("ball", "foot", [0.196, 0.029, 0.138], [0.192, 0.034, 0.211]),
    ];
    // each finger's knuckle, middle joint, end joint and tip
    const HAND: [(&str, [[f32; 3]; 4]); 5] = [
        ("thumb", [[0.535, 1.102, 0.259], [0.513, 1.094, 0.281], [0.501, 1.081, 0.314], [0.495, 1.064, 0.336]]),
        ("index", [[0.563, 1.070, 0.313], [0.566, 1.055, 0.335], [0.566, 1.039, 0.352], [0.562, 1.019, 0.371]]),
        ("middle", [[0.580, 1.058, 0.295], [0.587, 1.032, 0.317], [0.590, 1.010, 0.335], [0.590, 0.988, 0.352]]),
        ("ring", [[0.588, 1.049, 0.277], [0.598, 1.025, 0.293], [0.602, 1.004, 0.307], [0.604, 0.979, 0.319]]),
        ("pinky", [[0.589, 1.040, 0.257], [0.596, 1.020, 0.266], [0.597, 1.006, 0.272], [0.600, 0.987, 0.280]]),
    ];
    let mut out: Vec<(String, String, Vec3, Vec3)> = Vec::new();
    for (n, p, h, t) in MID {
        out.push((n.into(), p.into(), h.into(), t.into()));
    }
    let on = |v: [f32; 3], s: Side| Vec3::new(v[0] * s.sign(), v[1], v[2]);
    let sided = |n: &str, s: Side| if MID.iter().any(|m| m.0 == n) { n.to_string() } else { format!("{n}_{}", s.sx()) };
    for group in [&LIMB[..4], &LIMB[4..]] {
        for s in Side::BOTH {
            for (n, p, h, t) in group {
                out.push((sided(n, s), sided(p, s), on(*h, s), on(*t, s)));
            }
        }
    }
    for s in Side::BOTH {
        for (f, pts) in HAND {
            for k in 0..3 {
                let parent = if k == 0 { format!("hand_{}", s.sx()) } else { format!("{f}_0{k}_{}", s.sx()) };
                out.push((format!("{f}_0{}_{}", k + 1, s.sx()), parent, on(pts[k], s), on(pts[k + 1], s)));
            }
        }
    }
    let names: Vec<String> = out.iter().map(|j| j.0.clone()).collect();
    Skeleton::new(
        out.into_iter()
            .map(|(name, parent, head, tail)| Joint { parent: names.iter().position(|n| *n == parent), name, head, tail })
            .collect(),
    )
}
