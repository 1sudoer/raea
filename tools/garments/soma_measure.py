# SOMA's body measured in GarmentCode's terms (docs/Body Measurements GarmentCode.pdf), in cm.
#   python soma_measure.py BODY.glb SKELETON.json NEUTRAL.yaml OUT.yaml [TEMPLATE_MEASURED.yaml]
# TEMPLATE_MEASURED: this tool's own reading of the template body (the one NEUTRAL was written for). NEUTRAL holds a
# few values set by hand where GarmentCode means something other than the tape (soma.yaml: neck_w 18.5 where the
# template's neck reads 10.8, shoulder_w 35.5 for 32.2, bust 89.9 for 91.9); given it, a body of another build
# (tools/clothes/body.py SHAPE=...) gets each such value scaled by its own reading over the template's.
# Girths are convex-hull perimeters of the torso's horizontal slice (a tape measure lies over hollows).
# The geodesic lengths (waist_line, bust lines) are read along the body's back or front midline profile.
# What isn't measured here is the neutral body's value scaled by height, and printed as such.
import json, sys, math
import numpy as np, trimesh, yaml
from scipy.spatial import ConvexHull

glb, skel_path, neutral_path, out = sys.argv[1:5]
scene = trimesh.load(glb, process=False)
meshes = list(scene.geometry.values()) if isinstance(scene, trimesh.Scene) else [scene]
body = max(meshes, key=lambda m: len(m.vertices))
V = np.asarray(body.vertices)
J = {j["name"]: np.array(j["head"]) for j in json.load(open(skel_path))["joints"]}
neutral = yaml.safe_load(open(neutral_path))["body"]

def polys(y):
    sec = body.section(plane_origin=[0, y, 0], plane_normal=[0, 1, 0])
    if sec is None: return []
    out = []
    for ent in sec.discrete:  # closed loops in 3D
        out.append(np.asarray(ent)[:, [0, 2]])
    return out

def hull_perim(p):
    h = ConvexHull(p); q = p[h.vertices]
    return float(np.sum(np.linalg.norm(q - np.roll(q, 1, axis=0), axis=1)))

def torso(y):
    """the loop at height y around x=0 (the torso), or None"""
    best = None
    for p in polys(y):
        if len(p) < 8: continue
        if p[:, 0].min() < 0 < p[:, 0].max():
            if best is None or len(p) > len(best): best = p
    return best

H = V[:, 1].max() - V[:, 1].min(); floor = V[:, 1].min()
m = {}
m["height"] = H * 100
nape = J["Neck1"][1]
m["head_l"] = (V[:, 1].max() - nape) * 100

# waist: the narrowest torso girth between the hips joint and the chest joint
ys = np.linspace(J["Hips"][1] + 0.03, J["Chest"][1], 40)
g = [(hull_perim(t), y) for y in ys if (t := torso(y)) is not None]
waist, waist_y = min(g); m["waist"] = waist * 100
# bust: the torso girth at the chest's most forward level, below the armpits
ys = np.linspace(J["Chest"][1] - 0.08, J["Chest"][1] + 0.12, 40)
front = []
for y in ys:
    t = torso(y)
    if t is None: continue
    if t[:, 0].max() - t[:, 0].min() > 0.45: continue  # the arms joined in: above the armpit
    front.append((t[:, 1].max(), y, hull_perim(t), t))
fz, bust_y, bust, bt = max(front, key=lambda r: r[0])
# the girth is the chest at its fullest, under the arms, as a tailor takes it: on a man that is above the most forward
# level (SOMA: 89.9 cm against 81 there), and a fitted top cut to the smaller one is squeezed down off the chest.
# bust_y stays at the forward level, where the darts aim.
full = max(front, key=lambda r: r[2]); m["bust"] = full[2] * 100; bt = full[3]  # back_width is read there too
ub = torso(bust_y - 0.07); m["underbust"] = hull_perim(ub) * 100
# hips: the widest girth between the crotch and the hips joint, where the slice is still one loop
crotch_y = V[(np.abs(V[:, 0]) < 0.01) & (V[:, 1] > 0.5), 1].min()
for y in []:
    t = torso(y)
    if t is None or not (t[:, 0].min() < -0.03 and t[:, 0].max() > 0.03 and len(polys(y)) >= 1 and abs(t[:, 0].mean()) < 0.02):
        crotch_y = y; break
    loops = [p for p in polys(y) if len(p) > 8 and p[:, 0].max() - p[:, 0].min() > 0.08]
    if len(loops) >= 2: crotch_y = y; break
