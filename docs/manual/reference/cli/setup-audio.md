---
title: "raea setup-audio"
summary: "Install or check the local audio models: Stable Audio Open for foley and TokenSynth for instruments."
order: 86
sources: ["apps/tools/src/sound.rs"]
---

`raea sao` and `raea tokensynth` install their models the first time they're used. `raea setup-audio` does it on
purpose, ahead of time, and says where each lives and how much disk it takes. Name one model to set up only that one.

```sh
raea setup-audio
raea setup-audio --check
raea setup-audio tokensynth
```

Each model's setup (`share/raea/tools/sao/setup.py` and `share/raea/tools/tokensynth/setup.py` beside an installed
raea, each with a `setup.sh` that runs it on a Mac or Linux) installs it into `~/.cache/raea-sao` or
`~/.cache/raea-tokensynth` (`RAEA_SAO` and `RAEA_TOKENSYNTH` move them):

- **SAO, about 2 GB:** on a Mac, Stable Audio Open Small's Core ML models from a Hugging Face mirror at a pinned
  revision, each file checked against its SHA-256, t5-base's tokenizer, a small Python environment, and a T5 encoder
  converted once. On Windows and Linux, the same weights as three ONNX graphs (each checked against its SHA-256) on
  ONNX Runtime: with CUDA where `nvidia-smi` finds an NVIDIA GPU (its CUDA libraries add a few GB; `RAEA_SAO_CUDA=0`
  keeps the CPU build), else on the CPU.
  Its licence is Stability AI's Community License
  ([stability.ai/community-license-agreement](https://stability.ai/community-license-agreement)), not an open one.
  As between you and Stability, you own the sounds it makes, as far as the law allows; they may not be used to
  create or improve another generative AI model, and Stability's acceptable use policy applies to them. What else
  the licence asks, of raea and of a commercial user, raea hasn't written down yet: read it before an SAO sound goes
  into a commercial piece.
- **TokenSynth, about 6 GB:** its code from GitHub at a pinned commit (an archive: no git), a Python environment
  with torch, and its weights, DAC and CLAP from Hugging Face, then a converted fp16 runtime (and, on a Mac, Core ML
  models for `--precision coreml`). MIT.

Both need only a network connection, on a Mac, Windows or Linux: no Homebrew, git or FFmpeg. raea runs each setup
through [uv](https://docs.astral.sh/uv/), the one on the PATH or else its own (uv 0.12.24, fetched once into
`~/.cache/raea-tools/uv` and checked against its SHA-256; `RAEA_TOOLS_CACHE` moves it), and uv brings the Python.

When a setup fails, the error names the command: run it yourself to see why (`sh share/raea/tools/sao/setup.sh` on
a Mac or Linux; on Windows `uv run --no-project --python 3.11 python -I share\raea\tools\sao\setup.py`, with uv
from the PATH or `%USERPROFILE%\.cache\raea-tools\uv`). Exit status 75 is another setup running in the same folder.
`RAEA_OFFLINE` refuses the download. `RAEA_AUDIO_AUTOSETUP=0` stops a render from installing a model itself, and
stops `raea setup-audio` and `ensure.sh` too: with it set, run the model's setup yourself.

<!-- generated:begin by tools/manual-ref/cli.ts from `raea help --json`; edit only the flag descriptions -->

## Usage

```sh
raea setup-audio [--check] [sao|tokensynth]   (install or check the local audio models)
```

Headless: it needs nothing running and opens no window. Takes up to 1 argument besides its flags.

## Flags

| Flag | Takes | What it does |
| --- | --- | --- |
| `--check` | – | Say whether each model is installed, where it lives and how much disk it takes, without installing anything. |

<!-- generated:end -->
