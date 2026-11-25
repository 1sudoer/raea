//! Kimodo's text encoder on MLX: LLM2Vec (McGill-NLP) on Llama 3 8B Instruct, 8-bit (mlx-community's, group 64), its
//! two LoRA adapters (mntp, then supervised; rank 16, alpha 32, on every projection) run beside the quantised base in
//! f32. Every token sees every token (LLM2Vec's bidirectional attention); the final norm's outputs are averaged over
//! the text's own tokens and <|eot_id|>. 4096 numbers a line.
//!
//! Measured 11-16 against the float32 encoder on eight lines: cosine 0.99989 to 0.99993; with the same noise the
//! motion's joints move 0.01 to 0.74 cm on average (1.44 cm at worst, a run and leap) over 5 s. 4-bit moved them 8 cm.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mlx_rs::{Array, Dtype, fast, ops};
use mlx_rs::ops::{QuantizedMatmulOptions, quantized_matmul};
use raea_sys_mlx::weights::Weights;

const LAYERS: usize = 32;
const HEADS: i32 = 32;
const KV_HEADS: i32 = 8;
const HEAD_DIM: i32 = 128;
const THETA: f32 = 500000.0;
const EPS: f32 = 1e-5;
const LORA_SCALE: f32 = 32.0 / 16.0;
const PROJ: [&str; 7] = ["self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj", "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj"];

pub struct Encoder {
    w: Weights,
    /// per (layer, projection): each adapter's A [16, in] and B [out, 16]
    lora: HashMap<(usize, &'static str), Vec<(Array, Array)>>,
    tok: tokenizers::Tokenizer,
    group: i32,
    bits: i32,
}

/// kimodo.sanitize.sanitize_text (paragraph=True): one space between words, from the first letter or digit, the
/// first letter capitalised and the rest lowered, ended with a full stop, each sentence capitalised.
pub fn sanitize(text: &str) -> String {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return t;
    }
    let chars: Vec<char> = t.chars().collect();
    let start = chars.iter().position(|c| c.is_alphanumeric()).unwrap_or(chars.len() - 1);
    let rest: String = chars[start..].iter().collect();
    let mut it = rest.chars();
    let mut t: String = match it.next() {
        Some(f) => f.to_uppercase().chain(it.as_str().to_lowercase().chars()).collect(),
        None => String::new(),
    };
    let chars: Vec<char> = t.chars().collect();
    if let Some(end) = chars.iter().rposition(|c| c.is_alphanumeric() || matches!(c, '.' | '!' | '?' | '"' | ']' | ')' | '\'')) {
        t = chars[..=end].iter().collect();
    }
    if !t.ends_with(['.', '!', '?']) {
        t.push('.');
    }
    for brk in ['.', '!', '?'] {
        let parts: Vec<String> = t
            .split(brk)
            .map(|x| {
                let y = x.trim();
                let mut c = y.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().chain(c).collect(),
                    None => String::new(),
                }
            })
            .collect();
        t = parts.join(&format!("{brk} ")).trim().to_string();
    }
    t
}

fn rms(x: &Array, w: &Array) -> Result<Array> {
    Ok(fast::rms_norm(x, Some(&w.as_dtype(Dtype::Float32)?), EPS)?)
}

