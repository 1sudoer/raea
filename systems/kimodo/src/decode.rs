//! Features to joints: KimodoMotionRep.inverse, then SOMASkeleton30.output_to_SOMASkeleton77. Each frame's 369
//! numbers (normalised) are: smooth root place 3, heading cos and sin 2, the 30 joints' places from the smooth root
//! 90, their world turns as two matrix columns 180, their speeds 90, foot contacts 4.

use crate::skeleton::{M3, Skeleton, fk, mul, transpose};
use crate::stats::Stats;

pub const DIM: usize = 369;
pub const J: usize = 30;

pub struct Motion {
    pub local77: Vec<Vec<M3>>,
    pub global77: Vec<Vec<M3>>,
    pub joints77: Vec<Vec<[f32; 3]>>,
    pub root: Vec<[f32; 3]>,
    pub smooth_root: Vec<[f32; 3]>,
    pub heading: Vec<[f32; 2]>,
    /// left heel, left toe, left toe end, right heel, right toe, right toe end
    pub contacts: Vec<[bool; 6]>,
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3])
}

/// Two columns of a turn (6 numbers) -> the turn: Gram-Schmidt, columns x, z cross x, z
pub fn cont6d(c: &[f32]) -> M3 {
    let x = norm([c[0], c[1], c[2]]);
    let z = norm(cross(x, [c[3], c[4], c[5]]));
    let y = cross(z, x);
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// One clip's features [T * 369], normalised, to the 77 joints.
pub fn decode(features: &[f32], st: &Stats, sk: &Skeleton) -> Motion {
    let t_n = features.len() / DIM;
    let f: Vec<f32> = features.iter().enumerate().map(|(i, v)| v * st.std[i % DIM] + st.mean[i % DIM]).collect();
    let (mut local77, mut root, mut smooth, mut heading, mut contacts) = (vec![], vec![], vec![], vec![], vec![]);
    for t in 0..t_n {
        let r = &f[t * DIM..(t + 1) * DIM];
        let g30: Vec<M3> = (0..J).map(|j| cont6d(&r[95 + j * 6..95 + j * 6 + 6])).collect();
        let mut loc = sk.relaxed77.clone();
        for j in 0..J {
            let p = sk.parents30[j];
            let parent = if p < 0 || j == 0 { crate::skeleton::EYE } else { g30[p as usize] };
            loc[sk.slice30in77[j]] = mul(&transpose(&parent), &g30[j]);
        }
        local77.push(loc);
        let s = [r[0], r[1], r[2]];
        root.push([r[5] + s[0], r[6], r[7] + s[2]]);
        smooth.push(s);
        heading.push([r[3], r[4]]);
        let c = [r[365] > 0.5, r[366] > 0.5, r[367] > 0.5, r[368] > 0.5];
        contacts.push([c[0], c[1], c[1], c[2], c[3], c[3]]);
    }
    let (global77, joints77) = fk(&local77, &root, &sk.parents77, &sk.neutral77);
    Motion { local77, global77, joints77, root, smooth_root: smooth, heading, contacts }
}

/// A turn as a quaternion x y z w, as the cloud runner writes them (kimodo_constraints.py `quat`).
pub fn quat(m: &M3) -> [f64; 4] {
    let m = m.map(|r| r.map(|v| v as f64));
    let w = (1.0 + m[0][0] + m[1][1] + m[2][2]).max(1e-12).sqrt() / 2.0;
    let x = (1.0 + m[0][0] - m[1][1] - m[2][2]).max(0.0).sqrt() / 2.0;
    let y = (1.0 - m[0][0] + m[1][1] - m[2][2]).max(0.0).sqrt() / 2.0;
    let z = (1.0 - m[0][0] - m[1][1] + m[2][2]).max(0.0).sqrt() / 2.0;
    let (x, y, z) = (x.copysign(m[2][1] - m[1][2]), y.copysign(m[0][2] - m[2][0]), z.copysign(m[1][0] - m[0][1]));
    let n = (x * x + y * y + z * z + w * w).sqrt();
    [x / n, y / n, z / n, w / n]
}

/// to `places` decimals, as the cloud runner wrote them
fn round(v: f64, places: i32) -> f64 {
    format!("{v:.*}", places as usize).parse().unwrap_or(v)
}

/// The clip file sdk/body/tools/fromkimodo.ts reads: {fps, names (the 77), root (the hips' place a frame), turns (each
/// joint's world turn from SOMA's T-pose, x y z w), contacts, seed, steps, source, prompt}.
pub fn clip_json(m: &Motion, sk: &Skeleton, seed: u64, steps: usize, prompt: &str, constrained: bool) -> serde_json::Value {
    clip_json_on(m, sk, seed, steps, prompt, constrained, "on this Mac (raea-sys-kimodo, MLX)")
}

/// clip_json, its source saying where it was made (`made_on`: "on this Mac (raea-sys-kimodo, MLX)", or PyTorch's
/// device), for one file whichever backend made the clip.
pub fn clip_json_on(m: &Motion, sk: &Skeleton, seed: u64, steps: usize, prompt: &str, constrained: bool, made_on: &str) -> serde_json::Value {
    let source = format!(
        "Kimodo-SOMA-RP-v1 (nv-tlabs/kimodo), NVIDIA Open Model License, {made_on}; {}",
        match (prompt.trim().is_empty(), constrained) {
            (false, true) => format!("text: {prompt:?}, and constraints"),
            (false, false) => format!("text: {prompt:?}"),
            (true, _) => "constraints only, no text".into(),
        }
    );
    serde_json::json!({
        "fps": 30,
        "names": sk.names77,
        "root": m.joints77.iter().map(|f| f[0].map(|v| round(v as f64, 5))).collect::<Vec<_>>(),
        "turns": m.global77.iter().map(|f| f.iter().map(|g| quat(g).map(|v| round(v, 6))).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "contacts": m.contacts.iter().map(|c| c.map(|b| b as u8)).collect::<Vec<_>>(),
        "seed": seed,
        "steps": steps,
        "source": source,
        "prompt": prompt,
    })
}
