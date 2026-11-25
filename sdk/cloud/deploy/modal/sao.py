# Stable Audio Open Small on raea's Modal GPUs (docs/cloud-render.feature CR24, CR25): tools/sao installed once into the
# volume raea-models with its ONNX backend (/models/sao, what the farm names as RAEA_SAO), then sounds made on an
# NVIDIA GPU and on the CPU in the same container, each as raea makes one (a process each, run.py --times). From
# sdk/cloud:
#
#   MODAL_PROFILE=raea modal run deploy/modal/sao.py --setup-only        # the install (CPU, once)
#   MODAL_PROFILE=raea modal run deploy/modal/sao.py [--gpu L4] --out DIR   # times and the wavs
#
# run.py's sampler is the same on every backend, from the same seed, so a wav here is compared with the Mac's Core ML
# take of the same prompt and seed sample by sample (DIR/coreml-*.wav, made with tools/sao/run.py on the Mac).
# The venv's Python comes from the image (/opt/uv-python), so every container of this image runs it.

import json
import os
import shutil
import subprocess
import time

import modal

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, "..", "..", "..", "..", "tools", "sao"))
DIR = "/models/sao"
models = modal.Volume.from_name("raea-models", create_if_missing=True, version=2)
image = (
    modal.Image.from_registry("debian:trixie-slim", add_python="3.12")
    .apt_install("ca-certificates", "curl", "ffmpeg")
    .pip_install("uv==0.8.0")
    .env({"UV_PYTHON_INSTALL_DIR": "/opt/uv-python", "UV_PYTHON_PREFERENCE": "only-managed", "UV_CACHE_DIR": "/tmp/uv-cache"})
    .run_commands("uv python install 3.11")
    .add_local_dir(TOOL, "/tools/sao", ignore=["__pycache__", ".venv"])
)
app = modal.App("raea-sao", image=image)

# (prompt, seed, seconds): what a game's or a film's foley asks for
SOUNDS = [("a wooden door creaks open", 7, 3.0), ("footsteps on gravel", 1, 4.0), ("glass shatters on a stone floor", 3, 2.0),
          ("heavy rain on a tin roof", 2, 6.0)]


@app.function(volumes={"/vol": models}, cpu=4, memory=8192, timeout=3600)
def setup() -> dict:
    # built at its own path on the container's disk (uv's temporary files fail on the volume, and the venv names its
    # path), then copied to the volume, which is mounted elsewhere here; the CUDA build, though this container has none
    t = time.time()
    env = {**os.environ, "RAEA_SAO": DIR, "RAEA_SAO_FROM": "/nonexistent", "RAEA_SAO_BACKEND": "onnx", "RAEA_SAO_CUDA": "1"}
    r = subprocess.run(["sh", "/tools/sao/setup.sh"], capture_output=True, text=True, env=env)
    out = {"code": r.returncode, "setup_s": round(time.time() - t, 1), "log": (r.stderr or r.stdout)[-1500:]}
    if r.returncode:
        return out
    t = time.time()
    if os.path.exists("/vol/sao"):
        shutil.rmtree("/vol/sao")
    subprocess.run(["cp", "-a", DIR, "/vol/sao"], check=True)
    models.commit()
    out["copied_s"] = round(time.time() - t, 1)
    out["size"] = subprocess.run(["du", "-sh", "/vol/sao"], capture_output=True, text=True).stdout.split()[0]
    return out


@app.function(volumes={"/models": models}, gpu=["T4", "L4", "A10G"], cpu=4, memory=16384, timeout=3600)
def bench(sounds: list, cpu_too: int = 2) -> dict:
    gpu = subprocess.run(["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"], capture_output=True, text=True).stdout.strip()
    print(f"bench: {gpu}", flush=True)
    py = f"{DIR}/.venv/bin/python"
    if not os.path.exists(py):
        raise SystemExit(f"no install at {DIR}: run with --setup-only first")
    takes = []
    for i, (prompt, seed, secs) in enumerate(sounds):
        for device in ["cuda"] + (["cpu"] if i < cpu_too else []):
            wav = f"/tmp/{device}-{i}.wav"
            t = time.time()
            r = subprocess.run([py, "-I", f"{DIR}/run.py", "--prompt", prompt, "--seed", str(seed), "--seconds", str(secs),
                                "--device", device, "--times", "--wav", wav], capture_output=True, text=True)
            wall = time.time() - t
            if r.returncode:
                raise SystemExit(f"run.py {device} {prompt!r}: {r.stderr[-2000:]}")
            times = json.loads(r.stderr.strip().splitlines()[-1])
            print(f"bench: {device} {prompt!r}: wall {wall:.1f} s {times}", flush=True)
            with open(wav, "rb") as f:
                takes.append({"i": i, "prompt": prompt, "seed": seed, "seconds": secs, "device": device, "wall_s": round(wall, 2),
                              "times": times, "wav": f.read()})
    return {"gpu": gpu, "takes": takes}


@app.local_entrypoint()
def main(gpu: str = "", out: str = "", setup_only: bool = False):
    if setup_only:
        print(json.dumps(setup.remote(), indent=1))
        return
    res = (bench.with_options(gpu=gpu) if gpu else bench).remote(SOUNDS)
    if out:
        os.makedirs(out, exist_ok=True)
    for t in res["takes"]:
        wav = t.pop("wav")
        if out:
            with open(os.path.join(out, f"onnx-{t['device']}-{t['i']}.wav"), "wb") as f:
                f.write(wav)
    report = json.dumps(res, indent=1)
    print(report)
    if out:
        with open(os.path.join(out, "bench.json"), "w") as f:
            f.write(report)
