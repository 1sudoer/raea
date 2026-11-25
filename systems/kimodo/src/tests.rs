//! Against the PyTorch reference (nv-tlabs/kimodo on the CPU, float32) and the float32 text encoder: fixtures made by
//! ~/.cache/raea-kimodo/mlx (ref_torch.py, then converted to safetensors in ref/rust), the weights from the models
//! folder or RAEA_KIMODO_DIR / RAEA_KIMODO_TEXT_DIR. A test whose files are absent says so and passes.
//! Run one at a time (`-- --test-threads=1`): the text encoder holds 9 GB.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Once;

use mlx_rs::Array;

use crate::decode::DIM;
use crate::net::f32_array;

static TF32_OFF: Once = Once::new();

/// float32 parity needs MLX's TF32 matmuls off, process-wide, before the first matmul
fn exact() {
    TF32_OFF.call_once(|| unsafe { std::env::set_var("MLX_ENABLE_TF32", "0") });
}

fn cache() -> PathBuf {
    raea_io_home::user_home().unwrap().join(".cache/raea-kimodo/mlx")
}

fn fixture(name: &str) -> Option<HashMap<String, Array>> {
    let p = std::env::var("RAEA_KIMODO_REF").map(PathBuf::from).unwrap_or_else(|_| cache().join("ref/rust")).join(name);
    if !p.exists() {
        eprintln!("skipped: no fixture {}", p.display());
        return None;
    }
    Some(Array::load_safetensors(&p).unwrap())
}

fn motion_dir() -> Option<PathBuf> {
    let d = std::env::var("RAEA_KIMODO_DIR").map(PathBuf::from).unwrap_or_else(|_| cache().join("weights/Kimodo-SOMA-RP-v1"));
    if d.join("model.safetensors").exists() { Some(d) } else { eprintln!("skipped: no Kimodo checkpoint at {}", d.display()); None }
}

fn text_dir() -> Option<PathBuf> {
    let d = std::env::var("RAEA_KIMODO_TEXT_DIR").map(PathBuf::from).unwrap_or_else(|_| cache().join("weights/llama3-8b-instruct-8bit"));
    if d.join("llm2vec/mntp.safetensors").exists() { Some(d) } else { eprintln!("skipped: no text encoder at {}", d.display()); None }
}

fn v(m: &HashMap<String, Array>, k: &str) -> Vec<f32> {
    f32_array(&m[k]).unwrap()
}

