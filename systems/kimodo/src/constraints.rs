//! A Kimodo constraints file (full-body keyframes, hand and foot targets, a root path: what sdk/body/tools/between.ts
//! writes) turned into the denoiser's conditions: the observed features (normalised) and which of them are given.
//! kimodo/constraints.py with KimodoMotionRep.create_conditions; where two constraints give one number, the last wins.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::decode::{DIM, J};
use crate::skeleton::{M3, Skeleton, fk, floats as nums};
use crate::stats::Stats;

/// Each of the frames' 369 numbers: what is asked (normalised) and whether it is asked (1) or free (0).
pub struct Conditions {
    pub obs: Vec<f32>,
    pub mask: Vec<f32>,
}

fn axis_angle(a: [f32; 3]) -> M3 {
    let angle = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let (x, y, z) = (a[0] / (angle + 1e-6), a[1] / (angle + 1e-6), a[2] / (angle + 1e-6));
    let k = [[0.0, -z, y], [z, 0.0, -x], [-y, x, 0.0]];
    let kk = crate::skeleton::mul(&k, &k);
    let (s, c) = (angle.sin(), 1.0 - angle.cos());
    let mut m = crate::skeleton::EYE;
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] += s * k[i][j] + c * kk[i][j];
        }
    }
    m
}

fn end_effector(class: &str) -> Option<&'static [&'static str]> {
    Some(match class {
        "left-hand" => &["LeftHand"],
        "right-hand" => &["RightHand"],
        "left-foot" => &["LeftFoot"],
        "right-foot" => &["RightFoot"],
        _ => return None,
    })
}

/// the joints whose places pin an end effector (the first is the one whose turn is pinned too)
fn ee_places(base: &str) -> Result<&'static [&'static str]> {
    Ok(match base {
        "LeftFoot" => &["LeftFoot", "LeftToeBase"],
        "RightFoot" => &["RightFoot", "RightToeBase"],
        "LeftHand" => &["LeftHand", "LeftHandMiddleEnd"],
        "RightHand" => &["RightHand", "RightHandMiddleEnd"],
        "Hips" => &["Hips"],
        _ => bail!("kimodo: no end effector {base}"),
    })
}

