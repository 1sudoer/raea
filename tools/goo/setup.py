"""Goo for raea's physics (<Goo>, the `physics.goo` native call): objects melted into viscous goo by an MLS-MPM solver
(Taichi: Metal on a Mac, CUDA or Vulkan elsewhere, else the CPU; Apache-2.0), each frame's surface reconstructed
(marching cubes, scikit-image: BSD) into a mesh sequence (.rseq) the engine plays back by frame; thin liquids
("solver": "sph") by SPlisHSPlasH (MIT) instead. run.py and sph.py are copied in beside the venv. Installs into
$RAEA_GOO (default ~/.cache/raea-goo); raea runs this itself the first time a goo bake is asked for
(tools/local_setup.py: how, on any OS). pySPlisHSPlasH is pinned: 2.19.0 hangs after its first step on a Mac
(2026-10-20); 2.17.0 runs. Size: about 600 MB (the venv). No model weights."""

import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from local_setup import Setup  # noqa: E402

with Setup("goo", "RAEA_GOO", "raea-goo") as s:
    Path(".ready").unlink(missing_ok=True)
    # uv's own Python, never a framework build (Homebrew's): SPlisHSPlasH finds its emitters' meshes beside the running
    # executable (the venv's bin/resources), and a framework Python runs as Python.app inside its own install
    framework = "import sys, sysconfig; sys.exit(1 if sysconfig.get_config_var('PYTHONFRAMEWORK') else 0)"
    if s.python.exists() and subprocess.run([str(s.python), "-c", framework]).returncode != 0:
        s.say("remaking the venv on uv's Python (this one is a framework build)")
        shutil.rmtree(".venv")
    s.venv("3.12", ["taichi", "numpy", "scipy", "trimesh", "scikit-image", "zstandard", "pillow", "pySPlisHSPlasH==2.17.0"],
           imports=("taichi", "numpy", "scipy", "trimesh", "skimage", "zstandard", "PIL", "pysplishsplash"), managed=True)
    s.run([s.python, "-c", "import taichi, numpy, trimesh, skimage; print('taichi', taichi.__version__, 'numpy', numpy.__version__, 'trimesh', trimesh.__version__, 'skimage', skimage.__version__)"],
          stdout=sys.stderr)
    for f in ("run.py", "sph.py"):
        shutil.copy2(HERE / f, f)
    s.ready(".venv")
