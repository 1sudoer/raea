//! causes1's touch and spots.json (WR3): the model handed to raea-sys-touch with each triangle's material and whether
//! it is walked on, and each texel with what the other maps already say there (its height over the walk, the rain's
//! reach, the run-off passing, the nearness to an arris). The same texels go to raea-sys-damage (WR6): causes3's a
//! is each one's nearness to a crack from an opening's corner, and debris joins the spots with its loss potential.

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;
use raea_sys_damage as damage;
use raea_sys_touch::{Mat, Point, Spot, Surface, mark};
use serde_json::{Value, json};

use crate::Env;
use crate::charts::Atlas;
use crate::flow::walks;
use crate::rays::GROUND_RANGE;
use crate::scene::{Class, Scene};

fn mat(c: Class) -> Mat {
    match c {
        Class::Plaster => Mat::Plaster,
        Class::Brick | Class::Sandstone | Class::Granite => Mat::Masonry,
        Class::Wood => Mat::Wood,
        Class::Iron => Mat::Metal,
        _ => Mat::Other,
    }
}

/// Touch into causes1's a; the spots, as JSON, and each texel's crack byte (causes3's a).
pub(crate) fn touch(sc: &Scene, at: &Atlas, env: &Env, mesh: &TriMesh, c0: &[[u8; 4]], c1: &mut [[u8; 4]], c2: &[[u8; 4]]) -> (Value, Vec<u8>) {
    let mats: Vec<Mat> = sc.class.iter().map(|&c| mat(c)).collect();
    let walk: Vec<bool> = (0..sc.tri.len()).map(|t| walks(sc, env, t)).collect();
    let s = Surface { pos: &sc.pos, tri: &sc.tri, nrm: &sc.nrm, mat: &mats, walk: &walk, mesh, up: env.up };
    let f = |b: u8| b as f32 / 255.0;
    let pts: Vec<Point> = at.texels.iter().map(|tx| {
        let i = at.px(tx.x, tx.y);
        Point { p: tx.p, n: tx.n, tri: tx.tri, ground: f(c2[i][0]) * GROUND_RANGE, rain: f(c0[i][0]), flow: f(c1[i][0]), edge: f(c0[i][3]) }
    }).collect();
    let (touch, spots) = mark(&s, &pts);
    let (cracks, _) = damage::cracks(&s, &pts);
    for (tx, v) in at.texels.iter().zip(touch) {
        c1[at.px(tx.x, tx.y)][3] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    let crack = cracks.iter().map(|d| ((1.0 - d / damage::REACH).clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    let v3 = |v: Vec3| json!([round(v.x), round(v.y), round(v.z)]);
    let spot = |s: &Spot| json!({ "what": s.what, "by": s.by, "at": v3(s.at), "n": v3(s.n), "size": s.size });
    let mut out: Vec<Value> = spots.iter().map(spot).collect();
    out.extend(damage::debris(&s, &pts, &cracks).iter().map(|(s, l)| {
        let mut v = spot(s);
        v["loss"] = json!(round(*l));
        v
    }));
    (json!(out), crack)
}

fn round(v: f32) -> f64 {
    (f64::from(v) * 1000.0).round() / 1000.0
}
