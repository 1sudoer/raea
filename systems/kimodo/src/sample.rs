//! Kimodo's sampler (kimodo/model/kimodo_model.py, diffusion.py, cfg.py): a cosine schedule on 1000 base steps
//! respaced to N, deterministic DDIM (eta 0) on the model's clean-motion prediction, and separated guidance:
//! uncond + w_text (text - uncond) + w_constraint (constrained - uncond). Kimodo always runs the three passes as one
//! batch; a pass whose inputs equal the unconditional one's (no text, or no constraints) is run once here.

use anyhow::Result;
use mlx_rs::{Array, ops};

use crate::decode::DIM;
use crate::net::{Net, TEXT_DIM};

pub const BASE_STEPS: usize = 1000;

pub struct Schedule {
    pub use_t: Vec<i32>,
    pub recip: Vec<f32>,
    pub recipm1: Vec<f32>,
    pub prev: Vec<f32>,
}

/// In f32 as PyTorch (and the MLX reference) compute it.
pub fn schedule(steps: usize) -> Schedule {
    let ab = |t: f64| ((t + 0.008) / 1.008 * std::f64::consts::PI / 2.0).cos().powi(2);
    let betas: Vec<f32> = (0..BASE_STEPS).map(|i| (1.0 - ab((i + 1) as f64 / BASE_STEPS as f64) / ab(i as f64 / BASE_STEPS as f64)).min(0.999) as f32).collect();
    let mut base = Vec::with_capacity(BASE_STEPS);
    let mut acc = 1.0f32;
    for b in &betas {
        acc *= 1.0 - b;
        base.push(acc);
    }
    // (numpy: the fraction divided in f64 and cast, the products rounded half to even)
    let frac = ((BASE_STEPS - 1) as f64 / (steps.max(2) - 1) as f64) as f32;
    let use_t: Vec<i32> = (0..steps).map(|i| ((i as f32 * frac).round_ties_even() as i32).min(BASE_STEPS as i32 - 1)).collect();
    let ac: Vec<f32> = use_t.iter().map(|&u| base[u as usize]).collect();
    let mut acp = Vec::with_capacity(steps);
    let mut acc = 1.0f32;
    for (i, a) in ac.iter().enumerate() {
        let before = if i == 0 { 1.0 } else { ac[i - 1] };
        let b = 1.0 - a / before;
        acc *= 1.0 - b;
        acp.push(acc.max(1e-9));
    }
    let prev: Vec<f32> = (0..steps).map(|i| if i == 0 { 1.0 } else { acp[i - 1] }).collect();
    Schedule {
        use_t,
        recip: acp.iter().map(|a| 1.0 / a.sqrt()).collect(),
        recipm1: acp.iter().map(|a| 1.0 / (a / (1.0 - a)).sqrt()).collect(),
        prev,
    }
}

/// One clip [frames, 369] from `noise` (normalised features), a text embedding (4096 numbers, or none) and conditions.
pub fn sample(net: &Net, noise: &[f32], frames: usize, steps: usize, text: Option<&[f32]>, cond: Option<(&[f32], &[f32])>, cfg: (f32, f32)) -> Result<Vec<f32>> {
    let t_n = frames as i32;
    let mut x = Array::from_slice(noise, &[1, t_n, DIM as i32]);
    let zt = ops::zeros::<f32>(&[1, 1, TEXT_DIM])?;
    let text = text.map(|t| Array::from_slice(t, &[1, 1, TEXT_DIM]));
    let cond_in = cond;
    let cond = cond.map(|(obs, mask)| (Array::from_slice(mask, &[1, t_n, DIM as i32]), Array::from_slice(obs, &[1, t_n, DIM as i32])));
    let has_text = text.is_some();
    let any_con = cond_in.is_some_and(|(_, m)| m.iter().any(|&v| v > 0.0));
    let has_con = any_con;
    // the passes: unconditional; text and no constraints; constraints and no text
    let zm = ops::zeros::<f32>(&[1, t_n, DIM as i32])?;
    let mut texts = vec![zt.clone()];
    let mut masks = vec![zm.clone()];
    if let Some(t) = &text {
        texts.push(t.clone());
        masks.push(zm.clone());
    }
    if has_con {
        texts.push(zt.clone());
        masks.push(cond.as_ref().unwrap().0.clone());
    }
    let n = texts.len() as i32;
    let text_b = ops::concatenate(&texts, 0)?;
    let mask_b = ops::concatenate(&masks, 0)?;
    let obs_b = match &cond {
        Some((_, o)) => ops::concatenate(&vec![o.clone(); n as usize], 0)?,
        None => ops::zeros::<f32>(&[n, t_n, DIM as i32])?,
    };
    let lengths = vec![t_n; n as usize];
    let heading = ops::zeros::<f32>(&[n])?;
    let s = schedule(steps);
    for i in (0..steps).rev() {
        let xb = ops::concatenate(&vec![x.clone(); n as usize], 0)?;
        let t = Array::from_slice(&vec![s.use_t[i]; n as usize], &[n]);
        let out = net.denoise(&xb, &lengths, &text_b, &t, &heading, Some((&mask_b, &obs_b)))?;
        let parts = ops::split_equal(&out, n, 0)?;
        let unc = &parts[0];
        let mut pred = unc.clone();
        if has_text {
            pred = ops::add(&pred, &ops::multiply(&ops::subtract(&parts[1], unc)?, &Array::from_f32(cfg.0))?)?;
        }
        if has_con {
            pred = ops::add(&pred, &ops::multiply(&ops::subtract(&parts[n as usize - 1], unc)?, &Array::from_f32(cfg.1))?)?;
        }
        let eps = ops::divide(&ops::subtract(&ops::multiply(&x, &Array::from_f32(s.recip[i]))?, &pred)?, &Array::from_f32(s.recipm1[i]))?;
        let (a, b) = ((s.prev[i] as f64).sqrt() as f32, (1.0 - s.prev[i] as f64).sqrt() as f32);
        x = ops::add(&ops::multiply(&pred, &Array::from_f32(a))?, &ops::multiply(&eps, &Array::from_f32(b))?)?;
        x.eval()?;
    }
    crate::net::f32_array(&x)
}

/// Starting noise from a seed: Box-Muller over a PCG32 stream. Not numpy's or torch's: a seed here is not the same
/// motion as that seed in the cloud.
pub fn noise(seed: u64, n: usize) -> Vec<f32> {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let x = (((state >> 18) ^ state) >> 27) as u32;
        let rot = (state >> 59) as u32;
        (x.rotate_right(rot) as f64 + 0.5) / 4294967296.0
    };
    let mut out = Vec::with_capacity(n + 1);
    while out.len() < n {
        let (u, v) = (next(), next());
        let r = (-2.0 * u.ln()).sqrt();
        out.push((r * (2.0 * std::f64::consts::PI * v).cos()) as f32);
        out.push((r * (2.0 * std::f64::consts::PI * v).sin()) as f32);
    }
    out.truncate(n);
    out
}
