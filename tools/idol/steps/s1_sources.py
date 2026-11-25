"""The two ungated encoder sources are the weights IDOL expects (FLUX.2 VAE from klein, DINOv3 from timm); then the
PyTorch reference on one image and seed, with every network's inputs and outputs saved to /vol/ref."""
import os, sys, time
import numpy as np
import torch
from PIL import Image
from huggingface_hub import hf_hub_download
from safetensors.torch import load_file
from common import *

dev = torch.device("cuda")
img_path = "/sdk/demo/examples/toy-robot-tin.webp"

# --- FLUX.2 VAE: klein's file against FLUX.2-dev's (an ungated diffusers-format copy), tensor by tensor
a = load_file(hf_hub_download(KLEIN, "vae/diffusion_pytorch_model.safetensors"))
b = load_file(hf_hub_download("diffusers/FLUX.2-dev-bnb-4bit", "vae/diffusion_pytorch_model.safetensors"))
assert a.keys() == b.keys(), (set(a) ^ set(b))
K = "encoder.conv_in.weight"
exact = sum(torch.equal(a[k], b[k].to(a[k].dtype)) for k in a)
worst = max(float((a[k].float() - b[k].float()).abs().max()) for k in a)
print(f"VAE: {len(a)} tensors, klein dtype {a[K].dtype}, dev dtype {b[K].dtype}, "
      f"equal after cast {exact}/{len(a)}, max abs diff {worst:.3e}, params {sum(v.numel() for v in a.values())/1e6:.1f}M")
del a, b

# --- DINOv3: timm's weights in transformers' DINOv3ViTModel, against timm's own forward
from transformers import DINOv3ViTConfig, DINOv3ViTModel
sd = load_file(hf_hub_download(TIMM, "model.safetensors"))
cfg = DINOv3ViTConfig(patch_size=16, hidden_size=1280, intermediate_size=5120, num_hidden_layers=32,
                      num_attention_heads=20, num_register_tokens=4, use_gated_mlp=True, hidden_act="silu", layerscale_value=1.0,
                      query_bias=False, key_bias=False, value_bias=False, proj_bias=True, mlp_bias=True,
                      rope_theta=100.0, pos_embed_rescale=2.0, layer_norm_eps=1e-5, image_size=224)
hf = DINOv3ViTModel(cfg).eval().requires_grad_(False)
want = hf.state_dict()
print("hf keys:", len(want), [k for k in want if ".layer." not in k and not k.startswith("layer.")])
new = {"embeddings.cls_token": sd["cls_token"], "embeddings.register_tokens": sd["reg_token"],
       "embeddings.mask_token": torch.zeros_like(want["embeddings.mask_token"]),
       "embeddings.patch_embeddings.weight": sd["patch_embed.proj.weight"],
       "embeddings.patch_embeddings.bias": sd["patch_embed.proj.bias"],
       "norm.weight": sd["norm.weight"], "norm.bias": sd["norm.bias"]}
pre = "model." if any(k.startswith("model.layer.") for k in want) else ""
for i in range(32):
    p, q = f"blocks.{i}.", f"{pre}layer.{i}."
    w = sd[p + "attn.qkv.weight"]
    for j, n in enumerate("qkv"):
        new[q + f"attention.{n}_proj.weight"] = w[j * 1280:(j + 1) * 1280]
    new[q + "attention.o_proj.weight"], new[q + "attention.o_proj.bias"] = sd[p + "attn.proj.weight"], sd[p + "attn.proj.bias"]
    new[q + "layer_scale1.lambda1"], new[q + "layer_scale2.lambda1"] = sd[p + "gamma_1"], sd[p + "gamma_2"]
    for n in ("norm1", "norm2"):
        new[q + n + ".weight"], new[q + n + ".bias"] = sd[p + n + ".weight"], sd[p + n + ".bias"]
    # timm's SwiGLU: fc1_g is the gate (activated), fc1_x the value; transformers: gate_proj (activated) * up_proj
    for s, d in (("fc1_g", "gate_proj"), ("fc1_x", "up_proj"), ("fc2", "down_proj")):
        new[q + f"mlp.{d}.weight"], new[q + f"mlp.{d}.bias"] = sd[p + f"mlp.{s}.weight"], sd[p + f"mlp.{s}.bias"]
missing = set(want) - set(new); extra = set(new) - set(want)
print("missing", sorted(missing)[:10], "extra", sorted(extra)[:10])
hf.load_state_dict(new, strict=True)
print(f"DINOv3: transformers params {sum(p.numel() for p in hf.parameters())/1e6:.1f}M")
tm = timm_dinov3()
print(f"DINOv3: timm params {sum(p.numel() for p in tm.parameters())/1e6:.1f}M")

from idol.pipeline import IdolPipeline
enc = None
pipe = IdolPipeline(idol_dir(), device="cuda", optimize=False, dinov3_dtype="fp32",
                    image_encoders={"dinov3": hf, "flux2_vae": klein_vae()})
prepared = pipe.preprocess(img_path, mode="synthetic")
os.makedirs(REF, exist_ok=True)
prepared.save(f"{REF}/prepared.png")
Image.open(img_path).save(f"{REF}/input.png")
pixels = torch.from_numpy(np.array(prepared)).permute(2, 0, 1).float().div(255.0)[None].to(dev)
mean = torch.tensor((0.485, 0.456, 0.406), device=dev).view(1, 3, 1, 1); std = torch.tensor((0.229, 0.224, 0.225), device=dev).view(1, 3, 1, 1)
x = (pixels - mean) / std
with torch.no_grad():
    t_hf = hf.to(dev)(pixel_values=x).last_hidden_state
    t_tm = tm.to(dev).forward_features(x)