impl Encoder {
    /// From the model folder: the 8-bit Llama's config.json and model*.safetensors, llm2vec/{mntp,supervised}.safetensors
    /// and llm2vec/tokenizer.json.
    pub fn load(dir: &Path) -> Result<Self> {
        let q: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("config.json")).context("kimodo text: no config.json")?)?;
        let (group, bits) = (q["quantization"]["group_size"].as_i64().unwrap_or(64) as i32, q["quantization"]["bits"].as_i64().unwrap_or(8) as i32);
        let mut shards: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.starts_with("model") && n.ends_with(".safetensors")
        }).collect();
        shards.sort();
        anyhow::ensure!(!shards.is_empty(), "kimodo text: no model*.safetensors in {}", dir.display());
        let mut all = Vec::new();
        for s in &shards {
            let m = Array::load_safetensors(s).map_err(|e| anyhow!("kimodo text: can't read {}: {e:?}", s.display()))?;
            all.extend(m.into_iter().filter(|(k, _)| !k.starts_with("lm_head")));
        }
        let w: Weights = all.into_iter().collect();
        let mut lora: HashMap<(usize, &'static str), Vec<(Array, Array)>> = HashMap::new();
        for f in ["mntp", "supervised"] {
            let a = Weights::load(&dir.join("llm2vec").join(format!("{f}.safetensors")))?;
            for i in 0..LAYERS {
                for p in PROJ {
                    let k = format!("base_model.model.layers.{i}.{p}");
                    let (la, lb) = (a.get(&format!("{k}.lora_A.weight"))?.as_dtype(Dtype::Float32)?, a.get(&format!("{k}.lora_B.weight"))?.as_dtype(Dtype::Float32)?);
                    lora.entry((i, p)).or_default().push((la.transpose()?, lb.transpose()?));
                }
            }
        }
        let tok = tokenizers::Tokenizer::from_file(dir.join("llm2vec/tokenizer.json")).map_err(|e| anyhow!("kimodo text: tokenizer: {e}"))?;
        Ok(Encoder { w, lora, tok, group, bits })
    }

    fn lin(&self, i: usize, p: &'static str, h: &Array) -> Result<Array> {
        let k = format!("model.layers.{i}.{p}");
        let (scales, biases) = (self.w.get(&format!("{k}.scales"))?.as_dtype(Dtype::Float32)?, self.w.get(&format!("{k}.biases"))?.as_dtype(Dtype::Float32)?);
        let opts = QuantizedMatmulOptions { transpose: true, group_size: Some(self.group), bits: Some(self.bits), ..Default::default() };
        let mut y = quantized_matmul(h, self.w.get(&format!("{k}.weight"))?, &scales, &biases, opts).map_err(|e| anyhow!("kimodo text: {e:?}"))?;
        for (a, b) in &self.lora[&(i, p)] {
            y = ops::add(&y, &ops::multiply(&h.matmul(a)?.matmul(b)?, &Array::from_f32(LORA_SCALE))?)?;
        }
        Ok(y)
    }

    /// A line's 4096 numbers (and the line as sanitised). An empty line is all zeros, as Kimodo's is.
    pub fn encode(&self, line: &str) -> Result<(String, Vec<f32>)> {
        let t = sanitize(line);
        if t.is_empty() {
            return Ok((t, vec![0.0; 4096]));
        }
        let ids = self.tok.encode(format!("<|start_header_id|>user<|end_header_id|>\n\n{t}<|eot_id|>"), true).map_err(|e| anyhow!("{e}"))?.get_ids().to_vec();
        let n = self.tok.encode(format!("{t}<|eot_id|>"), false).map_err(|e| anyhow!("{e}"))?.get_ids().len() as i32;
        let l = ids.len() as i32;
        let idx = Array::from_slice(&ids.iter().map(|&i| i as i32).collect::<Vec<_>>(), &[l]);
        let mut x = ops::indexing::take_axis(self.w.get("model.embed_tokens.weight")?, &idx, 0)?.as_dtype(Dtype::Float32)?.expand_dims(0)?;
        // rotary tables as the reference computes them: f32, the two halves alike
        let inv: Vec<f32> = (0..HEAD_DIM / 2).map(|k| 1.0 / THETA.powf((2 * k) as f32 / HEAD_DIM as f32)).collect();
        let mut f = vec![0.0f32; (l * HEAD_DIM) as usize];
        for p in 0..l as usize {
            for k in 0..(HEAD_DIM / 2) as usize {
                let a = p as f32 * inv[k];
                f[p * HEAD_DIM as usize + k] = a;
                f[p * HEAD_DIM as usize + k + (HEAD_DIM / 2) as usize] = a;
            }
        }
        let cos = Array::from_slice(&f.iter().map(|a| a.cos()).collect::<Vec<_>>(), &[1, 1, l, HEAD_DIM]);
        let sin = Array::from_slice(&f.iter().map(|a| a.sin()).collect::<Vec<_>>(), &[1, 1, l, HEAD_DIM]);
        let rope = |a: &Array| -> Result<Array> {
            let half = HEAD_DIM / 2;
            let (a1, a2) = (crate::net::slice(a, 3, 0, half)?, crate::net::slice(a, 3, half, HEAD_DIM)?);
            let rot = ops::concatenate(&[ops::negative(&a2)?, a1], 3)?;
            Ok(ops::add(&ops::multiply(a, &cos)?, &ops::multiply(&rot, &sin)?)?)
        };
        for i in 0..LAYERS {
            let h = rms(&x, self.w.get(&format!("model.layers.{i}.input_layernorm.weight"))?)?;
            let heads = |a: Array, n: i32| -> Result<Array> { Ok(a.reshape(&[1, l, n, HEAD_DIM])?.transpose_axes(&[0, 2, 1, 3])?) };
            let q = rope(&heads(self.lin(i, "self_attn.q_proj", &h)?, HEADS)?)?;
            let k = rope(&heads(self.lin(i, "self_attn.k_proj", &h)?, KV_HEADS)?)?;
            let v = heads(self.lin(i, "self_attn.v_proj", &h)?, KV_HEADS)?;
            // no mask: every token sees every token; MLX's kernel shares each key head among four query heads
            let a = fast::scaled_dot_product_attention(&q, &k, &v, 1.0 / (HEAD_DIM as f32).sqrt(), None, None)?;
            let a = a.transpose_axes(&[0, 2, 1, 3])?.reshape(&[1, l, HEADS * HEAD_DIM])?;
            x = ops::add(&x, &self.lin(i, "self_attn.o_proj", &a)?)?;
            let h = rms(&x, self.w.get(&format!("model.layers.{i}.post_attention_layernorm.weight"))?)?;
            let g = self.lin(i, "mlp.gate_proj", &h)?;
            let up = self.lin(i, "mlp.up_proj", &h)?;
            x = ops::add(&x, &self.lin(i, "mlp.down_proj", &ops::multiply(&mlx_rs::nn::silu(&g)?, &up)?)?)?;
            x.eval()?;
        }
        let x = rms(&x, self.w.get("model.norm.weight")?)?;
        let mean = crate::net::slice(&x, 1, l - n, l)?.mean_axis(1, false)?;
        Ok((t, crate::net::f32_array(&mean)?))
    }
}
