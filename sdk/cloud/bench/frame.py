# One Blender frame timed end to end (bench/gpus.py): the Open Data benchmark's classroom scene at 1920x1080 with its
# own samples, denoised, on the launcher's Blender. Blender's start and the scene's load and BVH build are timed apart
# from the render, since a frame split across many GPUs pays them once per container.

import os
import re
import subprocess
import time

SETUP = """
import bpy, time
s = bpy.context.scene
s.render.resolution_x, s.render.resolution_y, s.render.resolution_percentage = 1920, 1080, 100
s.cycles.use_denoising = True
p = bpy.context.preferences.addons["cycles"].preferences
p.compute_device_type = "{kind}" if "{kind}" != "CPU" else "NONE"
p.get_devices()
for d in p.devices:
    d.use = d.type == "{kind}"
s.cycles.device = "GPU" if "{kind}" != "CPU" else "CPU"
print("RAEA_SAMPLES", s.cycles.samples, "RAEA_LOADED", time.time())
"""


def classroom(version, kind):
    # the launcher keeps them in its cache folder, wherever its build put it
    hits = subprocess.run(["find", "/", "-xdev", "(", "-name", "blender", "-type", "f", "-o", "-name", "*.blend", ")"], capture_output=True, text=True).stdout.split()
    exe = next((h for h in hits if h.endswith("/blender") and version in h), None) or next((h for h in hits if h.endswith("/blender")), None)
    blend = next((h for h in hits if h.endswith("/classroom.blend")), None)
    if not exe or not blend:
        return {"error": "the launcher's Blender or classroom scene isn't in the image", "exe": exe, "blend": blend, "found": hits[:20]}
    t = time.time()
    r = subprocess.run(
        [exe, "-b", blend, "-E", "CYCLES", "--python-expr", SETUP.format(kind=kind), "-o", "/tmp/frame_####", "-F", "PNG", "-f", "1"],
        capture_output=True, text=True, timeout=1800,
    )
    total = time.time() - t
    out = r.stdout + r.stderr
    loaded = re.search(r"RAEA_LOADED ([\d.]+)", out)
    samples = re.search(r"RAEA_SAMPLES (\d+)", out)
    render = re.findall(r"Time: (\d+):([\d.]+)", out)
    render_s = int(render[-1][0]) * 60 + float(render[-1][1]) if render else None
    return {
        "ok": r.returncode == 0 and os.path.exists("/tmp/frame_0001.png"),
        "samples": int(samples.group(1)) if samples else None,
        "load_s": round(float(loaded.group(1)) - t, 2) if loaded else None,
        "render_s": render_s,
        "total_s": round(total, 2),
        "out": out[-2000:],
    }
