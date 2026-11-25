//! Kimodo's two-stage denoiser on MLX (kimodo/model/twostage_denoiser.py, backbone.py): two torch
//! TransformerEncoders of the same shape (16 layers, width 1024, 8 heads, feed-forward 2048, GELU, norm after each
//! sublayer, no final norm). The root stage predicts the 5 global root numbers; they become 4 local root speeds; the
//! body stage predicts the other 364. Each stage sees [50 text tokens, the timestep, the first heading, the frames]
//! plus a fixed sin/cos table over the whole sequence. Weights by the checkpoint's own names.

use std::path::Path;

use anyhow::Result;
use mlx_rs::{Array, Dtype, fast, ops};
use raea_sys_mlx::nn::linear;
use raea_sys_mlx::weights::Weights;

use crate::decode::DIM;
use crate::stats::Stats;

pub const LATENT: i32 = 1024;
pub const HEADS: i32 = 8;
pub const LAYERS: usize = 16;
pub const TEXT_TOKENS: i32 = 50;
pub const TEXT_DIM: i32 = 4096;
pub const FPS: f32 = 30.0;
const ROOT_DIM: i32 = 5;
const P: &str = "denoiser.backbone.";

pub struct Net {
    pub w: Weights,
    pe: Array,
    mean_g: Array,
    std_g: Array,
    mean_l: Array,
    std_l: Array,
}

/// Kimodo's fixed sin/cos table, in f32 as PyTorch computes it.
fn pe_table(n: usize, d: usize) -> Vec<f32> {
    let mut pe = vec![0.0f32; n * d];
    let div: Vec<f32> = (0..d / 2).map(|k| 10000f32.powf(-((2 * k) as f32) / d as f32)).collect();
    for p in 0..n {
        for (k, dv) in div.iter().enumerate() {
            let a = p as f32 * dv;
            pe[p * d + 2 * k] = a.sin();
            pe[p * d + 2 * k + 1] = a.cos();
        }
    }
    pe
}

fn arr(v: &[f32]) -> Array {
    Array::from_slice(v, &[v.len() as i32])
}

/// frames start..end along `axis`
pub fn slice(a: &Array, axis: i32, start: i32, end: i32) -> Result<Array> {
    let idx: Vec<i32> = (start..end).collect();
    Ok(ops::indexing::take_axis(a, Array::from_slice(&idx, &[idx.len() as i32]), axis)?)
}

impl Net {
    pub fn load(ckpt: &Path, st: &Stats) -> Result<Self> {
        let w = Weights::load(&ckpt.join("model.safetensors"))?;
        w.eval()?;
        Ok(Net {
            w,
            pe: Array::from_slice(&pe_table(5000, LATENT as usize), &[5000, LATENT]),
            mean_g: arr(&st.mean_global_root),
            std_g: arr(&st.std_global_root),
            mean_l: arr(&st.mean_local_root),
            std_l: arr(&st.std_local_root),
        })
    }

    fn w(&self, k: &str) -> Result<&Array> {
        self.w.get(k)
    }

    /// torch.nn.TransformerEncoderLayer: norm after (norm_first false), GELU (erf), no dropout
    fn layer(&self, p: &str, x: &Array, mask: &Array) -> Result<Array> {
        let s = x.shape();
        let (b, l, d) = (s[0], s[1], s[2]);
        let qkv = ops::add(&x.matmul(&self.w(&format!("{p}.self_attn.in_proj_weight"))?.transpose()?)?, self.w(&format!("{p}.self_attn.in_proj_bias"))?)?;
        let parts = ops::split_equal(&qkv, 3, -1)?;
        let heads = |a: &Array| -> Result<Array> { Ok(a.reshape(&[b, l, HEADS, d / HEADS])?.transpose_axes(&[0, 2, 1, 3])?) };
        let (q, k, v) = (heads(&parts[0])?, heads(&parts[1])?, heads(&parts[2])?);
        let a = fast::scaled_dot_product_attention(&q, &k, &v, 1.0 / ((d / HEADS) as f32).sqrt(), mask, None)?;
        let a = linear(&self.w, &format!("{p}.self_attn.out_proj"), &a.transpose_axes(&[0, 2, 1, 3])?.reshape(&[b, l, d])?)?;
        let x = fast::layer_norm(&ops::add(x, &a)?, Some(self.w(&format!("{p}.norm1.weight"))?), Some(self.w(&format!("{p}.norm1.bias"))?), 1e-5)?;
        let f = linear(&self.w, &format!("{p}.linear2"), &mlx_rs::nn::gelu(linear(&self.w, &format!("{p}.linear1"), &x)?)?)?;
        Ok(fast::layer_norm(&ops::add(&x, &f)?, Some(self.w(&format!("{p}.norm2.weight"))?), Some(self.w(&format!("{p}.norm2.bias"))?), 1e-5)?)
    }

