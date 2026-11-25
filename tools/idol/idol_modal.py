"""IDOL -> ONNX on Modal: runs one step script on a GPU with the idol-sdk checkout and a volume (idol-work) for
weights, reference dumps and exports.

    IDOL_SDK=/path/to/idol-sdk modal run tools/idol/idol_modal.py --step s1_sources [--on gpu|big|cpu]
"""
import os, subprocess, sys
import modal

HERE = os.path.dirname(os.path.abspath(__file__))
# a checkout of github.com/speridlabs/idol-sdk (8db7003 when this was written), outside this repository
SDK = os.environ["IDOL_SDK"]
vol = modal.Volume.from_name("idol-work", create_if_missing=True)
image = (
    modal.Image.debian_slim(python_version="3.11")
    .apt_install("git")
    .uv_pip_install(
        "torch==2.8.0", "torchvision==0.23.0", "transformers>=4.56", "diffusers>=0.40", "accelerate>=1.0",
        "safetensors>=0.4", "tensordict>=0.8", "numpy>=1.26", "pillow>=10.0",
        "plyfile>=1.0", "tqdm", "timm>=1.0.20", "onnx>=1.18", "onnxscript", "onnxconverter-common",
        "onnxruntime-gpu==1.22.0", "hf_transfer",
    )
    .env({"HF_HOME": "/vol/hf", "HF_HUB_ENABLE_HF_TRANSFER": "1", "PYTHONUNBUFFERED": "1"})
    .add_local_dir(SDK, "/sdk", ignore=[".git"])
    .add_local_dir(os.path.join(HERE, "steps"), "/steps")
)
app = modal.App("idol-onnx", image=image)


def _run(step, args):
    vol.reload()
    r = subprocess.run([sys.executable, f"/steps/{step}.py", *args.split()], cwd="/vol",
                       env={**os.environ, "PYTHONPATH": "/sdk:/steps"})
    vol.commit()
    if r.returncode:
        raise SystemExit(r.returncode)


@app.function(gpu=["L4", "A10G", "L40S"], volumes={"/vol": vol}, timeout=3600, memory=32768)
def gpu(step: str, args: str = ""):
    _run(step, args)


@app.function(gpu="A100-40GB", volumes={"/vol": vol}, timeout=3600, memory=65536)
def big(step: str, args: str = ""):
    _run(step, args)


@app.function(volumes={"/vol": vol}, timeout=3600, memory=65536, cpu=8)
def cpu(step: str, args: str = ""):
    _run(step, args)


@app.local_entrypoint()
def main(step: str, args: str = "", on: str = "gpu"):
    {"gpu": gpu, "big": big, "cpu": cpu}[on].remote(step, args)
