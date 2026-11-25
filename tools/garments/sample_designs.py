# The garment data engine's first step: random GarmentCode designs, cut to SOMA's measurements, each a folder with
# its design (the labels), a caption in words, the pattern and the flat pieces for the drape.
#   python sample_designs.py GARMENTCODE_SRC SOMA.yaml OUT_DIR N [SEED]
#   DESIGN=design.yaml python sample_designs.py ... OUT_DIR 1     that design, not a random one (garment_model.py make)
# OUT_DIR/<id>/{design.yaml, caption.txt, <id>_specification.json, <id>.panels.json}
import sys, json, random, string, traceback
from pathlib import Path
import numpy as np, yaml

src, body_yaml, out, N = sys.argv[1], sys.argv[2], Path(sys.argv[3]), int(sys.argv[4])
seed = int(sys.argv[5]) if len(sys.argv) > 5 else 1
sys.path.insert(0, src)
import pygarment as pyg
from pattern_sampler import assert_param_combinations
from assets.garment_programs.meta_garment import MetaGarment
from assets.bodies.body_params import BodyParameters
from pygarment.meshgen.boxmeshgen import BoxMesh

random.seed(seed); np.random.seed(seed)
body = BodyParameters(body_yaml)
sampler = pyg.DesignSampler(str(Path(src) / "assets/design_params/default.yaml"))
v = lambda d, *p: (lambda n: n["v"])(__import__("functools").reduce(lambda a, k: a[k], p, d))

def caption(d):
    """the design in plain words, from its settings (a first caption; a picture's caption can join it later)"""
    words = []
    up, low, wb = v(d, "meta", "upper"), v(d, "meta", "bottom"), v(d, "meta", "wb")
    if up:
        sleeveless = v(d, "sleeve", "sleeveless")
        sl = "sleeveless" if sleeveless else ("long-sleeved" if v(d, "sleeve", "length") > 0.7 else "short-sleeved")
        coll = v(d, "collar", "component", "style")
        neck = {"Hood2Panels": "with a hood", "Turtle": "with a turtle neck", "SimpleLapel": "with lapels"}.get(coll, "")
        if v(d, "collar", "fc_depth") > 1.0: neck = (neck + " and" if neck else "with") + " a deep neckline"
        fit = "fitted" if up == "FittedShirt" else ("loose" if v(d, "shirt", "width") > 1.1 else "")
        if up == "FittedShirt" and v(d, "shirt", "strapless"): sl, neck, sleeveless = "strapless", "", True
        length = "long" if v(d, "shirt", "length") > 1.4 else ("cropped" if v(d, "shirt", "length") < 1.0 else "")
        cuff = "with cuffs" if v(d, "sleeve", "cuff", "type") and not sleeveless else ""
        asym = "asymmetric" if v(d, "left", "enable_asym") else ""
        words.append(" ".join(x for x in [length, fit, asym, sl, "top", neck, cuff] if x))
    if low:
        kind = {"Pants": "trousers", "PencilSkirt": "pencil skirt", "SkirtCircle": "circle skirt", "AsymmSkirtCircle": "asymmetric circle skirt",
                "GodetSkirt": "godet skirt", "Skirt2": "skirt", "SkirtManyPanels": "panelled skirt", "SkirtLevels": "tiered skirt"}[low]
        words.append(kind)
    if wb: words.append("a gathered waistband" if v(d, "waistband", "waist") > 1.15 else "a waistband")
    return ", ".join(words)

GIVEN = __import__("os").environ.get("DESIGN")
made, tries = 0, 0
while made < N and tries < (1 if GIVEN else N * 20):
    tries += 1
    d = yaml.safe_load(open(GIVEN))["design"] if GIVEN else sampler.randomize()
    gid = __import__("os").environ.get("GID") or "g" + "".join(random.choices(string.ascii_lowercase + string.digits, k=8))
    folder = out / gid
    try:
        assert_param_combinations(d)
        garment = MetaGarment(gid, body, d)
        garment.assert_total_length()
        if garment.is_self_intersecting(): raise ValueError("self-intersecting panels")
        folder.mkdir(parents=True, exist_ok=True)
        pattern = garment.assembly()
        pattern.serialize(folder, tag="", to_subfolder=False, with_3d=False, with_text=False, view_ids=False, with_printable=False)
        box = BoxMesh(next(folder.glob(f"{gid}*specification.json")), 1.0); box.load()
        pieces, index = [], {}
        for pname, panel in box.panels.items():
            v2 = np.asarray(panel.panel_vertices, float); v3 = np.asarray(panel.rot_trans_panel(v2), float)
            index[pname] = len(pieces)
            labels = {}  # GarmentCode's edge labels (lower_interface, right_collar, left_collar, strapless_top) as piece vertices
            for edge in panel.edges:
                if getattr(edge, "label", ""):
                    labels.setdefault(edge.label, []).extend(int(i) for i in edge.vertex_range)
            pieces.append({"name": pname, "flat": v2.round(4).tolist(), "placed": v3.round(4).tolist(),
                           "faces": np.asarray(panel.panel_faces, int).tolist(), "labels": labels})
        seams = []
        for locs in box.verts_glob_loc:
            locs = sorted(set(locs))
            seams += [[index[a[0]], int(a[1]), index[b[0]], int(b[1])] for a, b in zip(locs, locs[1:])]
        # the bottoms' rise (1 at the waist, 0 at the hips): where the waistband's pin goes
        rise = next((float(s.get_rise()) for s in garment.subs if hasattr(s, "get_rise")), 1.0)
        json.dump({"name": gid, "units": "cm", "rise": rise, "pieces": pieces, "seams": seams}, open(folder / f"{gid}.panels.json", "w"))
        yaml.safe_dump({"design": d}, open(folder / "design.yaml", "w"), sort_keys=False)
        (folder / "caption.txt").write_text(caption(d) + "\n")
        made += 1
        print(f"sample: {gid} {sum(len(p['flat']) for p in pieces)} vertices, {len(pieces)} pieces: {caption(d)}", flush=True)
    except BaseException as e:  # GarmentCode's IncorrectElementConfiguration is a BaseException
        if isinstance(e, KeyboardInterrupt): raise
        # a design rejected after its pattern was written leaves no folder behind (one with no panels.json is not a design)
        if folder.is_dir() and not (folder / "design.yaml").exists(): __import__("shutil").rmtree(folder)
        print(f"sample: rejected ({type(e).__name__}: {str(e)[:80]})", flush=True)
print(f"sample: {made} designs from {tries} tries -> {out}")