ys = np.linspace(crotch_y + 0.01, J["Hips"][1], 30)
g = [(hull_perim(t), y, t) for y in ys if (t := torso(y)) is not None]
hips, hip_y, ht = max(g, key=lambda r: r[0]); m["hips"] = hips * 100
m["hips_line"] = (waist_y - hip_y) * 100
m["crotch_hip_diff"] = (hip_y - crotch_y) * 100
wt = torso(waist_y)
m["hip_inclination"] = math.degrees(math.atan2(ht[:, 0].max() - wt[:, 0].max(), waist_y - hip_y))
# back widths: the hull's back half between the balance lines (the slice's x extremes)
def back_width(t):
    h = ConvexHull(t); q = t[h.vertices]
    zc = (q[np.argmin(q[:, 0]), 1] + q[np.argmax(q[:, 0]), 1]) / 2
    seg = 0.0
    for a, b in zip(q, np.roll(q, -1, axis=0)):
        if a[1] < zc and b[1] < zc: seg += np.linalg.norm(a - b)
        elif a[1] < zc or b[1] < zc: seg += 0.5 * np.linalg.norm(a - b)
    return seg * 100
m["waist_back_width"], m["back_width"], m["hip_back_width"] = back_width(wt), back_width(bt), back_width(ht)
# thigh: the largest single-leg loop just under the crotch
legs = []
for y in np.linspace(crotch_y - 0.01, crotch_y - 0.08, 8):
    for p in polys(y):
        if len(p) > 8 and p[:, 0].mean() > 0.02: legs.append(hull_perim(p))
m["leg_circ"] = max(legs) * 100
# arm: shoulder joint to wrist, and its angle down from the horizontal
sh, el, wr = J["LeftArm"], J["LeftForeArm"], J["LeftHand"]
m["arm_length"] = float(np.linalg.norm(wr - sh)) * 100
d = el - sh; m["arm_pose_angle"] = math.degrees(math.atan2(-d[1], abs(d[0])))
# wrist: the loop of a plane across the forearm at the wrist
n = (wr - el) / np.linalg.norm(wr - el)
sec = body.section(plane_origin=wr - n * 0.01, plane_normal=n)
loops = [np.asarray(e) for e in sec.discrete]
lp = min(loops, key=lambda l: np.linalg.norm(l.mean(axis=0) - wr))
basis = np.linalg.svd(lp - lp.mean(axis=0))[2][:2]
m["wrist"] = hull_perim((lp - lp.mean(axis=0)) @ basis.T) * 100
# shoulders and neck
m["shoulder_w"] = 2 * abs(sh[0]) * 100
nt = torso(J["Neck2"][1])
m["neck_w"] = (nt[:, 0].max() - nt[:, 0].min()) * 100
top = lambda x: V[(np.abs(V[:, 0] - x) < 0.01) & (np.abs(V[:, 2] - sh[2]) < 0.05), 1].max()
x0, x1 = m["neck_w"] / 200, abs(sh[0])

m["vert_bust_line"] = (nape - bust_y) * 100
# geodesics along the midline profiles
def profile(x, y0, y1, side):
    pts = []
    for y in np.linspace(y0, y1, 60):
        sel = V[(np.abs(V[:, 0] - x) < 0.012) & (np.abs(V[:, 1] - y) < 0.006)]
        if len(sel): pts.append([y, sel[:, 2].max() if side > 0 else sel[:, 2].min()])
    p = np.array(pts); return float(np.sum(np.linalg.norm(np.diff(p, axis=0), axis=1))) * 100
m["waist_line"] = (nape - waist_y) * 100 * 1.06  # vertical, plus the back's curve

scale = m["height"] / neutral["height"]
for k, v in neutral.items():
    if k not in m:
        m[k] = v * scale; print(f"scaled from the neutral body: {k}")
if len(sys.argv) > 5:
    tm = yaml.safe_load(open(sys.argv[5]))["body"]
    for k, v in neutral.items():
        if k in tm and tm[k] and abs(v / tm[k] - 1) > 0.01 and k in m:
            print(f"set by hand on the template, scaled: {k} {m[k]:.2f} -> {v * m[k] / tm[k]:.2f}"); m[k] = v * m[k] / tm[k]
for k in sorted(m): print(f"{k:22s} SOMA {m[k]:7.2f}   neutral {neutral[k]:7.2f}")
yaml.safe_dump({"body": {k: round(float(v), 4) for k, v in sorted(m.items())}}, open(out, "w"))
print("wrote", out)