pub fn conditions(saved: &Value, frames: usize, st: &Stats, sk: &Skeleton) -> Result<Conditions> {
    let list = saved.as_array().context("kimodo: a constraints file is a list")?;
    let idx: BTreeMap<&str, usize> = sk.names30.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
    let [r_hip, l_hip] = sk.hips30;
    // (frame, joint or 0) -> values, the last given winning
    let mut smooth: BTreeMap<usize, [f32; 2]> = BTreeMap::new();
    let mut root_y: BTreeMap<usize, f32> = BTreeMap::new();
    let mut heading: BTreeMap<usize, [f32; 2]> = BTreeMap::new();
    let mut rots: BTreeMap<(usize, usize), M3> = BTreeMap::new();
    let mut places: BTreeMap<(usize, usize), [f32; 3]> = BTreeMap::new();
    for el in list {
        let fi: Vec<usize> = el["frame_indices"].as_array().context("kimodo: a constraint without frame_indices")?.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect();
        let kind = el["type"].as_str().unwrap_or("");
        let given_smooth = |k: usize| -> Option<[f32; 2]> {
            let s = nums(&el["smooth_root_2d"]);
            let per = s.len() / fi.len().max(1);
            (per >= 2).then(|| [s[k * per], s[k * per + 1]])
        };
        if kind == "root2d" {
            let h = nums(&el["global_root_heading"]);
            for (k, &f) in fi.iter().enumerate() {
                if let Some(s) = given_smooth(k) {
                    smooth.insert(f, s);
                }
                if h.len() >= 2 * fi.len() {
                    heading.insert(f, [h[2 * k], h[2 * k + 1]]);
                }
            }
            continue;
        }
        let aa = nums(&el["local_joints_rot"]);
        let per = aa.len() / 3 / fi.len().max(1);
        anyhow::ensure!(per == 30 || per == 77, "kimodo: local_joints_rot has {per} joints a frame, not 30 or 77");
        let local: Vec<Vec<M3>> = (0..fi.len())
            .map(|k| {
                let all: Vec<M3> = (0..per).map(|j| axis_angle([aa[(k * per + j) * 3], aa[(k * per + j) * 3 + 1], aa[(k * per + j) * 3 + 2]])).collect();
                if per == 77 { sk.slice30in77.iter().map(|&i| all[i]).collect() } else { all }
            })
            .collect();
        let rp = nums(&el["root_positions"]);
        let root: Vec<[f32; 3]> = rp.chunks_exact(3).take(fi.len()).map(|c| [c[0], c[1], c[2]]).collect();
        let (g, pos) = fk(&local, &root, &sk.parents30, &sk.neutral30);
        let (pj, rj): (Vec<usize>, Vec<usize>) = if kind == "fullbody" {
            ((0..J).collect(), vec![])
        } else {
            let base: Vec<String> = match end_effector(kind) {
                Some(b) => b.iter().map(|s| s.to_string()).collect(),
                None => el["joint_names"].as_array().context("kimodo: an end-effector constraint without joint_names")?.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            };
            let mut pj = vec![];
            let mut rj = vec![];
            for b in &base {
                let names = ee_places(b)?;
                for n in names {
                    pj.push(*idx.get(n).with_context(|| format!("kimodo: no joint {n}"))?);
                }
                rj.push(idx[names[0]]);
            }
            (pj, rj)
        };
        for (k, &f) in fi.iter().enumerate() {
            for &j in &pj {
                places.insert((f, j), pos[k][j]);
            }
            for &j in &rj {
                rots.insert((f, j), g[k][j]);
            }
            let d = [0, 1, 2].map(|c| pos[k][r_hip][c] - pos[k][l_hip][c]);
            let ang = d[2].atan2(-d[0]);
            smooth.insert(f, given_smooth(k).unwrap_or([pos[k][0][0], pos[k][0][2]]));
            root_y.insert(f, pos[k][0][1]);
            heading.insert(f, [ang.cos(), ang.sin()]);
        }
    }
    let mut obs = vec![0.0f32; frames * DIM];
    let mut mask = vec![0.0f32; frames * DIM];
    fn put(obs: &mut [f32], mask: &mut [f32], frames: usize, f: usize, c: usize, v: f32) {
        if f < frames {
            obs[f * DIM + c] = v;
            mask[f * DIM + c] = 1.0;
        }
    }
    macro_rules! set {
        ($f:expr, $c:expr, $v:expr) => {
            put(&mut obs, &mut mask, frames, $f, $c, $v)
        };
    }
    for (&f, s) in &smooth {
        set!(f, 0, s[0]);
        set!(f, 2, s[1]);
    }
    for (&f, &y) in &root_y {
        set!(f, 1, y);
    }
    for (&f, h) in &heading {
        set!(f, 3, h[0]);
        set!(f, 4, h[1]);
    }
    for (&(f, j), r) in &rots {
        for k in 0..3 {
            set!(f, 95 + j * 6 + k, r[k][0]);
            set!(f, 95 + j * 6 + 3 + k, r[k][1]);
        }
    }
    for (&(f, j), p) in &places {
        if f >= frames {
            continue;
        }
        anyhow::ensure!(
            mask[f * DIM] > 0.0 && mask[f * DIM + 2] > 0.0,
            "kimodo: to pin joint places the smooth root must be pinned on those frames too (frame {f})"
        );
        let (rx, rz) = (obs[f * DIM], obs[f * DIM + 2]);
        set!(f, 5 + j * 3, p[0] - rx);
        set!(f, 5 + j * 3 + 1, p[1]);
        set!(f, 5 + j * 3 + 2, p[2] - rz);
    }
    for i in 0..frames * DIM {
        obs[i] = (obs[i] - st.mean[i % DIM]) / st.std[i % DIM];
    }
    Ok(Conditions { obs, mask })
}
