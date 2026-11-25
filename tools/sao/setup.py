"""Stable Audio Open Small (stabilityai/stable-audio-open-small, Stability AI Community License) for raea-render's asset
store: `make("sao", prompt, seed, {seconds})` renders one sound locally (run.py, copied in beside the models). On a Mac
it runs on Core ML (the packages are mlboydaisuke/Stable-Audio-Open-Small-CoreML); on Linux and Windows on ONNX Runtime
(the same weights as three fp32 graphs, atauser001/stable-audio-open-small-onnx: 2 GB, with onnxruntime-gpu and its
CUDA libraries where nvidia-smi finds a GPU, else onnxruntime; RAEA_SAO_CUDA=1|0 decides). RAEA_SAO_BACKEND=coreml|onnx
picks the backend. Installs into
$RAEA_SAO (default ~/.cache/raea-sao); raea-render runs this the first time a sound is asked for and nothing is
installed, and `raea setup-audio` runs it on purpose (tools/local_setup.py: how, on any OS).

What it fetches, once: the models at a pinned revision, each checked against its SHA-256; t5-base's tokenizer.json
(1.4 MB), so runs never touch the network; and a small venv. On a Mac the published fp16 T5 encoder overflows, so the
T5 is raea's own fp32 conversion (t5_to_coreml.py, 419 MB), made once in uv's throwaway environment with torch and
t5-base's weights (892 MB, deleted after), unless another install has it to copy ($RAEA_SAO_FROM, else
~/.cache/raea-sao when installing somewhere else)."""

import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from local_setup import MAC, Setup, low  # noqa: E402

REPO, REV = "mlboydaisuke/Stable-Audio-Open-Small-CoreML", "8fa80903adebfeb62fc885c45505ac81628f3320"
T5REV = "a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1"
T5SUM = "d2acde0d8d71dd30a711834b07781b9c89feaac33fd332f60507699282740066"  # t5-base's tokenizer.json at T5REV
ONNXREPO, ONNXREV = "atauser001/stable-audio-open-small-onnx", "8f9544df5933b9ed4a7e8ed7fa6f77b3dd48ddce"
# each Core ML package's model.mlmodel and weight.bin
COREML = {
    "NumberEmbedder": ("14ecb9a0f6c3d04c3a611352343d7dea0c5b0720c9f8b56d87b8eae794b9c64c", "8ae6ab6d7bf960678564163be68ad300e245030b56eb5db10b2ab3e9090220d8"),
    "VAEDecoder": ("9614b1ec8ba90995c1b8e797da19ba21fc4acc67b0dfe0bbb28527dcbfc0a262", "a8a1bdb84b854f38e2ef1d3adb5860a03303a442f8eaa8d567aeb606e66a4fc8"),
    "DiT": ("a118772c17fb831a6eda6a1ec2527f535d004064e27a8eb4597d5ae6d33a0576", "c5444973f1a2bb3878a33049a4c559a1097de77fca8dc358aba38ee72033e97d"),
}
# the ONNX graphs, against the SHA-256 their model card publishes
ONNX = {
    "cond.onnx": "847822809975d0c36a665594fad0312c4c225147910657820e9936b09014629c",
    "dit.onnx": "8e9c8b14baac4a26159d5ecf3d365ef5a097f321541f9548d62d136354a23d8b",
    "dec.onnx": "96029859b4d563f3aff266e97e715190e8d53278c6e27ab94095535a0da4c911",
}


def nvidia():
    smi = shutil.which("nvidia-smi")
    return bool(smi) and subprocess.run([smi, "-L"], capture_output=True).returncode == 0


backend = os.environ.get("RAEA_SAO_BACKEND") or ("coreml" if MAC else "onnx")
with Setup("sao", "RAEA_SAO", "raea-sao") as s:
    # the venv: only what run.py imports. An existing one that already has them is left alone
    if backend == "onnx":
        # RAEA_SAO_CUDA=1 asks for the CUDA build, =0 for the CPU one (its CUDA libraries are a few GB); unset, a GPU decides
        cuda = os.environ["RAEA_SAO_CUDA"] != "0" if os.environ.get("RAEA_SAO_CUDA") else nvidia()
        runtime = "onnxruntime-gpu[cuda,cudnn]==1.23.2" if cuda else "onnxruntime==1.23.2"
        s.venv("3.11", ["numpy==2.4.6", "tokenizers==0.23.3", runtime], imports=("numpy", "onnxruntime", "tokenizers"), what=f"making the venv ({runtime})")
    else:
        s.venv("3.11", ["numpy==2.4.6", "coremltools==9.0", "tokenizers==0.23.3"], imports=("numpy", "coremltools", "tokenizers"))

    # the tokenizer from the install, never the Hugging Face cache
    s.fetch("t5-tokenizer/tokenizer.json", f"https://huggingface.co/google-t5/t5-base/resolve/{T5REV}/tokenizer.json", T5SUM)

    if backend == "onnx":
        o = f"https://huggingface.co/{ONNXREPO}/resolve/{ONNXREV}"
        for name, sha in ONNX.items():
            s.fetch(f"onnx/{name}", f"{o}/{name}", sha)
        # its licence and NOTICE (what the conversion changed) beside them
        s.fetch("onnx/LICENSE", f"{o}/LICENSE")
        s.fetch("onnx/NOTICE", f"{o}/NOTICE")
        shutil.copy2(HERE / "run.py", "run.py")
        s.ready("onnx", ".venv")
        sys.exit(0)

    hf, p = f"https://huggingface.co/{REPO}/resolve/{REV}", "Data/com.apple.CoreML"
    for name, (mlmodel, weights) in COREML.items():
        pkg = f"StableAudio{name}.mlpackage"
        s.fetch(f"coreml-ref/{pkg}/Manifest.json", f"{hf}/{pkg}/Manifest.json")
        s.fetch(f"coreml-ref/{pkg}/{p}/model.mlmodel", f"{hf}/{pkg}/{p}/model.mlmodel", mlmodel)
        s.fetch(f"coreml-ref/{pkg}/{p}/weights/weight.bin", f"{hf}/{pkg}/{p}/weights/weight.bin", weights)

    t5 = Path("coreml/T5EncoderFP32.mlpackage")
    # made under another name until it's whole (coremltools insists on the .mlpackage extension)
    part = Path("coreml/T5EncoderFP32.part.mlpackage")
    if not (t5 / "Manifest.json").is_file():
        Path("coreml").mkdir(exist_ok=True)
        shutil.rmtree(part, ignore_errors=True)
        if s.other and (s.other / t5 / "Manifest.json").is_file():
            s.copy(s.other / t5, part)
        else:
            # torch, transformers and t5-base's weights only for this one conversion, in uv's throwaway environment and
            # a Hugging Face cache that's deleted after: none of it stays in the install
            s.say("converting t5-base's encoder to Core ML at fp32 (a one-time ~1 GB download, deleted after)")
            try:
                s.run([s.uv, "run", "-q", "--no-project", "--python", "3.11", "--with", "torch==2.7.1", "--with", "transformers==5.19.0",
                       "--with", "coremltools==9.0", "--with", "numpy==2.4.6", "python", "-I", HERE / "t5_to_coreml.py", "--revision", T5REV,
                       "--tokenizer", "t5-tokenizer/tokenizer.json", "--out", part], env={**os.environ, "HF_HOME": str(s.dir / ".t5-hf")}, **low())
            finally:
                shutil.rmtree(s.dir / ".t5-hf", ignore_errors=True)
        os.replace(part, t5)

    shutil.copy2(HERE / "run.py", "run.py")
    s.ready("coreml-ref", "coreml", ".venv")