print(f"DINOv3 tokens at 1024: shape {tuple(t_hf.shape)} vs {tuple(t_tm.shape)}, cosine {cos(t_hf, t_tm):.8f}, "
      f"max abs diff {float((t_hf - t_tm).abs().max()):.3e} (token max {float(t_hf.abs().max()):.2f})")
for i, n in ((0, "cls"), (1, "reg0"), (5, "patch0")):
    print(f"  {n} cosine {cos(t_hf[0, i], t_tm[0, i]):.8f}")
del tm
# ground truth: the transformers model with the gated repo's own weights, as its public ONNX conversion
import onnxruntime as ort, onnx
ort.preload_dlls()
gt_dir = os.path.dirname(hf_hub_download("onnx-community/dinov3-vith16plus-pretrain-lvd1689m-ONNX", "onnx/model.onnx"))
hf_hub_download("onnx-community/dinov3-vith16plus-pretrain-lvd1689m-ONNX", "onnx/model.onnx_data")
mo = onnx.load(f"{gt_dir}/model.onnx", load_external_data=False)
print("gt onnx inputs", [(i.name, [d.dim_param or d.dim_value for d in i.type.tensor_type.shape.dim]) for i in mo.graph.input],
      "outputs", [o.name for o in mo.graph.output], "initializers", len(mo.graph.initializer))
print("gt initializers with bias in the name, layer 0:", [(i.name, list(i.dims)) for i in mo.graph.initializer if "layer.0." in i.name and "bias" in i.name])
del mo
so = ort.SessionOptions()
sess = ort.InferenceSession(f"{gt_dir}/model.onnx", so, providers=["CUDAExecutionProvider", "CPUExecutionProvider"])
print("gt providers", sess.get_providers())
t_gt = torch.from_numpy(sess.run(["last_hidden_state"], {sess.get_inputs()[0].name: x.cpu().numpy()})[0]).to(dev)
del sess
for n, t in (("timm", t_tm), ("transformers arch + timm weights", t_hf)):
    print(f"DINOv3 {n} vs the gated model's ONNX conversion: cosine {cos(t, t_gt):.8f}, max abs diff {float((t - t_gt).abs().max()):.3e}")
torch.cuda.empty_cache()

# --- the reference, on the same randoms whatever the device; fp32 DINOv3 and bf16 DINOv3 (IDOL's default) both
def run(tag, dtype, seed=42, n=262144):
    enc.dinov3 = hf.to(dev, {"fp32": torch.float32, "bf16": torch.bfloat16}[dtype])
    t0 = time.time()
    with CpuRandom(seed) as r:
        out = pipe(img_path, mode="synthetic", seed=seed, num_gaussians=n)
    torch.cuda.synchronize()
    print(f"reference {tag}: {time.time() - t0:.1f}s, peak VRAM {torch.cuda.max_memory_allocated()/2**30:.2f} GB, camera {out.camera}")
    out.gaussian.save_ply(f"{REF}/ref_{tag}.ply")
    return out, r.log

# every network's inputs and outputs on the fp32 run, in the seam's shapes
calls = {"dit": 0, "oct": 0}
def h_mask(m, a, o): save("mask_in", a[0]); save("mask_out", o)
def h_dit(m, a, o):
    i = calls["dit"]; calls["dit"] += 1
    step, neg = divmod(i, 2)
    if step in (0, 12, 25, 37, 49):
        tag = f"dit_s{step:02d}_{'neg' if neg else 'cond'}"
        save(tag + "_latent", a[0]["latent"]); save(tag + "_camera", a[0]["camera"]); save(tag + "_t", a[1])
        save(tag + "_v_latent", o["latent"]); save(tag + "_v_camera", o["camera"])
def h_oct(m, a, o):
    calls["oct"] += 1; l = calls["oct"]
    save(f"oct_l{l}_cells", a[0]); save(f"oct_l{l}_resolution", a[1]); save(f"oct_l{l}_logits", o)
def h_gs(m, a, o): save("gs_anchors", a[0]); save("gs_latent", a[1]); save("gs_features", o)
hooks = [pipe.background_remover.register_forward_hook(h_mask), pipe.denoiser.register_forward_hook(h_dit),
         pipe.decoder.register_forward_hook(h_oct), pipe.decoder_gs.register_forward_hook(h_gs)]
enc = pipe.image_encoder
class Enc:
    def __call__(self, px):
        c = enc(px); save("pixels", px); save("feature1", c["feature1"]); save("feature2", c["feature2"]); return c
pipe.image_encoder = Enc()
out, log = run("fp32", "fp32")
for h in hooks: h.remove()
pipe.image_encoder = enc
save("vae_eps", log[0][1]); save("noise_latent", log[1][1]); save("noise_camera", log[2][1])
save("octree_rand", np.concatenate([v.reshape(-1) for k, v in log[3:]]))
[os.remove(p) for p in __import__("glob").glob(f"{REF}/octree_rand_*.npy")]
save("gs_points_offset_perturbation", pipe.decoder_gs.points_offset_perturbation); save("gs_base_offset_scale", pipe.decoder_gs.base_offset_scale)
import glob
with open(f"{REF}/shapes.txt", "w") as f:
    for p in sorted(glob.glob(f"{REF}/*.npy")):
        a = np.load(p); f.write(f"{os.path.basename(p)}\t{list(a.shape)}\t{a.dtype}\n")
print(open(f"{REF}/shapes.txt").read())
print("dit calls", calls)
print("randoms drawn:", [(k, v.shape) for k, v in log])
out2, _ = run("bf16", "bf16")
print(f"splat fp32 vs bf16 DINOv3: xyz max diff {float((out.gaussian.xyz - out2.gaussian.xyz).abs().max()):.4f}")
