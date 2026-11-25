"""raea motion on PyTorch, for the machines MLX doesn't run on (Linux, Windows): upstream Kimodo (nv-tlabs/kimodo, code
Apache-2.0) in the Python environment `raea setup motion` makes in raea's models folder (kimodo-torch), run by raea
(systems/kimodo/src/torch.rs), which keeps this file in its binary and writes it beside the environment before each use.

    python -I run.py fetch DIR             the weights into DIR, each repository at a fixed revision: Kimodo-SOMA-RP-v1
                                           (NVIDIA Open Model License), LLM2Vec's two adapters (MIT) and Meta Llama 3 8B
                                           Instruct in bf16 (Meta Llama 3 Community License) from NousResearch's ungated
                                           copy, whose four weight files are checked by SHA-256 against Meta's own
    python -I run.py probe DIR             what this machine has and where each network would run, as one JSON line
    python -I run.py make DIR ASK.json OUT one clip: ASK is {line, frames, seed, steps, keys (a constraints file or null),
                                           cfg}; OUT gets the denoised features (frames x 369 float32, normalised, as
                                           Kimodo's sampler leaves them), which raea decodes into the same clip the MLX
                                           path writes; stdout's last line is JSON about the run

The networks are upstream's own, called as its generate script calls them (post_processing=False, as on MLX). Where
they run: the denoiser on CUDA when there is an NVIDIA GPU, else on the CPU; LLM2Vec (16 GB in bf16) beside it on a GPU
of 18 GB or more, else on the CPU. RAEA_MOTION_DEVICE=cpu|cuda and RAEA_MOTION_TEXT_DEVICE=cpu|cuda say otherwise. A
line's 4096 numbers are kept in DIR/lines once read, so the encoder loads once a line, as on MLX.
"""
import hashlib
import json
import os
import pathlib
import sys
import time

MODEL = "Kimodo-SOMA-RP-v1"
MNTP = "McGill-NLP/LLM2Vec-Meta-Llama-3-8B-Instruct-mntp"
SUPERVISED = "McGill-NLP/LLM2Vec-Meta-Llama-3-8B-Instruct-mntp-supervised"
# the adapters name Meta's repository as their base; its bytes are fetched from an ungated copy into a folder of that
# name under DIR/text, which is where the encoder is loaded from (relative to it: see Text.load)
BASE = "meta-llama/Meta-Llama-3-8B-Instruct"
SHARDS = [f"model-0000{i}-of-00004.safetensors" for i in range(1, 5)]
# (repository, revision, files, folder under DIR)
FILES = [
    ("nvidia/Kimodo-SOMA-RP-v1", "defbe1f34f5fd031eb0ccfe19f3f92a9880c287b",
     ["config.yaml", "model.safetensors", "LICENSE"] + [f"stats/motion/{p}/{s}.npy" for p in ("global_root", "local_root", "body") for s in ("mean", "std")],
     f"checkpoints/{MODEL}"),
    (MNTP, "31474e395ada192e8ed1586db6be79fb3b70c9c0",
     ["adapter_config.json", "adapter_model.safetensors", "config.json", "special_tokens_map.json", "tokenizer.json", "tokenizer_config.json"],
     f"text/{MNTP}"),
    (SUPERVISED, "baa8ebf04a1c2500e61288e7dad65e8ae42601a7", ["adapter_config.json", "adapter_model.safetensors"], f"text/{SUPERVISED}"),
    ("NousResearch/Meta-Llama-3-8B-Instruct", "53346005fb0ef11d3b6a83b12c895cca40156b6c",
     ["config.json", "generation_config.json", "model.safetensors.index.json", "special_tokens_map.json", "tokenizer.json", "tokenizer_config.json", "LICENSE", "USE_POLICY.md"] + SHARDS,
     f"text/{BASE}"),
]
# meta-llama/Meta-Llama-3-8B-Instruct's own weight files (Hugging Face's LFS SHA-256, read 11-17), which the copy matches
META_SHA256 = {
    "model-00001-of-00004.safetensors": "d8cf9c4d0dd972e1a2131bfe656235ee98221679711a3beef6d46dadf0f20b5c",
    "model-00002-of-00004.safetensors": "8d4782b4a69ef03845159ce1a15e272aadaaf134dc138d68f616098e8531729c",
    "model-00003-of-00004.safetensors": "3acdd690e65c24f42a24581b8467af98bd3ca357444580f8012aacd2bd607921",
    "model-00004-of-00004.safetensors": "67e9ad31c8c32abf3a55ee7fc7217b3ecb35fd3c74d98a5bd233e0e4d6964f46",
}
TEXT_DIM = 4096
# the encoder's name in a kept line's key: bf16 here, so not the MLX path's 8-bit numbers
ENCODER = "llm2vec-llama3-8b-instruct-bf16"
# a GPU this big holds LLM2Vec in bf16 beside the denoiser (about 17 GB together)
TEXT_ON_GPU_BYTES = 18 << 30
# memory LLM2Vec needs on the CPU in bf16: its 16 GB and Python's own (a little under 18 GB, so a machine sold as 18 GB passes)
TEXT_ON_CPU_BYTES = 17 << 30


