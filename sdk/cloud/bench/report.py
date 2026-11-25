# bench/results/*.json as the cost tables in docs/cloud-render.md: python3 bench/report.py from sdk/cloud.
# Prices are a container of 4 cores and 16 GiB (what the farm gives each GPU) at Modal's list price.

import glob
import json
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "deploy", "modal"))
import prices  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ORDER = ["none", "T4", "L4", "A10", "L40S", "RTX-PRO-6000", "A100-40GB", "H100"]


def ms(r, name):
    x = r["raea"].get(name) or {}
    lines = x.get("out") if isinstance(x.get("out"), list) else [x.get("out") or ""]
    for l in lines:
        m = re.search(r"\(([\d.]+) ms a frame", l)
        if m:
            return float(m.group(1))
    return None


rows = {}
for f in glob.glob(os.path.join(HERE, "results", "*.json")):
    r = json.load(open(f))
    if not r.get("error"):
        rows[r["kind"]] = r

print("| GPU | $/h (4 cores, 16 GiB) | cold start | Cycles samples/min (monster, junkshop, classroom) | classroom samples per $ | edit.tsx ms/frame (clip check on, off) | Solari ms/frame (on, off) | $ per 1,000 Solari frames |")
print("|---|---|---|---|---|---|---|---|")
for k in [k for k in ORDER if k in rows]:
    r = rows[k]
    hour = prices.per_hour("" if k == "none" else k, 8 if k == "none" else 4, 16)
    spm = r["blender"].get("samples_per_minute") or {}
    per_dollar = spm.get("classroom", 0) * 60 / hour if spm else 0
    sol = ms(r, "solari-noclip")
    name = "CPU only (8 cores)" if k == "none" else k
    print(
        f"| {name} | ${hour:.2f} | {r['start']:.0f} s | {spm.get('monster', 0):,.0f}, {spm.get('junkshop', 0):,.0f}, {spm.get('classroom', 0):,.0f} "
        f"| {per_dollar:,.0f} | {ms(r, 'edit') or 0:,.0f}, {ms(r, 'edit-noclip') or 0:,.0f} | {ms(r, 'solari') or 0:.0f}, {sol or 0:.0f} "
        f"| ${(sol or 0) / 1000 * 1000 * hour / 3600:.3f} |"
    )
