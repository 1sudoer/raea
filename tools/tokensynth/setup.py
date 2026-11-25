"""TokenSynth (MIT code and weights: github.com/KyungsuKim42/tokensynth, huggingface.co/KyungsuKim/TokenSynth) for
raea-render's asset store: `make("tokensynth", timbre, seed, {midi, velocity, seconds, precision})` renders one note
(torch on the CPU, or CUDA where asked). Installs the code at a pinned commit, the four fp32 weights (about 4 GB) and a
venv into $RAEA_TOKENSYNTH (default ~/.cache/raea-tokensynth); convert.py then makes the smaller runtime run.py defaults
to (weights-fp16/, 1.0 GB) and, on a Mac, the Core ML models (coreml/). raea-render runs this the first time a note is
asked for, and `raea setup-audio` runs it on purpose, on any OS (tools/local_setup.py). Another install's files are
copied rather than downloaded ($RAEA_TOKENSYNTH_FROM, else ~/.cache/raea-tokensynth when installing elsewhere)."""

import shutil
import sys
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from local_setup import MAC, Setup, low  # noqa: E402

# the code at the commit the spike ran (2026-10-18; the repo's last commit, 2026-06-24), as GitHub's archive: no git needed
COMMIT = "be7f5fbeef93a8a59b203945ccbabb7c9aa4f57e"
WEIGHTS = ["token_synth_aug.pt", "token_synth_unconditional.pt", "dac_weights_44khz_8kbps_0.0.1.pt", "clap_music_audioset_epoch_15_esc_90.14.pt"]

with Setup("tokensynth", "RAEA_TOKENSYNTH", "raea-tokensynth") as s:
    for d in ("weights", "hf"):
        Path(d).mkdir(exist_ok=True)
    if not Path("repo").is_dir():
        if s.other and (s.other / "repo").is_dir():
            s.copy(s.other / "repo", "repo")
        else:
            s.fetch("repo.zip", f"https://github.com/KyungsuKim42/tokensynth/archive/{COMMIT}.zip")
            with zipfile.ZipFile("repo.zip") as z:
                z.extractall("repo.part")
            shutil.move(next(Path("repo.part").iterdir()), "repo")
            shutil.rmtree("repo.part")
            Path("repo.zip").unlink()
            Path("repo.commit").write_text(COMMIT + "\n")
    # the versions the spike ran (2026-10-18): laion-clap 1.1.6 needs transformers 4.30; scipy 1.15's wheel wouldn't
    # load on a Mac. coremltools only on a Mac, for convert.py's Core ML models (run.py --precision coreml)
    packages = ["torch==2.5.1", "torchaudio==2.5.1", "transformers==4.30.2", "scipy==1.14.1", *(["coremltools==8.3.0"] if MAC else []),
                "laion-clap", "descript-audio-codec", "pretty_midi", "soundfile", "numpy", "-e", "repo"]
    s.venv("3.11", packages, imports=("torch", "laion_clap", "dac", "tokensynth", *(["coremltools"] if MAC else [])), what="making the venv (about 1.1 GB)")
    for f in WEIGHTS:
        s.fetch(f"weights/{f}", f"https://huggingface.co/KyungsuKim/TokenSynth/resolve/main/{f}")
    # the converted runtime from the other install, when it has it whole: convert.py then finds everything made
    for d in ("weights-fp16", "coreml"):
        if not Path(d).exists() and s.other and (s.other / d).is_dir():
            s.copy(s.other / d, d)
    shutil.copy2(HERE / "run.py", "run.py")
    # the smaller runtime run.py's other precisions read (weights-fp16/, coreml/); keeps what is already made
    s.run([s.python, "-I", HERE / "convert.py", "--dir", s.dir], **low())
    s.ready("weights", "weights-fp16", ".venv")
