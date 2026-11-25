//! Kimodo on PyTorch, for the machines MLX doesn't run on (Linux and Windows): upstream nv-tlabs/kimodo at the commit
//! the MLX port was checked against, in a Python environment raea makes in its models folder (`raea setup motion`),
//! with the same Kimodo-SOMA-RP-v1 checkpoint and LLM2Vec on Llama 3 8B Instruct (in bf16 here, 8 bits on MLX). The
//! runner (torch/run.py, kept in this binary) hands back the sampler's features, and decode.rs turns them into the
//! clip the MLX path writes, so the two backends write one file. Where the networks run is the runner's call: the
//! denoiser on CUDA when there is an NVIDIA GPU, else the CPU; the text encoder beside it on a GPU of 18 GB or more,
//! else the CPU.
//!
//! Plain Rust (it runs a program), so it builds everywhere; `raea motion` uses it where the networks aren't on MLX.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// upstream Kimodo, at the commit the MLX port was checked against (tests.rs)
pub const KIMODO: &str = "e1c219c99b4b94ba48be1fb6a1cbc64ecee4c954";
/// what the environment's Python runs: written beside the environment before each use, so it is always this binary's
pub const RUNNER: &str = include_str!("../torch/run.py");
/// The environment's packages: what generation imports, at upstream's own pins (docker_requirements.txt at KIMODO,
/// compiled by uv; the cloud runner installed that list on 11-15), and Kimodo at KIMODO with what it brings. PyTorch is
/// left to uv's pick for the machine (--torch-backend auto: its CUDA build for the NVIDIA driver there, else CPU only),
/// as upstream leaves it to its container. Not yet installed by `raea setup motion` on a machine (11-17: the CUDA box's
/// link fell to 1.5 MB/s mid-install), so this list is untested as a whole.
pub const PACKAGES: &[&str] = &[
    "torch>=2.6,<3",
    "transformers==5.1.0",
    "peft==0.18.1",
    "huggingface-hub==1.6.0",
    "safetensors==0.7.0",
    "tokenizers==0.22.2",
    "numpy==1.26.4",
    "scipy==1.15.3",
    "hydra-core==1.3.2",
    "omegaconf==2.3.0",
    "einops==0.8.2",
    "kimodo @ git+https://github.com/nv-tlabs/kimodo@e1c219c99b4b94ba48be1fb6a1cbc64ecee4c954",
];

/// A file the environment needs, and what it is (for saying what's missing).
const NEEDS: &[(&str, &str)] = &[
    ("checkpoints/Kimodo-SOMA-RP-v1/model.safetensors", "the motion model"),
    ("checkpoints/Kimodo-SOMA-RP-v1/config.yaml", "the motion model"),
    ("checkpoints/Kimodo-SOMA-RP-v1/stats/motion/body/std.npy", "the motion model"),
    ("text/McGill-NLP/LLM2Vec-Meta-Llama-3-8B-Instruct-mntp/adapter_model.safetensors", "the text encoder"),
    ("text/McGill-NLP/LLM2Vec-Meta-Llama-3-8B-Instruct-mntp-supervised/adapter_model.safetensors", "the text encoder"),
    ("text/meta-llama/Meta-Llama-3-8B-Instruct/.sha256-checked", "the text encoder"),
];

/// Kimodo's PyTorch environment in a folder (raea's models folder's kimodo-torch).
pub struct Torch {
    pub dir: PathBuf,
    /// the Python environment
    pub venv: PathBuf,
    /// the motion checkpoint's folder (its stats are decode.rs's too)
    pub checkpoint: PathBuf,
    /// the environment's record of what it was made from: a different list makes it again
    made_from: PathBuf,
}

/// One clip asked of PyTorch.
pub struct Job<'a> {
    pub line: &'a str,
    pub frames: usize,
    pub seed: u64,
    pub steps: usize,
    /// a Kimodo constraints file (keyframes, end effectors, a root path), or none
    pub keys: Option<&'a Path>,
    pub cfg: (f32, f32),
}