fn max_abs(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

#[test]
fn sanitising_a_line_matches_kimodo() {
    use crate::text::sanitize;
    assert_eq!(sanitize("  a person walks forward and stops "), "A person walks forward and stops.");
    assert_eq!(sanitize("wave at the camera"), "Wave at the camera.");
    assert_eq!(sanitize("A drunk man staggers. then he SITS down!"), "A drunk man staggers. Then he sits down!");
    assert_eq!(sanitize("   "), "");
}

#[test]
fn the_denoiser_matches_pytorch() {
    exact();
    let (Some(r), Some(dir)) = (fixture("denoiser.safetensors"), motion_dir()) else { return };
    let st = crate::stats::Stats::load(&dir).unwrap();
    let net = crate::net::Net::load(&dir, &st).unwrap();
    // A: zero text, no constraints, t = 500, 150 frames
    let t = Array::from_slice(&[v(&r, "A_t")[0] as i32], &[1]);
    let y = net.denoise(&r["A_x"], &[150], &mlx_rs::ops::zeros::<f32>(&[1, 1, 4096]).unwrap(), &t, &mlx_rs::ops::zeros::<f32>(&[1]).unwrap(), None).unwrap();
    let err = max_abs(&f32_array(&y).unwrap(), &v(&r, "A_y"));
    assert!(err < 2e-3, "A: max abs difference {err}");
    // B: two clips of different lengths, 12 text tokens, constraints, headings; compared on each clip's own frames
    let pad = v(&r, "B_pad");
    let lengths: Vec<i32> = pad.chunks(120).map(|c| c.iter().filter(|&&p| p > 0.5).count() as i32).collect();
    let tb: Vec<i32> = v(&r, "B_t").iter().map(|&x| x as i32).collect();
    let y = net
        .denoise(&r["B_x"], &lengths, &r["B_text"], &Array::from_slice(&tb, &[2]), &r["B_head"], Some((&r["B_mask"], &r["B_obs"])))
        .unwrap();
    let (got, want) = (f32_array(&y).unwrap(), v(&r, "B_y"));
    let mut err = 0f32;
    for (c, &l) in lengths.iter().enumerate() {
        let s = c * 120 * DIM;
        err = err.max(max_abs(&got[s..s + l as usize * DIM], &want[s..s + l as usize * DIM]));
    }
    assert!(err < 2e-3, "B: max abs difference {err} (lengths {lengths:?})");
}

#[test]
fn a_whole_generation_matches_pytorch() {
    exact();
    let (Some(r), Some(dir)) = (fixture("gen_text_walk_stop.safetensors"), motion_dir()) else { return };
    let k = crate::Kimodo::open(&dir, None).unwrap();
    let ask = crate::Ask { line: "A person walks forward and stops.", frames: 150, seed: 0, steps: 100, constraints: None, cfg: (2.0, 2.0) };
    let made = k.make(&ask, Some(&v(&r, "text")), Some(&v(&r, "noise"))).unwrap();
    let want = v(&r, "posed_joints");
    let got: Vec<f32> = made.motion.joints77.iter().flatten().flatten().copied().collect();
    let cm: Vec<f32> = got.chunks(3).zip(want.chunks(3)).map(|(a, b)| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() * 100.0).collect();
    let (worst, mean) = (cm.iter().cloned().fold(0.0, f32::max), cm.iter().sum::<f32>() / cm.len() as f32);
    eprintln!("whole generation against PyTorch: joints {mean:.4} cm mean, {worst:.4} cm worst");
    assert!(worst < 0.05, "joints {worst} cm off at worst ({mean} mean)");
    let contacts = v(&r, "foot_contacts");
    let differ = made.motion.contacts.iter().flatten().zip(&contacts).filter(|(a, b)| **a != (**b > 0.5)).count();
    assert!(differ <= 2, "{differ} foot contacts differ");
}

#[test]
fn constraints_become_pytorchs_conditions() {
    let (Some(r), Some(dir)) = (fixture("cond_03.safetensors"), motion_dir()) else { return };
    let path = std::env::var("RAEA_KIMODO_REF").map(PathBuf::from).unwrap_or_else(|_| cache().join("ref/rust")).join("cond_03_constraints.json");
    let c: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let st = crate::stats::Stats::load(&dir).unwrap();
    let sk = crate::skeleton::Skeleton::soma().unwrap();
    let got = crate::constraints::conditions(&c, 150, &st, &sk).unwrap();
    let (mask, obs) = (v(&r, "mask"), v(&r, "obs"));
    assert_eq!(got.mask, mask, "the same numbers are constrained");
    let err = got.obs.iter().zip(&got.mask).zip(obs.iter().zip(&mask)).map(|((a, m), (b, n))| (a * m - b * n).abs()).fold(0.0, f32::max);
    assert!(err < 1e-4, "observed values {err} off (normalised units)");
}

/// `raea motion`'s chain through raea's models folder: RAEA_KIMODO_LINE (and RAEA_KIMODO_KEYS, a constraints file) to
/// the clip file sdk/body/tools/fromkimodo.ts reads, at RAEA_KIMODO_CLIP. Skipped unless RAEA_KIMODO_CLIP is set.
#[test]
fn a_line_becomes_a_clip_file() {
    let Ok(out) = std::env::var("RAEA_KIMODO_CLIP") else { return eprintln!("skipped: RAEA_KIMODO_CLIP unset") };
    exact();
    let line = std::env::var("RAEA_KIMODO_LINE").unwrap_or_else(|_| "a person walks forward and waves".into());
    let keys: Option<serde_json::Value> = std::env::var("RAEA_KIMODO_KEYS").ok().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
    let frames = std::env::var("RAEA_KIMODO_FRAMES").map(|f| f.parse().unwrap()).unwrap_or(150);
    let t0 = std::time::Instant::now();
    let mut k = crate::Kimodo::fetch(&mut |f, n| eprintln!("fetching {f}: {} MB", n >> 20)).unwrap();
    let (clean, emb) = k.encode(&line).unwrap();
    let loaded = t0.elapsed().as_secs_f64();
    let ask = crate::Ask { line: &line, frames, seed: 1, steps: 100, constraints: keys.as_ref(), cfg: (2.0, 2.0) };
    let made = k.make(&ask, emb.as_deref(), None).unwrap();
    let clip = crate::decode::clip_json(&made.motion, &k.skeleton, 1, 100, &clean, keys.is_some());
    std::fs::write(&out, clip.to_string()).unwrap();
    eprintln!("{clean:?}: {frames} frames in {:.1} s after {loaded:.1} s loading and encoding -> {out}", t0.elapsed().as_secs_f64() - loaded);
}

#[test]
fn the_8bit_text_encoder_matches_the_reference() {
    exact();
    let (Some(r), Some(dir)) = (fixture("text_walk_stop.safetensors"), text_dir()) else { return };
    let enc = crate::text::Encoder::load(&dir).unwrap();
    let (line, e) = enc.encode("a person walks forward and stops").unwrap();
    assert_eq!(line, "A person walks forward and stops.");
    let cos = |a: &[f32], b: &[f32]| {
        let (d, na, nb) = a.iter().zip(b).fold((0f64, 0f64, 0f64), |(d, na, nb), (x, y)| (d + (*x as f64) * (*y as f64), na + (*x as f64).powi(2), nb + (*y as f64).powi(2)));
        d / (na.sqrt() * nb.sqrt())
    };
    let (c8, c32) = (cos(&e, &v(&r, "q8_local")), cos(&e, &v(&r, "f32")));
    eprintln!("8-bit encoder: cosine {c8:.6} to the Python MLX 8-bit, {c32:.6} to float32");
    assert!(c8 > 0.99995, "cosine {c8} to the Python MLX 8-bit run");
    assert!(c32 > 0.9998, "cosine {c32} to the float32 encoder");
}
