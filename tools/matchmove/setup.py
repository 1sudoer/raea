"""Matchmove (camera solve) for raea-render's asset store: `Store::matchmove(clip, {from, seconds, subject, height})`
solves the camera of a stretch of footage (COLMAP through pycolmap: BSD; OpenCV for the rotation-only fallback:
Apache-2.0) and keeps the solve as JSON, cached by the clip's content and the params. run.py is copied in beside the
venv; it reads the footage through PyAV (BSD; its wheels carry FFmpeg's own libraries), so no FFmpeg need be installed.
Installs into $RAEA_MATCHMOVE (default ~/.cache/raea-matchmove); raea-render runs this the first time a solve is asked
for and nothing is installed (a first `raea track`, a <Matchmove>), on any OS (tools/local_setup.py). Size: the venv
only (pycolmap, numpy, opencv-python-headless, pillow, av: about 300 MB). No model weights."""

import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from local_setup import FOOTAGE, Setup  # noqa: E402

with Setup("matchmove", "RAEA_MATCHMOVE", "raea-matchmove") as s:
    s.venv("3.11", FOOTAGE, imports=("pycolmap", "numpy", "cv2", "PIL", "av"))
    shutil.copy2(HERE / "run.py", "run.py")
    s.ready(".venv")