def say(*a):
    print("raea motion:", *a, file=sys.stderr, flush=True)


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 24), b""):
            h.update(b)
    return h.hexdigest()


def fetch(d):
    from huggingface_hub import hf_hub_download

    for repo, rev, files, sub in FILES:
        dest = d / sub
        for f in files:
            if (dest / f).exists():
                continue
            say(f"fetching {repo} {f}")
            hf_hub_download(repo, f, revision=rev, local_dir=str(dest))
    base = d / "text" / BASE
    checked = base / ".sha256-checked"
    if not checked.exists():
        for f, want in META_SHA256.items():
            say(f"checking {f} against Meta's SHA-256")
            got = sha256(base / f)
            if got != want:
                (base / f).unlink()
                raise SystemExit(f"{base / f}: SHA-256 {got} isn't Meta's {want}; removed, so the next setup fetches it again")
        checked.write_text(json.dumps(META_SHA256, indent=1))
    print(json.dumps({"fetched": str(d)}))


def ram_bytes():
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (AttributeError, ValueError, OSError):
        pass
    try:
        import ctypes

        class Status(ctypes.Structure):
            _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong), ("ullTotalPhys", ctypes.c_ulonglong)] + [(n, ctypes.c_ulonglong) for n in ("a", "b", "c", "d", "e", "f")]

        s = Status()
        s.dwLength = ctypes.sizeof(Status)
        ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(s))
        return int(s.ullTotalPhys)
    except Exception:
        return None


def plan():
    import torch

    cuda = torch.cuda.is_available()
    p = {"torch": torch.__version__, "cuda": torch.version.cuda if cuda else None, "gpu": None, "vram_gb": None, "ram_gb": None}
    if cuda:
        g = torch.cuda.get_device_properties(0)
        p["gpu"], p["vram_gb"] = g.name, round(g.total_memory / 2**30, 1)
    r = ram_bytes()
    p["ram_gb"] = round(r / 2**30, 1) if r else None
    motion = os.environ.get("RAEA_MOTION_DEVICE") or ("cuda" if cuda else "cpu")
    text = os.environ.get("RAEA_MOTION_TEXT_DEVICE") or ("cuda" if motion == "cuda" and cuda and torch.cuda.get_device_properties(0).total_memory >= TEXT_ON_GPU_BYTES else "cpu")
    p["motion"], p["text"] = motion, text
    p["threads"] = torch.get_num_threads()
    return p


