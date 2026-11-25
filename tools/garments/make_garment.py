# A GarmentCode garment for a measured body, written as flat pieces for a drape of our own.
#   python make_garment.py GARMENTCODE_SRC BODY.yaml DESIGN.yaml NAME OUT_DIR [RES_CM]
# OUT_DIR/NAME_specification.json is GarmentCode's pattern; OUT_DIR/NAME.panels.json is, per piece, its flat
# vertices (cm, the cut), its triangles and its place around the body (3D, cm, Y up, facing +Z), and the seams as
# pairs of piece vertices to be sewn together.
import sys, json
from pathlib import Path
import numpy as np, yaml

src, body_yaml, design_yaml, name, out = sys.argv[1:6]
res = float(sys.argv[6]) if len(sys.argv) > 6 else 1.5
sys.path.insert(0, src)
from assets.garment_programs.meta_garment import MetaGarment
from assets.bodies.body_params import BodyParameters
from pygarment.meshgen.boxmeshgen import BoxMesh

body = BodyParameters(body_yaml)
design = yaml.safe_load(open(design_yaml))["design"]
garment = MetaGarment(name, body, design)
pattern = garment.assembly()
if garment.is_self_intersecting(): print(f"{name}: self-intersecting panels")
folder = Path(pattern.serialize(Path(out), tag="", to_subfolder=False, with_3d=False, with_text=False,
                                view_ids=False, with_printable=False))
spec = next(folder.glob(f"{name}*specification.json"))
box = BoxMesh(spec, res)
box.load()

pieces, index = [], {}
for pname, panel in box.panels.items():
    v2 = np.asarray(panel.panel_vertices, dtype=float)
    v3 = np.asarray(panel.rot_trans_panel(v2), dtype=float)
    index[pname] = len(pieces)
    pieces.append({"name": pname, "flat": v2.round(4).tolist(), "placed": v3.round(4).tolist(),
                   "faces": np.asarray(panel.panel_faces, dtype=int).tolist()})
seams = []
for locs in box.verts_glob_loc:
    locs = sorted(set(locs))
    for a, b in zip(locs, locs[1:]):
        seams.append([index[a[0]], int(a[1]), index[b[0]], int(b[1])])
json.dump({"name": name, "units": "cm", "pieces": pieces, "seams": seams}, open(folder / f"{name}.panels.json", "w"))
print(f"{name}: {len(pieces)} pieces, {sum(len(p['flat']) for p in pieces)} vertices, {len(seams)} seam pairs -> {folder}")
