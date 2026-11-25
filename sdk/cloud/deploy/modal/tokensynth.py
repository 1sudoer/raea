# TokenSynth on raea's Modal GPUs (docs/cloud-render.feature CR25): tools/tokensynth installed once into the volume
# raea-models (/models/tokensynth, what the farm mounts as RAEA_TOKENSYNTH), then its notes on an NVIDIA GPU
# (run.py --device cuda) measured against the CPU's in the same container. From sdk/cloud:
#
#   MODAL_PROFILE=raea modal run deploy/modal/tokensynth.py --setup-only         # the install (CPU, once)
#   MODAL_PROFILE=raea modal run deploy/modal/tokensynth.py [--gpu L4] [--warm-only] --out DIR   # the check: times, CLAP, the wavs
#
# A note runs as raea runs one (a process each, the model loaded each time) and warm (one process, the runtime loaded
# once, as a batch would). The GPU's notes are their own samples (its logits round differently), so they're judged as
# a new seed is: CLAP's text-audio match to the timbre, against the CPU's, and the GPU take's distance from the CPU take
# against a second CPU take's (another seed). The venv lives on the volume, its Python in the image (/opt/uv-python),
# so every container of this image runs it.

import json
import os
import subprocess
import threading
import time

import modal

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, "..", "..", "..", "..", "tools", "tokensynth"))
DIR = "/models/tokensynth"
models = modal.Volume.from_name("raea-models", create_if_missing=True, version=2)
image = (
    modal.Image.from_registry("debian:trixie-slim", add_python="3.12")
    .apt_install("ca-certificates", "curl", "git", "ffmpeg")
    .pip_install("uv==0.8.0")
    .env({"UV_PYTHON_INSTALL_DIR": "/opt/uv-python", "UV_PYTHON_PREFERENCE": "only-managed", "UV_CACHE_DIR": "/tmp/uv-cache", "HF_HOME": "/tmp/hf"})
    .run_commands("uv python install 3.11")
    .add_local_dir(TOOL, "/tools/tokensynth", ignore=["__pycache__", ".venv"])
)
app = modal.App("raea-tokensynth", image=image)

NOTES = [("grand piano", 48), ("grand piano", 72), ("nylon string guitar", 55), ("warm analog synth pad", 60), ("muted trumpet", 65), ("plucked cello", 43)]
SECONDS, SEED = 2.0, 1234

WARM = r"""
import json, sys, time
sys.path.insert(0, sys.argv[1])
import run, soundfile as sf, numpy as np, torch
notes, device, seed, out = json.loads(sys.argv[2]), sys.argv[3], int(sys.argv[4]), sys.argv[5]
t = time.time(); rt = run.Runtime("fp16", sys.argv[1] + "/weights", device); load = time.time() - t
times = []
for timbre, midi in notes:
    t = time.time(); (audio, _), _ = rt.render(timbre, midi, 100, 2.0, seed)
    if device == "cuda": torch.cuda.synchronize()
    times.append(time.time() - t)
    sf.write(f"{out}/{device}-s{seed}-{midi}-{timbre.replace(' ', '_')}.wav", np.stack([audio, audio], 1), run.SR, subtype="FLOAT")
print(json.dumps({"load": load, "times": times}))
"""

CLAP = r"""
import json, sys, glob, os
import numpy as np, soundfile as sf, torch, laion_clap
from scipy.signal import resample_poly
m = laion_clap.CLAP_Module(enable_fusion=False, amodel="HTSAT-base", device="cuda" if torch.cuda.is_available() else "cpu")
m.load_ckpt(sys.argv[1], verbose=False)
unit = lambda v: v / np.linalg.norm(v)
out = {}
for f in sorted(glob.glob(sys.argv[2] + "/*.wav")):
    a, sr = sf.read(f, dtype="float32")
    a = resample_poly(a.mean(1) if a.ndim > 1 else a, 160, 147).astype(np.float32)  # 44.1 to 48 kHz, CLAP's rate
    timbre = os.path.basename(f)[:-4].split("-", 3)[3].replace("_", " ")
    e = unit(m.get_audio_embedding_from_data(x=a[None], use_tensor=False)[0])
    t = unit(m.get_text_embedding([timbre, timbre])[0])
    out[os.path.basename(f)] = {"text": float(e @ t), "emb": e.tolist()}
print(json.dumps(out))
"""


def beat(label, path, done):
    while not done.wait(30):
        n = sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(path) for f in fs) if os.path.isdir(path) else 0
        print(f"{label}: {n / 1e9:.2f} GB in {path} so far", flush=True)


