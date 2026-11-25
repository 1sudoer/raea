//! causes1's flow, deposit and its kind (WR2): a drop on every texel the rain reaches, its water the rain's reach
//! times the texel's area, walked over the model by raea-sys-runoff. Iron gives rust, zinc white, copper green, and any
//! other face that looks up gives the dust that lies on it; the walking surfaces end a drop, so do roofs (they drain to
//! their gutters).

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;
use raea_sys_runoff::{Drop, Kind, Role, Surface, run};

use crate::charts::Atlas;
use crate::scene::{Class, Scene};
use crate::{Env, Walk};

pub(crate) fn flow(sc: &Scene, at: &Atlas, env: &Env, mesh: &TriMesh, c0: &[[u8; 4]], c1: &mut [[u8; 4]]) {
    let mut uv = vec![[[0f32; 2]; 3]; sc.tri.len()];
    for (k, pr) in sc.prims.iter().enumerate() {
        for j in 0..pr.tris {
            for c in 0..3 {
                let u = at.uv[k][at.index[k][3 * j + c] as usize];
                uv[pr.first_tri + j][c] = [u[0] * at.w as f32, u[1] * at.h as f32];
            }
        }
    }
    let role: Vec<Role> = (0..sc.tri.len()).map(|t| role_of(sc, env, t)).collect();
    let s = Surface { pos: &sc.pos, tri: &sc.tri, nrm: &sc.nrm, role: &role, uv: &uv, mesh };
    let dens = |t: u32| at.chart_dens[at.chart_of[t as usize] as usize];
    let drops: Vec<Drop> = at.texels.iter().filter_map(|tx| {
        let sky = c0[at.px(tx.x, tx.y)][0] as f32 / 255.0;
        (sky > 0.02).then(|| Drop { p: tx.p + tx.n * 0.002, tri: tx.tri, water: sky / dens(tx.tri).powi(2) })
    }).collect();
    let out = run(&s, &drops, env.up, at.w, at.h);
    for tx in &at.texels {
        let i = at.px(tx.x, tx.y);
        let [f, d, k] = out.bytes(i, dens(tx.tri));
        c1[i] = [f, d, k, c1[i][3]];
    }
}

/// Whether triangle t is a walking surface: of a walking class, or level at the walking height.
pub(crate) fn walks(sc: &Scene, env: &Env, t: usize) -> bool {
    match &env.walk {
        Walk::Classes(cs) => cs.contains(&sc.class[t]),
        Walk::Level(y) => sc.nrm[t].dot(env.up) > 0.9 && sc.tri[t].iter().all(|&v| sc.pos[v as usize].dot(env.up) <= y + 0.05),
    }
}

fn role_of(sc: &Scene, env: &Env, t: usize) -> Role {
    let (c, n, walk) = (sc.class[t], sc.nrm[t], walks(sc, env, t));
    let gives = match c {
        Class::Iron => Kind::Rust,
        Class::Zinc => Kind::Zinc,
        Class::Copper => Kind::Copper,
        _ if n.dot(env.up) > 0.5 => Kind::Grime,
        _ => Kind::None,
    };
    Role { gives, walk, drains: matches!(c, Class::Slate | Class::Felt) && n.dot(env.up) > 0.2 || n == Vec3::ZERO }
}
