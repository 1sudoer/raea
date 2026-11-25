"""Splat training for raea-render's asset store: `Store::splat(input, {frames, steps, ...})` makes a 3D Gaussian splat
from footage (frames read through PyAV, then COLMAP through pycolmap: BSD, then Brush: Apache-2.0, Rust + wgpu: Metal on
a Mac, Vulkan or DX12 elsewhere) and keeps the .ply and its sidecar, cached by the input's content and the params.
run.py is copied in beside the venv; Brush is its v0.3.0 release build for this OS, checked against its SHA-256.
Installs into $RAEA_SPLAT (default ~/.cache/raea-splat); raea-render runs this the first time a splat is asked for, and
`raea splat setup` runs it on purpose, on any OS (tools/local_setup.py). Size: about 360 MB (the venv, Brush's binary).
No model weights."""

import platform
import shutil
import sys
import tarfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from local_setup import FOOTAGE, WINDOWS, Setup  # noqa: E402

BRUSH = "v0.3.0"
# Brush's release builds raea runs, by (OS, machine), and their SHA-256 (the release's sha256.sum)
BUILDS = {
    ("Darwin", "arm64"): ("brush-app-aarch64-apple-darwin.tar.xz", "65b2631398c839be3c1d4d7160fe2326389dec87830aac0710985e6690a1048c"),
    ("Linux", "x86_64"): ("brush-app-x86_64-unknown-linux-gnu.tar.xz", "4f0f9a8785d1951c62df26aae247c02c5bba32b00f40b06df4e1c9b867399e20"),
    ("Windows", "AMD64"): ("brush-app-x86_64-pc-windows-msvc.zip", "b68e3e9cf052d51bf3ee30776fa5a364de7f2ba13b58443128ff797bb7bcfcd6"),
}

with Setup("splat", "RAEA_SPLAT", "raea-splat") as s:
    s.venv("3.11", FOOTAGE, imports=("pycolmap", "numpy", "cv2", "PIL", "av"))
    exe = Path("brush") / ("brush_app.exe" if WINDOWS else "brush_app")
    version = Path("brush/.version")
    if not exe.is_file() or not version.is_file() or version.read_text().strip() != BRUSH:
        build = BUILDS.get((platform.system(), platform.machine()))
        if not build:
            sys.exit(f"splat setup: Brush {BRUSH} has no release build for {platform.system()} {platform.machine()}: build it "
                     f"(cargo install --git https://github.com/ArthurBrussee/brush --tag {BRUSH} brush-app) and put brush_app in {s.dir / 'brush'}")
        name, sha = build
        s.say(f"fetching Brush {BRUSH}")
        shutil.rmtree("brush.part", ignore_errors=True)
        s.fetch(f"brush.part/{name}", f"https://github.com/ArthurBrussee/brush/releases/download/{BRUSH}/{name}", sha)
        if name.endswith(".zip"):
            with zipfile.ZipFile(f"brush.part/{name}") as z:
                z.extractall("brush.part")
        else:
            with tarfile.open(f"brush.part/{name}") as t:
                t.extractall("brush.part", **({"filter": "data"} if hasattr(tarfile, "data_filter") else {}))
        shutil.rmtree("brush", ignore_errors=True)
        Path("brush").mkdir()
        found = [p for p in Path("brush.part").rglob(exe.name) if p.is_file()]
        if not found:
            sys.exit(f"splat setup: {name} has no {exe.name}")
        shutil.move(found[0], exe)
        for lic in Path("brush.part").rglob("LICENSE*"):
            shutil.move(lic, Path("brush") / lic.name)
            break
        exe.chmod(0o755)
        version.write_text(BRUSH + "\n")
        shutil.rmtree("brush.part", ignore_errors=True)
    s.run([exe.resolve(), "--version"], stdout=sys.stderr)
    shutil.copy2(HERE / "run.py", "run.py")
    s.ready(".venv", "brush")
