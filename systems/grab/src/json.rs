//! A frame of a take asked for as JSON: the engine's native call `character.take`, which raea/character's `take`
//! makes each frame. The body's own motion comes as its poses at the times the take reads it (the frame's, the
//! grab's, the release's); the answer is the pose to draw and where the thing is.

use glam::Quat;
use raea_sys_body::json::{num, posed, quat, vec3};
use serde_json::{Value, json};

use crate::trunk::stand_for;
use crate::hand::{Grip, Hold, HoldOptions, find_holds};
use raea_sys_body::rig::{Pose, Side, Skeleton};
use raea_sys_body::shape::{Solid, Thing};
use crate::take::{Key, Release, Take, perform};

pub const TAKE_ARGS: &str = "{ joints: { name: string; parent: number; head: [number, number, number]; tail: [number, number, number] }[]; t: number; base: { t: number; root: [number, number, number]; turns: [number, number, number, number][] }[]; thing: { ball?: number; box?: [number, number, number]; capsule?: [number, number]; cylinder?: [number, number]; at: [number, number, number]; turn?: [number, number, number, number] }; start: number; grab?: number; carried?: number; carry?: [number, number, number]; hand?: \"L\" | \"R\"; lower?: number; holds?: { side: \"L\" | \"R\"; at: [number, number, number]; normal: [number, number, number]; along?: [number, number, number]; shape?: number[] }[]; path?: { t: number; carry: [number, number, number]; tilt?: [number, number, number, number] }[]; weight?: number; pass?: number; slide?: { t: number; at: [number, number, number]; turn?: [number, number, number, number] }[]; leave?: number; avoid?: { ball?: number; box?: [number, number, number]; capsule?: [number, number]; cylinder?: [number, number]; at: [number, number, number]; turn?: [number, number, number, number] }[]; put?: { at: number; down: number; place: [number, number, number] }; throw?: { at: number; velocity: [number, number, number]; floor?: number } }";
pub const TAKE_ANSWER: &str = "{ pose: { root: [number, number, number]; turns: [number, number, number, number][] }; thing: { at: [number, number, number]; turn: [number, number, number, number] }; phase: \"Before\" | \"Reach\" | \"Lift\" | \"Carry\" | \"Release\" | \"After\"; hands: 1 | 2; miss: number; moved: number; limited: boolean; stand: { at: [number, number, number]; yaw: number }; reach: { ok: boolean; move: number } }";

fn solid(v: &Value) -> Result<Solid, String> {
    if !v["ball"].is_null() {
        return Ok(Solid::Ball { r: num(&v["ball"], "thing.ball")? });
    }
    if !v["box"].is_null() {
        return Ok(Solid::Box { half: vec3(&v["box"], "thing.box")? });
    }
    // a capsule or a cylinder: half its length along its own y, then its radius
    let rod = ["capsule", "cylinder"].into_iter().find(|k| !v[*k].is_null()).ok_or("character.take: the thing needs a shape: ball, box, capsule or cylinder")?;
    let a = v[rod].as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("character.take: thing.{rod} isn't [half its length, radius]"))?;
    let (half, r) = (num(&a[0], rod)?, num(&a[1], rod)?);
    Ok(if rod == "capsule" { Solid::Capsule { half, r } } else { Solid::Cylinder { half, r } })
}