class Text:
    """Kimodo's text encoder, as Kimodo calls it: LLM2Vec (on Llama 3 8B Instruct, bf16), loaded only when a line hasn't
    been read before; a line's 4096 numbers are kept in DIR/lines, and an empty line is zeros (Kimodo zeroes it)."""

    def __init__(self, d, device):
        self.d, self.device, self.enc, self.read, self.load_s = d, device, None, [], 0.0

    def to(self, *a, **k):
        return self

    def eval(self):
        return self

    def kept(self, line):
        return self.d / "lines" / (hashlib.sha256(f"{ENCODER}\n{line}".encode()).hexdigest()[:16] + ".f32")

    def load(self):
        from kimodo.model.llm2vec import LLM2VecEncoder

        ram = ram_bytes()
        if self.device == "cpu" and ram and ram < TEXT_ON_CPU_BYTES:
            raise SystemExit(f"reading a new line runs the text encoder (Llama 3 8B Instruct, 16 GB in bf16) on the CPU, which needs about 18 GB of memory, and this machine has {ram / 2**30:.0f} GB: "
                             "a line already read still works, and a GPU of 18 GB or more holds the encoder instead")
        t = self.d / "text"
        # the adapters' base is named by its repository; from DIR/text that name is the local folder of its bytes, so
        # nothing is fetched (HF_HUB_OFFLINE) and the gated repository is never asked for
        os.chdir(t)
        os.environ["TEXT_ENCODERS_DIR"] = str(t)
        say(f"loading the text encoder (LLM2Vec on Llama 3 8B Instruct, 16 GB in bf16) on {self.device}")
        t0 = time.time()
        self.enc = LLM2VecEncoder(base_model_name_or_path=MNTP, peft_model_name_or_path=SUPERVISED, dtype="bfloat16", llm_dim=TEXT_DIM, device=self.device)
        self.load_s = round(time.time() - t0, 2)
        say(f"text encoder loaded in {self.load_s:.1f} s")

    def one(self, line):
        import numpy as np
        import torch

        if not line.strip():
            return torch.zeros(TEXT_DIM)
        p = self.kept(line)
        if p.exists() and p.stat().st_size == 4 * TEXT_DIM:
            return torch.from_numpy(np.fromfile(p, dtype="<f4").copy())
        if self.enc is None:
            self.load()
        t0 = time.time()
        e, _ = self.enc([line])
        e = e.reshape(-1, TEXT_DIM)[0].float().cpu()
        self.read.append(round(time.time() - t0, 2))
        p.parent.mkdir(parents=True, exist_ok=True)
        e.numpy().astype("<f4").tofile(p)
        return e

    def __call__(self, texts):
        import torch

        texts = [texts] if isinstance(texts, str) else list(texts)
        return torch.stack([self.one(t) for t in texts])[:, None], [1] * len(texts)


def make(d, ask_path, out_path):
    ask = json.loads(pathlib.Path(ask_path).read_text())
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["CHECKPOINT_DIR"] = str(d / "checkpoints")
    t0 = time.time()
    import torch
    from kimodo.constraints import load_constraints_lst
    from kimodo.model.load_model import load_model
    from kimodo.sanitize import sanitize_texts

    p = plan()
    clean = sanitize_texts([ask["line"]])[0]
    text = Text(d, p["text"])
    model = load_model(MODEL, device=p["motion"], text_encoder=text)
    cons = load_constraints_lst(str(pathlib.Path(ask["keys"]).resolve()), model.skeleton) if ask.get("keys") else None
    load_s = time.time() - t0
    # the features the sampler leaves, taken where Kimodo decodes them
    grab = {}
    inverse = model.motion_rep.inverse

    def keep(motion, *a, **k):
        grab["features"] = motion
        return inverse(motion, *a, **k)

    model.motion_rep.inverse = keep
    torch.manual_seed(int(ask["seed"]))
    t1 = time.time()
    out = model([ask["line"]], int(ask["frames"]), num_denoising_steps=int(ask["steps"]), constraint_lst=[cons] if cons else [],
                cfg_weight=[float(c) for c in ask.get("cfg", [2.0, 2.0])], num_samples=1, post_processing=False, progress_bar=lambda x, **k: x)
    gen_s = time.time() - t1 - sum(text.read) - text.load_s
    f = grab["features"].reshape(-1, int(ask["frames"]), grab["features"].shape[-1])[0].float().cpu().numpy()
    if f.shape[1] != 369:
        raise SystemExit(f"Kimodo's features are {f.shape[1]} a frame, not 369")
    f.astype("<f4").tofile(out_path)
    pj = out["posed_joints"]
    root = pj.reshape(-1, *pj.shape[-3:])[0, :, 0].float().cpu().tolist()
    print(json.dumps({"line": clean, "motion": p["motion"], "text": p["text"], "gpu": p["gpu"], "torch": p["torch"], "load_s": round(load_s, 2),
                      "text_load_s": text.load_s, "read_s": sum(text.read), "gen_s": round(gen_s, 2), "frames": f.shape[0], "root": [[round(v, 5) for v in r] for r in root]}))


if __name__ == "__main__":
    verb, d = sys.argv[1], pathlib.Path(sys.argv[2]).resolve()
    if verb == "fetch":
        fetch(d)
    elif verb == "probe":
        print(json.dumps(plan()))
    elif verb == "make":
        make(d, sys.argv[3], sys.argv[4])
    else:
        raise SystemExit(f"run.py fetch|probe|make DIR ...: no verb {verb!r}")
