"""Shared by the steps: the ungated encoder sources, and the reference pipeline with every random drawn on the CPU."""
import os
import numpy as np
import torch
from huggingface_hub import hf_hub_download, snapshot_download
from safetensors.torch import load_file

TIMM = "timm/vit_huge_plus_patch16_dinov3.lvd1689m"
KLEIN = "black-forest-labs/FLUX.2-klein-4B"
REF = "/vol/ref"
ONNX = "/vol/onnx"


def idol_dir():
    return snapshot_download("speridlabs/idol", allow_patterns=["*.json", "*.safetensors"])


def timm_dinov3():
    import timm
    m = timm.create_model("vit_huge_plus_patch16_dinov3.lvd1689m", pretrained=False, num_classes=0)
    m.load_state_dict(load_file(hf_hub_download(TIMM, "model.safetensors")), strict=True)
    return m.eval().requires_grad_(False)


class TimmTokens(torch.nn.Module):
    """timm's DINOv3 with the output IDOL reads from transformers: last_hidden_state [B, 1 + 4 + patches, 1280]."""
    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, pixel_values):
        from types import SimpleNamespace
        return SimpleNamespace(last_hidden_state=self.m.forward_features(pixel_values))


def klein_vae(dtype=torch.bfloat16):
    from diffusers.models import AutoencoderKLFlux2
    return AutoencoderKLFlux2.from_pretrained(KLEIN, subfolder="vae", torch_dtype=dtype).eval().requires_grad_(False)


class CpuRandom:
    """torch.randn / rand / rand_like drawn from one CPU generator, so a run is the same on any device."""
    def __init__(self, seed):
        self.g = torch.Generator().manual_seed(seed)
        self.log = []

    def __enter__(self):
        self.old = (torch.randn, torch.rand, torch.rand_like)
        def randn(*size, device=None, **kw):
            x = self.old[0](*size, generator=self.g); self.log.append(("randn", x.numpy().copy())); return x.to(device)
        def rand(*size, device=None, **kw):
            x = self.old[1](*size, generator=self.g); self.log.append(("rand", x.numpy().copy())); return x.to(device)
        def rand_like(t, dtype=None, **kw):
            x = self.old[1](*t.shape, generator=self.g); self.log.append(("rand", x.numpy().copy())); return x.to(t.device)
        torch.randn, torch.rand, torch.rand_like = randn, rand, rand_like
        return self

    def __exit__(self, *a):
        torch.randn, torch.rand, torch.rand_like = self.old


def save(name, x):
    os.makedirs(REF, exist_ok=True)
    np.save(f"{REF}/{name}.npy", x.detach().float().cpu().numpy() if torch.is_tensor(x) else np.asarray(x))


def cos(a, b):
    a, b = torch.as_tensor(a).double().flatten(), torch.as_tensor(b).double().flatten()
    return float(torch.dot(a, b) / (a.norm() * b.norm()))
