# raea cloud's GPU bench (docs/cloud-render.feature CR2): the same raea renders and Blender Cycles scenes on each
# Modal GPU kind, so a render's price per frame is measured, not guessed. From sdk/cloud:
#
#   MODAL_PROFILE=raea modal run bench/gpus.py                       # every kind in KINDS
#   MODAL_PROFILE=raea modal run bench/gpus.py --gpus L4,L40S        # some
#   MODAL_PROFILE=raea modal run bench/gpus.py --gpus none           # the CPU only (llvmpipe, Cycles on CPU)
#
# Each kind runs in its own container at once, and writes bench/results/KIND.json:
# - device: nvidia-smi's name, memory and driver; the Vulkan device raea opened
# - start: seconds from the call to the container running (a cold start: image pulled, GPU attached)
# - raea: wall seconds for test/gpu/edit.tsx (576 lit shadowed blocks, 1080p, 90 frames) and the realtime bench's
#   solari.tsx (a Solari-lit room, 720p, 150 frames), with RAEA_FRAME_LOG's per-frame means (react, sync, gpu ms)
# - blender: the Open Data benchmark's samples per minute (monster, junkshop, classroom; Blender 5.3, OptiX), the
#   number opendata.blender.org ranks cards by, and one classroom frame at 1920x1080 timed end to end
# - usd: what the container cost at Modal's list price per second (deploy/modal/prices.py)

import json
import os
import subprocess
import time

import modal

HERE = os.path.dirname(os.path.abspath(__file__))
CLOUD = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(CLOUD))
RAEA = os.environ.get("RAEA_VERSION", "0.1.6")
BLENDER = "5.3.0"
LAUNCHER = "https://download.blender.org/release/BlenderBenchmark2.0/launcher/benchmark-launcher-cli-3.3.0-linux.tar.gz"
KINDS = ["T4", "L4", "A10", "L40S", "RTX-PRO-6000", "A100-40GB", "H100"]
SCENES = ["monster", "junkshop", "classroom"]

image = (
    modal.Image.from_registry("debian:trixie-slim", add_python="3.12")
    # raea's runtime libraries and NVIDIA's Vulkan driver's (docs/cloud.feature CL13); Blender's X11 and GL libraries
    .apt_install(
        "ca-certificates", "curl", "xz-utils", "bzip2", "procps",
        "libasound2t64", "libfontconfig1", "libvulkan1", "mesa-vulkan-drivers", "vulkan-tools",
        "libxext6", "libx11-6", "libgl1", "libegl1",
        "libxi6", "libxxf86vm1", "libxfixes3", "libxrender1", "libxkbcommon0", "libsm6", "libice6",
    )
    .run_commands(
        f"set -e; f=raea-{RAEA}-linux-x86_64.tar.gz; u=https://github.com/shinyobjectz/raea/releases/download/v{RAEA}; "
        "cd /tmp && curl -fsSL -o $f $u/$f && curl -fsSL -o $f.sha256 $u/$f.sha256 && sha256sum -c $f.sha256 "
        "&& mkdir -p /opt/raea && tar -xzf $f -C /opt/raea --strip-components=1 && rm -f $f $f.sha256 "
        "&& ln -s /opt/raea/bin/raea /usr/local/bin/raea && raea --version",
        # the benchmark launcher, its Blender and its scenes, in the image so a run downloads nothing
        f"mkdir -p /opt/bench && curl -fsSL {LAUNCHER} | tar -xz -C /opt/bench",
        f"cd /opt/bench && ./benchmark-launcher-cli blender download {BLENDER} && "
        f"./benchmark-launcher-cli scenes download --blender-version {BLENDER} {' '.join(SCENES)}",
    )
    # NVIDIA's Vulkan driver by the library the container runtime mounts (Modal passes no ICD file in)
    .add_local_file(os.path.join(CLOUD, "image", "nvidia_icd.json"), "/usr/share/vulkan/icd.d/nvidia_icd.json", copy=True)
    .env({"WGPU_BACKEND": "vulkan", "NVIDIA_DRIVER_CAPABILITIES": "all"})
    .add_local_file(os.path.join(CLOUD, "test", "gpu", "edit.tsx"), "/opt/scenes/edit.tsx")
    .add_local_file(os.path.join(REPO, "bench", "realtime", "cases", "solari.tsx"), "/opt/scenes/solari.tsx")
    .add_local_file(os.path.join(CLOUD, "deploy", "modal", "prices.py"), "/opt/bench/prices.py")
    .add_local_file(os.path.join(HERE, "frame.py"), "/opt/bench/frame.py")
)

app = modal.App("raea-cloud-bench", image=image)


def sh(cmd, env=None, timeout=1800):
    print("bench:", " ".join(cmd)[:160], flush=True)
    t = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, **(env or {})}, timeout=timeout)
    return {"ok": r.returncode == 0, "s": round(time.time() - t, 2), "out": (r.stdout + r.stderr)[-3000:], "stdout": r.stdout}