pub struct Made {
    /// frames x 369, normalised, as the sampler leaves them (decode.rs reads them)
    pub features: Vec<f32>,
    /// the runner's account: the line as Kimodo read it, where each network ran, the seconds, PyTorch's own root
    pub info: serde_json::Value,
}

impl Torch {
    pub fn at(dir: PathBuf) -> Self {
        let venv = dir.join("venv");
        Torch { made_from: venv.join("raea-packages.txt"), venv, checkpoint: dir.join("checkpoints").join("Kimodo-SOMA-RP-v1"), dir }
    }

    pub fn python(&self) -> PathBuf {
        if cfg!(windows) { self.venv.join("Scripts").join("python.exe") } else { self.venv.join("bin").join("python") }
    }

    fn packages() -> String {
        PACKAGES.join("\n")
    }

    /// Whether the Python environment is made from this binary's package list.
    pub fn has_env(&self) -> bool {
        self.python().exists() && std::fs::read_to_string(&self.made_from).is_ok_and(|s| s == Self::packages())
    }

    /// What isn't there yet, by name: the Python environment, the motion model, the text encoder. Empty when it's all set up.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.has_env() {
            out.push("Python environment");
        }
        for (f, what) in NEEDS {
            if !self.dir.join(f).exists() && !out.contains(what) {
                out.push(what);
            }
        }
        out
    }

    /// The runner, written beside the environment when it differs from this binary's.
    fn runner(&self) -> Result<PathBuf> {
        let p = self.dir.join("run.py");
        if std::fs::read_to_string(&p).ok().as_deref() != Some(RUNNER) {
            std::fs::create_dir_all(&self.dir)?;
            std::fs::write(&p, RUNNER).with_context(|| format!("can't write {}", p.display()))?;
        }
        Ok(p)
    }

    /// The Python environment (made by uv: Python 3.11, PyTorch with CUDA where there is an NVIDIA driver, Kimodo),
    /// then the weights; each step only when it's missing. `say` gets a line a step; what was made is returned.
    pub fn setup(&self, say: &mut dyn FnMut(&str)) -> Result<Vec<String>> {
        let mut made = Vec::new();
        std::fs::create_dir_all(&self.dir).with_context(|| format!("can't make {}", self.dir.display()))?;
        if !self.has_env() {
            let uv = Command::new("uv").arg("--version").output();
            if uv.as_ref().map_or(true, |o| !o.status.success()) {
                bail!(
                    "raea setup motion makes its Python environment with uv, which isn't on PATH: install it (https://docs.astral.sh/uv; Linux and macOS: curl -LsSf https://astral.sh/uv/install.sh | sh; Windows: winget install astral-sh.uv), then run raea setup motion again"
                );
            }
            let venv = self.venv.clone();
            let _ = std::fs::remove_dir_all(&venv);
            say(&format!("making the Python environment (Python 3.11 by uv) in {}", venv.display()));
            run(Command::new("uv").args(["venv", "--quiet", "--python", "3.11"]).arg(&venv), "uv venv")?;
            say("installing PyTorch (CUDA where there is an NVIDIA driver, else CPU only) and Kimodo: a few GB the first time");
            let mut pip = Command::new("uv");
            pip.args(["pip", "install", "--python"]).arg(self.python()).args(["--torch-backend", "auto"]).args(PACKAGES);
            // Kimodo's own post-processing (a C++ extension built with CMake) isn't used: clips are post_processing=False
            pip.env("SKIP_MOTION_CORRECTION_IN_SETUP", "1");
            if let Err(e) = run(&mut pip, "uv pip install") {
                let _ = std::fs::remove_dir_all(&venv);
                return Err(e);
            }
            std::fs::write(&self.made_from, Self::packages())?;
            made.push(format!("made the Python environment (Python 3.11, PyTorch, Kimodo at {}): {}", &KIMODO[..7], venv.display()));
        }
        if self.missing().iter().any(|m| *m != "Python environment") {
            say("fetching the weights: Kimodo-SOMA-RP-v1 (1.1 GB), LLM2Vec's adapters (0.3 GB) and Llama 3 8B Instruct in bf16 (16 GB)");
            let runner = self.runner()?;
            run(Command::new(self.python()).arg("-I").arg(&runner).arg("fetch").arg(&self.dir), "fetching the weights")?;
            made.push(format!("fetched the weights: {}", self.dir.display()));
        }
        Ok(made)
    }

    /// What this machine has and where each network would run (the runner's probe: torch, cuda, gpu, vram_gb, ram_gb,
    /// motion, text, threads).
    pub fn probe(&self) -> Result<serde_json::Value> {
        let runner = self.runner()?;
        let o = Command::new(self.python()).arg("-I").arg(&runner).arg("probe").arg(&self.dir).stderr(Stdio::inherit()).output().context("can't run the environment's Python")?;
        if !o.status.success() {
            bail!("the environment's probe failed ({})", o.status);
        }
        last_json(&o.stdout)
    }

    /// One clip's features from PyTorch. The runner's progress goes to stderr as it comes.
    pub fn make(&self, job: &Job) -> Result<Made> {
        let runner = self.runner()?;
        let tmp = std::env::temp_dir().join(format!("raea-kimodo-{}", std::process::id()));
        std::fs::create_dir_all(&tmp)?;
        let (ask, out) = (tmp.join("ask.json"), tmp.join("features.f32"));
        let keys = job.keys.map(|k| dunce::canonicalize(k).unwrap_or_else(|_| k.to_path_buf()));
        let a = serde_json::json!({"line": job.line, "frames": job.frames, "seed": job.seed, "steps": job.steps, "keys": keys, "cfg": [job.cfg.0, job.cfg.1]});
        std::fs::write(&ask, a.to_string())?;
        let o = Command::new(self.python()).arg("-I").arg(&runner).arg("make").arg(&self.dir).arg(&ask).arg(&out).stderr(Stdio::inherit()).output().context("can't run the environment's Python");
        let read = std::fs::read(&out);
        let _ = std::fs::remove_dir_all(&tmp);
        let o = o?;
        if !o.status.success() {
            bail!("Kimodo on PyTorch failed ({}); what it said is above", o.status);
        }
        let b = read.context("Kimodo on PyTorch wrote no features")?;
        let features: Vec<f32> = b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        if features.len() != job.frames * crate::decode::DIM {
            bail!("Kimodo on PyTorch gave {} numbers, not {} frames of {}", features.len(), job.frames, crate::decode::DIM);
        }
        Ok(Made { features, info: last_json(&o.stdout)? })
    }
}

