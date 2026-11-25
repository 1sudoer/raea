//! SigLIP 2 B/16-256's text tower (docs/index.feature IX3): a query's words into the fast pass's space, on ONNX
//! Runtime's CPU kernels. Lower case, padded to 64 tokens, as SigLIP 2 was trained.

use anyhow::{Context, Result, anyhow};
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

const LEN: usize = 64;

pub struct Text {
    s: Session,
    tok: tokenizers::Tokenizer,
    pad: i64,
}

impl Text {
    /// From SigLIP 2's folder: onnx/text_model.onnx and tokenizer.json
    pub fn load(dir: &Path) -> Result<Text> {
        let s = raea_sys_ml::session(&dir.join("onnx/text_model.onnx")).context("SigLIP 2's text tower")?;
        let tok = tokenizers::Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow!("SigLIP 2's tokenizer: {e}"))?;
        Ok(Text { pad: tok.token_to_id("<pad>").map_or(0, i64::from), s, tok })
    }

    /// The words' vector, L2-normalised
    pub fn embed(&mut self, words: &str) -> Result<Vec<f32>> {
        let e = self.tok.encode(words.to_lowercase(), true).map_err(|e| anyhow!("tokenizing {words:?}: {e}"))?;
        let mut ids: Vec<i64> = e.get_ids().iter().map(|&i| i as i64).take(LEN).collect();
        ids.resize(LEN, self.pad);
        let input = Tensor::from_array((vec![1i64, LEN as i64], ids))?;
        let out = self.s.run(ort::inputs!["input_ids" => input])?;
        let (_, d) = out.get("pooler_output").ok_or_else(|| anyhow!("no pooler_output"))?.try_extract_tensor::<f32>()?;
        let mut v = d.to_vec();
        crate::normalise(&mut v);
        Ok(v)
    }
}