@app.function(volumes={"/vol": models}, cpu=8, memory=16384, timeout=3600)
def setup() -> dict:
    """tools/tokensynth/setup.sh, then the install onto the volume: the repo, the venv, the weights and convert.py's fp16
    runtime (its Core ML half fails here and is skipped, as on any machine that isn't a Mac). It's built at DIR on the
    container's own disk, the volume mounted beside it at /vol, then copied in: uv's temporary files in a venv on the
    volume fail (11-17), and the venv names its own path (the editable repo), so it's built where bench() finds it."""
    done = threading.Event()
    threading.Thread(target=beat, args=("tokensynth setup", DIR, done), daemon=True).start()
    t = time.time()
    r = subprocess.run(["sh", "/tools/tokensynth/setup.sh"], env={**os.environ, "RAEA_TOKENSYNTH": DIR, "RAEA_TOKENSYNTH_FROM": "/nonexistent"}, capture_output=True, text=True)
    built = round(time.time() - t)
    if r.returncode == 0:
        subprocess.run(["rm", "-rf", "/vol/tokensynth"], check=True)
        subprocess.run(["cp", "-a", DIR, "/vol/tokensynth"], check=True)
        models.commit()
    done.set()
    return {"code": r.returncode, "s": built, "copied_s": round(time.time() - t) - built, "tail": (r.stdout + r.stderr)[-1500:]}


# a list is Modal's fallback order, whichever is free first (11-17: no T4 for 10 minutes, then a T4 taken back mid-run); --gpu names one
@app.function(volumes={"/models": models}, gpu=["T4", "L4", "A10G"], cpu=4, memory=16384, timeout=3600)
def bench(notes: list, process: bool = True) -> dict:
    py = f"{DIR}/.venv/bin/python"
    out = "/tmp/notes"
    os.makedirs(out, exist_ok=True)
    gpu = subprocess.run(["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"], capture_output=True, text=True).stdout.strip()
    print(f"bench: on {gpu}", flush=True)
    res = {"gpu": gpu, "process": [], "warm": {}}
    # a process a note, as raea's asset store runs one (the model loads each time); the CPU on the first two only (11-17:
    # it's 103-105 s a note every time, and all six with the warm runs ran past a 30-minute limit)
    for i, (timbre, midi) in enumerate(notes if process else []):
        row = {"timbre": timbre, "midi": midi}
        for dev in ("cpu", "cuda") if i < 2 else ("cuda",):
            wav = f"/tmp/{dev}-{midi}.wav"
            t = time.time()
            r = subprocess.run([py, "-I", f"{DIR}/run.py", "--timbre", timbre, "--midi", str(midi), "--seconds", str(SECONDS), "--seed", str(SEED),
                                "--weights", f"{DIR}/weights", "--wav", wav] + (["--device", "cuda"] if dev == "cuda" else []), capture_output=True, text=True)
            row[dev] = round(time.time() - t, 2) if r.returncode == 0 else f"failed: {r.stderr.strip().splitlines()[-1:]}"
            print(f"bench: {timbre} {midi} on {dev}: {row[dev]}", flush=True)
        res["process"].append(row)
    # warm: one process, the runtime loaded once; the CPU twice (a second seed: how far two CPU takes are apart)
    for dev, seed in (("cuda", SEED), ("cpu", SEED), ("cpu", SEED + 1)):
        # -I: Modal puts this file, tokensynth.py, in the working folder, and `-c` would import it as the package (11-17)
        r = subprocess.run([py, "-I", "-c", WARM, DIR, json.dumps(notes), dev, str(seed), out], capture_output=True, text=True)
        res["warm"][f"{dev}-s{seed}"] = json.loads(r.stdout.strip().splitlines()[-1]) if r.returncode == 0 else r.stderr[-800:]
        print(f"bench: warm {dev} seed {seed}: {json.dumps(res['warm'][f'{dev}-s{seed}'])[:600]}", flush=True)
    r = subprocess.run([py, "-I", "-c", CLAP, f"{DIR}/weights/clap_music_audioset_epoch_15_esc_90.14.pt", out], capture_output=True, text=True)
    res["clap"] = json.loads(r.stdout.strip().splitlines()[-1]) if r.returncode == 0 else r.stderr[-800:]
    if isinstance(res["clap"], dict):
        for f, v in res["clap"].items():
            print(f"bench: clap {f}: text {v['text']:.3f}", flush=True)
    else:
        print(f"bench: clap failed: {res['clap']}", flush=True)
    res["wavs"] = {f: open(os.path.join(out, f), "rb").read() for f in sorted(os.listdir(out))}
    return res


@app.local_entrypoint()
def main(gpu: str = "", out: str = "", setup_only: bool = False, setup_first: bool = False, warm_only: bool = False):
    if setup_only or setup_first:
        s = setup.remote()
        print(f"setup: exit {s['code']} in {s['s']} s\n{s['tail']}")
        if setup_only or s["code"] != 0:
            return
    r = (bench.with_options(gpu=gpu) if gpu else bench).remote(NOTES, not warm_only)
    wavs = r.pop("wavs")
    if out:
        os.makedirs(out, exist_ok=True)
        for f, b in wavs.items():
            open(os.path.join(out, f), "wb").write(b)
        json.dump(r, open(os.path.join(out, "bench.json"), "w"), indent=1)
    clap = r.get("clap") if isinstance(r.get("clap"), dict) else {}
    print(json.dumps({k: v for k, v in r.items() if k != "clap"}, indent=1))
    for f, v in clap.items():
        print(f"clap {f}: text {v['text']:.3f}")