pub fn take_json(v: &Value) -> Result<Value, String> {
    let skel = Skeleton::from_value(v)?;
    let mut base: Vec<(f32, Pose)> = Vec::new();
    for b in v["base"].as_array().ok_or("character.take: no base poses")? {
        let turns = b["turns"].as_array().ok_or("character.take: a base pose has no turns")?;
        let mut pose = skel.rest();
        pose.root = vec3(&b["root"], "a base pose's root")?;
        for (slot, turn) in pose.turns.iter_mut().zip(turns) {
            *slot = quat(turn, "a base pose's turn")?;
        }
        base.push((num(&b["t"], "a base pose's t")?, pose));
    }
    if base.is_empty() {
        return Err("character.take: no base poses".into());
    }
    // the body's own motion at a time: the pose given nearest it
    let own = |t: f32| base.iter().min_by(|a, b| (a.0 - t).abs().total_cmp(&(b.0 - t).abs())).map(|b| b.1.clone()).unwrap_or_else(|| skel.rest());

    let placed = |shape: &Value| -> Result<Thing, String> {
        let turn = if shape["turn"].is_null() { Quat::IDENTITY } else { quat(&shape["turn"], "a thing's turn")? };
        Ok(Thing { solid: solid(shape)?, at: vec3(&shape["at"], "a thing's at")?, turn })
    };
    let thing = placed(&v["thing"])?;
    let start = num(&v["start"], "start")?;
    let mut options = HoldOptions::default();
    if v["hand"] == "L" {
        options.hand = Side::L;
    }
    if !v["lower"].is_null() {
        options.lower = num(&v["lower"], "lower")?;
    }
    // a thing held one way brings its holds (in its own frame); any other has them found from where the body
    // stands when it starts to reach
    let mut own_holds = Vec::new();
    for h in v["holds"].as_array().into_iter().flatten() {
        let along = if h["along"].is_null() { None } else { Some(vec3(&h["along"], "a hold's along")?.normalize_or_zero()) };
        // a shape's first three numbers (the ones left out are 0)
        let mut shape = None;
        if let Some(z) = h["shape"].as_array() {
            let mut first = [0.0f32; 3];
            for (slot, n) in first.iter_mut().zip(z) {
                *slot = num(n, "a number of a hold's shape")?;
            }
            shape = Some(first);
        }
        let side = if h["side"] == "L" { Side::L } else { Side::R };
        own_holds.push(Hold { side, at: vec3(&h["at"], "a hold's at")?, normal: vec3(&h["normal"], "a hold's normal")?.normalize_or_zero(), along, shape });
    }
    let grip = match own_holds[..] {
        [one] => Grip::One(one),
        [a, b, ..] => if a.side == Side::L { Grip::Two(a, b) } else { Grip::Two(b, a) },
        [] => find_holds(&thing, own(start).root, options),
    };
    let mut take = Take::new(thing, grip, start);
    if !v["weight"].is_null() {
        take = take.weighing(num(&v["weight"], "weight")?);
    }
    if !v["pass"].is_null() {
        take = take.passing(num(&v["pass"], "pass")?);
    }
    for k in v["slide"].as_array().into_iter().flatten() {
        let turn = if k["turn"].is_null() { thing.turn } else { quat(&k["turn"], "a slide key's turn")? };
        take.slide.push((num(&k["t"], "a slide key's t")?, vec3(&k["at"], "a slide key's at")?, turn));
    }
    if !v["leave"].is_null() {
        take.release = Some(Release::Leave { at: num(&v["leave"], "leave")? });
    }
    for k in v["path"].as_array().into_iter().flatten() {
        let tilt = if k["tilt"].is_null() { Quat::IDENTITY } else { quat(&k["tilt"], "a path key's tilt")? };
        take.path.push(Key { t: num(&k["t"], "a path key's t")?, carry: vec3(&k["carry"], "a path key's carry")?, tilt });
    }
    if !v["grab"].is_null() {
        take.grab = num(&v["grab"], "grab")?;
        take.carried = take.grab + 0.5;
    }
    if !v["carried"].is_null() {
        take.carried = num(&v["carried"], "carried")?;
    }
    if !v["carry"].is_null() {
        take.carry = vec3(&v["carry"], "carry")?;
    }
    for solid in v["avoid"].as_array().into_iter().flatten() {
        take.avoid.push(placed(solid)?);
    }
    if let Some(p) = v.get("put").filter(|p| !p.is_null()) {
        take.release = Some(Release::Put { at: num(&p["at"], "put.at")?, down: num(&p["down"], "put.down")?, place: vec3(&p["place"], "put.place")? });
    } else if let Some(t) = v.get("throw").filter(|t| !t.is_null()) {
        let floor = if t["floor"].is_null() { 0.0 } else { num(&t["floor"], "throw.floor")? };
        take.release = Some(Release::Throw { at: num(&t["at"], "throw.at")?, velocity: vec3(&t["velocity"], "throw.velocity")?, floor });
    }

    let f = perform(&skel, &own, &take, num(&v["t"], "t")?);
    // where the body would stand to take it, for a scene that walks it there first
    let (stand, facing) = stand_for(&thing, own(start).root);
    // whether the body can take it from where it stands when it starts to reach: at the grab, every wrist on its hold
    let in_reach = perform(&skel, &own, &take, take.grab).miss < 0.02;
    Ok(json!({
        "pose": posed(&f.pose),
        "thing": { "at": f.thing.at.to_array(), "turn": f.thing.turn.to_array() },
        "phase": format!("{:?}", f.phase),
        "hands": if matches!(grip, Grip::One(_)) { 1 } else { 2 },
        "miss": f.miss,
        "moved": f.moved,
        "limited": f.limited,
        "stand": { "at": stand.to_array(), "yaw": facing },
        "reach": { "ok": in_reach, "move": if in_reach { 0.0 } else { (stand - own(start).root).length() } },
    }))
}