/// A program run with its output going to the person as it comes; an error names it.
fn run(c: &mut Command, what: &str) -> Result<()> {
    let s = c.stdin(Stdio::null()).status().with_context(|| format!("can't run {what}"))?;
    if !s.success() {
        bail!("{what} failed ({s}); what it said is above");
    }
    Ok(())
}

/// the last line of a program's output that is a JSON object
fn last_json(out: &[u8]) -> Result<serde_json::Value> {
    String::from_utf8_lossy(out).lines().rev().find_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok().filter(|v| v.is_object())).context("the runner's answer had no JSON line")
}

/// The largest distance, in metres, between PyTorch's own decoded root and decode.rs's, frame by frame: a check that
/// the features were read as Kimodo wrote them.
pub fn root_gap(torch_root: &serde_json::Value, ours: &[[f32; 3]]) -> Option<f64> {
    let r = torch_root.as_array()?;
    (r.len() == ours.len()).then_some(())?;
    r.iter().zip(ours).map(|(a, b)| {
        let a: Vec<f64> = a.as_array().into_iter().flatten().filter_map(serde_json::Value::as_f64).collect();
        (0..3).map(|i| (a.get(i).copied().unwrap_or(f64::NAN) - b[i] as f64).powi(2)).sum::<f64>().sqrt()
    }).try_fold(0.0f64, |m, d| d.is_finite().then_some(m.max(d)))
}
