# A Hugging Face repo's files into a Modal volume, fetched inside Modal so nothing goes through this machine
# (docs/cloud-render.feature CR20). From sdk/cloud:
#   RAEA_WEIGHTS_VOLUME=raea-headgen-weights MODAL_PROFILE=raea modal run deploy/modal/weights.py --repo VAST-AI/TripoSG --dest TripoSG
# It prints a line every 30 s while it fetches, so a watcher can tell it's alive.

import os
import threading

import modal

VOLUME = os.environ.get("RAEA_WEIGHTS_VOLUME", "raea-headgen-weights")
vol = modal.Volume.from_name(VOLUME, create_if_missing=True)
app = modal.App("raea-weights", image=modal.Image.debian_slim(python_version="3.12").pip_install("huggingface_hub==0.35.3"))


def size(path):
    return sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(path) for f in fs)


@app.function(volumes={"/vol": vol}, timeout=3600, cpu=2)
def fetch(repo: str, dest: str, revision: str = "main") -> str:
    from huggingface_hub import snapshot_download

    out = f"/vol/{dest}"
    done = threading.Event()

    def beat():
        while not done.wait(30):
            print(f"weights: {size(out) / 1e9:.2f} GB of {repo} so far", flush=True) if os.path.isdir(out) else None

    threading.Thread(target=beat, daemon=True).start()
    snapshot_download(repo, revision=revision, local_dir=out)
    done.set()
    vol.commit()
    return f"weights: {repo}@{revision} in {VOLUME}:/{dest}, {size(out) / 1e9:.2f} GB"


@app.local_entrypoint()
def main(repo: str, dest: str, revision: str = "main"):
    print(fetch.remote(repo, dest, revision))