    /// kimodo.model.backbone.TransformerEncoderBlock. `keep` [B, T] is 1 on a clip's frames, 0 past its end;
    /// `t` [B] the timesteps; `heading` [B] the first frame's heading. All 50 text slots are attended (the checkpoint's
    /// use_text_mask is false), zero-padded past the text's own tokens.
    fn block(&self, p: &str, x: &Array, keep: &Array, text: &Array, t: &Array, heading: &Array) -> Result<Array> {
        let s = x.shape();
        let (b, tn) = (s[0], s[1]);
        let x = linear(&self.w, &format!("{p}.input_linear"), x)?;
        let n = text.shape()[1].min(TEXT_TOKENS);
        let text = slice(text, 1, 0, n)?;
        let text = if n < TEXT_TOKENS { ops::concatenate(&[text, ops::zeros::<f32>(&[b, TEXT_TOKENS - n, TEXT_DIM])?], 1)? } else { text };
        let e_text = linear(&self.w, &format!("{p}.embed_text"), &text)?;
        let pe_t = ops::indexing::take_axis(&self.pe, t, 0)?;
        let e_time = linear(&self.w, &format!("{p}.embed_timestep.time_embed.2"), &mlx_rs::nn::silu(linear(&self.w, &format!("{p}.embed_timestep.time_embed.0"), &pe_t)?)?)?;
        let e_head = linear(&self.w, &format!("{p}.linear_first_heading_angle"), &ops::stack(&[ops::cos(heading)?, ops::sin(heading)?], -1)?)?;
        let seq = ops::concatenate(&[e_text, e_time.expand_dims(1)?, e_head.expand_dims(1)?, x], 1)?;
        let len = seq.shape()[1];
        let pre = len - tn;
        let seq = ops::add(&seq, &slice(&self.pe, 0, 0, len)?.expand_dims(0)?)?;
        let keep_all = ops::concatenate(&[ops::ones::<f32>(&[b, pre])?, keep.clone()], 1)?;
        // additive: 0 where a key is kept, -1e9 past a clip's end
        let mask = ops::multiply(&ops::subtract(&keep_all, &Array::from_f32(1.0))?, &Array::from_f32(1e9))?.reshape(&[b, 1, 1, len])?;
        let mut seq = seq;
        for i in 0..LAYERS {
            seq = self.layer(&format!("{p}.seqTransEncoder.layers.{i}"), &seq, &mask)?;
        }
        linear(&self.w, &format!("{p}.output_linear"), &slice(&seq, 1, pre, len)?)
    }