def frame_log(path):
    # RAEA_FRAME_LOG: frame, react, sync, draw3d, gpu3d, page, gpupage, gpu (ms), tab-separated
    try:
        rows = [[float(x) for x in l.split("\t")] for l in open(path) if l.strip() and l[0].isdigit()]
    except OSError:
        return None
    if not rows:
        return None
    mean = lambda i: round(sum(r[i] for r in rows) / len(rows), 2)
    cols = ["react", "sync", "draw3d", "gpu3d", "page", "gpupage", "gpu"]
    return {"frames": len(rows), **{c: mean(i + 1) for i, c in enumerate(cols) if i + 1 < len(rows[0])}}


def raea_renders(on_cpu):
    # raea's home its own (the launcher keeps its Blender and scenes under root's); on the CPU, lavapipe and no EGL
    env = {"HOME": "/tmp/home"}
    if on_cpu:
        env |= {"VK_DRIVER_FILES": "/usr/share/vulkan/icd.d/lvp_icd.json", "__EGL_VENDOR_LIBRARY_DIRS": "/nonexistent"}
    os.makedirs("/tmp/home", exist_ok=True)
    vk = sh(["vulkaninfo", "--summary"], env)["stdout"]
    out = {"vulkan": [l.split("=")[1].strip() for l in vk.splitlines() if "deviceName" in l]}
    # each scene as raea renders it by default, then without the clipping check (RAEA_CLIP_CHECK=0), which a render
    # worker can leave to the box that asked for the render
    for name in ("edit", "solari"):
        for clip in ("1", "0"):
            log = f"/tmp/{name}-{clip}.log"
            r = sh(["raea", "render", f"/opt/scenes/{name}.tsx", "-o", f"/tmp/{name}-{clip}.mp4"], {**env, "RAEA_FRAME_LOG": log, "RAEA_TIMES": "1", "RAEA_CLIP_CHECK": clip})
            r.pop("stdout")
            r["out"] = [l for l in r["out"].splitlines() if l.startswith(("APPLIED", "REFUSED", "raea times", "Error", "error"))] if r["ok"] else r["out"]
            out[name if clip == "1" else f"{name}-noclip"] = {**r, "frames": frame_log(log)}
    return out


def launcher(*args):
    return sh(["/opt/bench/benchmark-launcher-cli", *args], timeout=3000)


def blender(device_type):
    devices = launcher("devices", "--blender-version", BLENDER)
    names = [l.split("\t")[0].strip() for l in devices["out"].splitlines() if device_type in l.split("\t")[1:]]
    if not names:
        return {"devices": devices["out"]}
    r = launcher("benchmark", "--blender-version", BLENDER, "--device-type", device_type, "--device-name", names[0], "--json", *SCENES)
    try:
        spm = {s["scene"]["label"]: round(s["stats"]["samples_per_minute"], 1) for s in json.loads(r["stdout"])}
    except (ValueError, KeyError, TypeError):
        spm = None
    return {"device": names[0], "s": r["s"], "samples_per_minute": spm, "out": None if spm else r["stdout"][-3000:]}


@app.function(cpu=8, memory=32768, timeout=3600)
def bench(kind: str, called: float) -> dict:
    import sys

    sys.path.insert(0, "/opt/bench")
    import frame
    import prices

    started = time.time()
    gpu = kind != "none"
    res = {"kind": kind, "raea_version": RAEA, "blender_version": BLENDER, "start": round(started - called, 1)}
    res["device"] = sh(["nvidia-smi", "--query-gpu=name,memory.total,driver_version", "--format=csv,noheader"])["out"].strip() if gpu else "cpu"
    res["raea"] = raea_renders(not gpu)
    device_type = "OPTIX" if gpu else "CPU"
    res["blender"] = blender(device_type)
    res["frame"] = frame.classroom(BLENDER, device_type)
    res["s"] = round(time.time() - started, 1)
    res["usd"] = prices.cost(kind, cpu=8, memory_gib=32, seconds=time.time() - called)
    return res


@app.local_entrypoint()
def main(gpus: str = ",".join(KINDS)):
    kinds = [g.strip() for g in gpus.split(",") if g.strip()]
    calls = [(k, bench.with_options(gpu=None if k == "none" else k).spawn(k, time.time())) for k in kinds]
    os.makedirs(os.path.join(HERE, "results"), exist_ok=True)
    for kind, call in calls:
        try:
            r = call.get()
        except Exception as e:  # one kind failing (no capacity, a crash) leaves the others' results
            r = {"kind": kind, "error": repr(e)}
        with open(os.path.join(HERE, "results", f"{kind}.json"), "w") as f:
            json.dump(r, f, indent=1)
        print(json.dumps({k: r.get(k) for k in ("kind", "device", "start", "s", "usd", "error")}))
