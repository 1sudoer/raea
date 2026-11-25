//! SOMA's 77-joint skeleton and the 30 joints Kimodo's features hold, as Kimodo's code builds them (soma77.json:
//! parents, rest places, the relaxed hands' local turns, which of the 77 the 30 are).

use anyhow::{Context, Result};
use serde_json::Value;

pub type M3 = [[f32; 3]; 3];

pub struct Skeleton {
    pub names30: Vec<String>,
    pub names77: Vec<String>,
    pub parents30: Vec<i32>,
    pub parents77: Vec<i32>,
    pub neutral30: Vec<[f32; 3]>,
    pub neutral77: Vec<[f32; 3]>,
    /// each of the 77's local turn in the relaxed-hands rest pose; the 30 Kimodo makes are written over it
    pub relaxed77: Vec<M3>,
    /// where each of the 30 sits in the 77
    pub slice30in77: Vec<usize>,
    /// the right and the left hip among the 30
    pub hips30: [usize; 2],
}

fn ints(v: &Value, k: &str) -> Result<Vec<i64>> {
    v[k].as_array().with_context(|| format!("soma77.json: no {k}"))?.iter().map(|x| x.as_i64().context("not an integer")).collect()
}

/// every number in a JSON value, nested arrays flattened
pub(crate) fn floats(v: &Value) -> Vec<f32> {
    match v {
        Value::Array(a) => a.iter().flat_map(floats).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap_or(0.0) as f32],
        _ => vec![],
    }
}

fn v3(v: &Value, k: &str) -> Vec<[f32; 3]> {
    floats(&v[k]).chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
}

fn names(v: &Value, k: &str) -> Vec<String> {
    v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

impl Skeleton {
    pub fn soma() -> Result<Self> {
        let v: Value = serde_json::from_str(include_str!("soma77.json"))?;
        let hips = ints(&v, "hips30")?;
        let sk = Skeleton {
            names30: names(&v, "names30"),
            names77: names(&v, "names77"),
            parents30: ints(&v, "parents30")?.into_iter().map(|p| p as i32).collect(),
            parents77: ints(&v, "parents77")?.into_iter().map(|p| p as i32).collect(),
            neutral30: v3(&v, "neutral30"),
            neutral77: v3(&v, "neutral77"),
            relaxed77: floats(&v["relaxed77"]).chunks(9).map(|c| [[c[0], c[1], c[2]], [c[3], c[4], c[5]], [c[6], c[7], c[8]]]).collect(),
            slice30in77: ints(&v, "slice30in77")?.into_iter().map(|i| i as usize).collect(),
            hips30: [hips[0] as usize, hips[1] as usize],
        };
        anyhow::ensure!(
            sk.names30.len() == 30 && sk.names77.len() == 77 && sk.relaxed77.len() == 77 && sk.neutral77.len() == 77 && sk.slice30in77.len() == 30,
            "soma77.json: a table has the wrong length"
        );
        Ok(sk)
    }
}

pub fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

pub fn apply(a: &M3, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

pub fn transpose(a: &M3) -> M3 {
    [0, 1, 2].map(|i| [a[0][i], a[1][i], a[2][i]])
}

pub const EYE: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Forward kinematics: local turns and the root's place per frame -> each joint's world turn and place. `neutral` is
/// the rest pose's joint places (only their offsets from the parent matter).
pub fn fk(local: &[Vec<M3>], root: &[[f32; 3]], parents: &[i32], neutral: &[[f32; 3]]) -> (Vec<Vec<M3>>, Vec<Vec<[f32; 3]>>) {
    let n = parents.len();
    let mut g_all = Vec::with_capacity(local.len());
    let mut p_all = Vec::with_capacity(local.len());
    for (t, loc) in local.iter().enumerate() {
        let mut g = vec![EYE; n];
        let mut p = vec![[0.0f32; 3]; n];
        for j in 0..n {
            let par = parents[j];
            if par < 0 {
                g[j] = loc[j];
                continue;
            }
            let par = par as usize;
            debug_assert!(par < j);
            g[j] = mul(&g[par], &loc[j]);
            let off = [0, 1, 2].map(|k| neutral[j][k] - neutral[par][k]);
            let r = apply(&g[par], off);
            p[j] = [0, 1, 2].map(|k| p[par][k] + r[k]);
        }
        for q in p.iter_mut() {
            for k in 0..3 {
                q[k] += root[t][k];
            }
        }
        g_all.push(g);
        p_all.push(p);
    }
    (g_all, p_all)
}
