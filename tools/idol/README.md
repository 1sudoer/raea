# IDOL (speridlabs/idol): image to 3D Gaussian splat, the ONNX work so far

Parked 2026-11-10 before any export. What is here runs on Modal, because the weights and exports need about 20 GB.

- `idol_modal.py`: one step script on a GPU, with an idol-sdk checkout (`IDOL_SDK`) and the volume `idol-work`
  (`/vol/hf` the Hugging Face cache, `/vol/ref` the reference dumps, `/vol/onnx` for exports).
- `steps/common.py`: the ungated encoder sources and `CpuRandom`, which draws every random of a run from one CPU
  generator so a run is the same on any device.
- `steps/s1_sources.py`: proves the two sources, runs the PyTorch reference (toy-robot-tin, seed 42, 262,144
  Gaussians, 50 steps) and saves each network's inputs and outputs as `.npy` with a `shapes.txt`.

## Measured

| Check | Result |
|---|---|
| FLUX.2 VAE: FLUX.2-klein-4B `vae/` (bf16) against FLUX.2-dev's (fp32, from diffusers/FLUX.2-dev-bnb-4bit) | 251 of 251 tensors equal after the bf16 cast, 84.0M parameters |
| DINOv3 ViT-H+/16 from timm against the gated transformers model (its public ONNX conversion, onnx-community/dinov3-vith16plus-pretrain-lvd1689m-ONNX), tokens at 1024x1024 | cosine 0.99999979, max abs difference 0.025 (tokens reach 10) |
| timm's forward against timm's weights loaded into transformers' `DINOv3ViTModel` | cosine 1.00000000, max abs difference 2.6e-5 |
| PyTorch reference, fp32 and bf16 DINOv3 | peak VRAM 8.72 GB; 20 s on the first GPU drawn, 59 s on the second |

The transformers config for H+ is `hidden_act="silu"`, gated MLP, 32 layers, 20 heads, 4 registers, rope_theta 100.
With the default GELU the tokens fall to cosine 0.95. timm's file has no q/k/v biases, and the conversion has none
either. The key map is in `s1_sources.py`.
