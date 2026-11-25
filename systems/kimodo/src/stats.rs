//! The checkpoint's feature statistics (stats/motion/{global_root,local_root,body}/{mean,std}.npy, float64), each
//! number normalised as (x - mean) / sqrt(std^2 + 1e-5) in f32, as Kimodo does.

use std::path::Path;

use anyhow::{Context, Result, bail};

pub const EPS: f32 = 1e-5;

pub struct Stats {
    /// the 369 feature numbers: the global root's 5, then the body's 364
    pub mean: Vec<f32>,
    pub std: Vec<f32>,
    pub mean_global_root: Vec<f32>,
    pub std_global_root: Vec<f32>,
    pub mean_local_root: Vec<f32>,
    pub std_local_root: Vec<f32>,
}

/// A 1-D little-endian float64 or float32 .npy file (format 1.0 or 2.0) as f32s.
pub fn npy(path: &Path) -> Result<Vec<f32>> {
    let b = std::fs::read(path).with_context(|| format!("can't read {}", path.display()))?;
    if b.len() < 10 || &b[..6] != b"\x93NUMPY" {
        bail!("{}: not a .npy file", path.display());
    }
    let (hlen, start) = if b[6] == 1 { (u16::from_le_bytes([b[8], b[9]]) as usize, 10) } else { (u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize, 12) };
    let header = std::str::from_utf8(&b[start..start + hlen])?;
    let data = &b[start + hlen..];
    if header.contains("'fortran_order': True") {
        bail!("{}: Fortran order", path.display());
    }
    Ok(if header.contains("'<f8'") {
        words::<8>(data).map(f64::from_le_bytes).map(|v| v as f32).collect()
    } else if header.contains("'<f4'") {
        words::<4>(data).map(f32::from_le_bytes).collect()
    } else {
        bail!("{}: not little-endian float32 or float64 ({header})", path.display())
    })
}

impl Stats {
    /// From the checkpoint folder (its stats/motion/...).
    pub fn load(ckpt: &Path) -> Result<Self> {
        let part = |p: &str| -> Result<(Vec<f32>, Vec<f32>)> {
            let d = ckpt.join("stats/motion").join(p);
            let mean = npy(&d.join("mean.npy"))?;
            let std = npy(&d.join("std.npy"))?.into_iter().map(|s| (s * s + EPS).sqrt()).collect();
            Ok((mean, std))
        };
        let (mg, sg) = part("global_root")?;
        let (ml, sl) = part("local_root")?;
        let (mb, sb) = part("body")?;
        anyhow::ensure!(mg.len() == 5 && ml.len() == 4 && mb.len() == 364, "kimodo: the feature statistics have the wrong sizes");
        Ok(Stats {
            mean: mg.iter().chain(&mb).copied().collect(),
            std: sg.iter().chain(&sb).copied().collect(),
            mean_global_root: mg,
            std_global_root: sg,
            mean_local_root: ml,
            std_local_root: sl,
        })
    }
}

/// little-endian words of N bytes
fn words<const N: usize>(data: &[u8]) -> impl Iterator<Item = [u8; N]> + '_ {
    data.chunks_exact(N).map(|c| <[u8; N]>::try_from(c).unwrap())
}