    /// normalised [smooth root x y z, heading cos, sin] -> normalised [turn speed, x speed, z speed, height]; the speeds
    /// are differences times 30 with a zero appended, the last valid frame taking the one before it
    fn local_root(&self, root: &Array, lengths: &[i32]) -> Result<Array> {
        let root = ops::add(&ops::multiply(root, &self.std_g)?, &self.mean_g)?;
        let tn = root.shape()[1];
        let b = root.shape()[0];
        let pos = slice(&root, 2, 0, 3)?;
        let ang = ops::atan2(&slice(&root, 2, 4, 5)?, &slice(&root, 2, 3, 4)?)?;
        let (c, s) = (ops::cos(&ang)?, ops::sin(&ang)?);
        let (c1, c0, s1, s0) = (slice(&c, 1, 1, tn)?, slice(&c, 1, 0, tn - 1)?, slice(&s, 1, 1, tn)?, slice(&s, 1, 0, tn - 1)?);
        let d = ops::multiply(
            &ops::atan2(&ops::subtract(&ops::multiply(&s1, &c0)?, &ops::multiply(&c1, &s0)?)?, &ops::add(&ops::multiply(&c1, &c0)?, &ops::multiply(&s1, &s0)?)?)?,
            &Array::from_f32(FPS),
        )?;
        let vel = ops::multiply(&ops::subtract(&slice(&pos, 1, 1, tn)?, &slice(&pos, 1, 0, tn - 1)?)?, &Array::from_f32(FPS))?;
        let pad = |a: &Array| -> Result<Array> {
            let z = ops::zeros::<f32>(&[b, 1, a.shape()[2]])?;
            let a = ops::concatenate(&[a.clone(), z.clone()], 1)?;
            let prev = ops::concatenate(&[z, slice(&a, 1, 0, tn - 1)?], 1)?;
            // the last valid frame of each clip takes the frame before it
            let mut last = vec![0.0f32; (b * tn) as usize];
            for (i, &l) in lengths.iter().enumerate() {
                last[i * tn as usize + (l - 1) as usize] = 1.0;
            }
            let last = Array::from_slice(&last, &[b, tn, 1]);
            Ok(ops::add(&ops::multiply(&last, &prev)?, &ops::multiply(&ops::subtract(&Array::from_f32(1.0), &last)?, &a)?)?)
        };
        let (rot, vel) = (pad(&d)?, pad(&vel)?);
        let local = ops::concatenate(&[rot, slice(&vel, 2, 0, 1)?, slice(&vel, 2, 2, 3)?, slice(&pos, 2, 1, 2)?], 2)?;
        Ok(ops::divide(&ops::subtract(&local, &self.mean_l)?, &self.std_l)?)
    }

    /// The denoiser's clean-motion prediction for x [B, T, 369] at timesteps t, with conditions (mask, obs) or none.
    pub fn denoise(&self, x: &Array, lengths: &[i32], text: &Array, t: &Array, heading: &Array, cond: Option<(&Array, &Array)>) -> Result<Array> {
        let s = x.shape();
        let (b, tn) = (s[0], s[1]);
        let zeros;
        let (mask, obs) = match cond {
            Some(c) => c,
            None => {
                zeros = ops::zeros::<f32>(&[b, tn, DIM as i32])?;
                (&zeros, &zeros)
            }
        };
        let x = ops::add(&ops::multiply(x, &ops::subtract(&Array::from_f32(1.0), mask)?)?, &ops::multiply(obs, mask)?)?;
        let mut keep = vec![0.0f32; (b * tn) as usize];
        for (i, &l) in lengths.iter().enumerate() {
            for f in 0..l as usize {
                keep[i * tn as usize + f] = 1.0;
            }
        }
        let keep = Array::from_slice(&keep, &[b, tn]);
        let root = self.block(&format!("{P}root_model"), &ops::concatenate(&[x.clone(), mask.clone()], -1)?, &keep, text, t, heading)?;
        let local = self.local_root(&root, lengths)?;
        let body_in = ops::concatenate(&[local, slice(&x, 2, ROOT_DIM, DIM as i32)?, mask.clone()], -1)?;
        let body = self.block(&format!("{P}body_model"), &body_in, &keep, text, t, heading)?;
        Ok(ops::concatenate(&[root, body], -1)?)
    }
}

pub fn f32_array(a: &Array) -> Result<Vec<f32>> {
    let a = if a.dtype() == Dtype::Float32 { a.clone() } else { a.as_dtype(Dtype::Float32)? };
    let a = a.contiguous()?;
    a.eval()?;
    Ok(a.as_slice::<f32>().to_vec())
}
