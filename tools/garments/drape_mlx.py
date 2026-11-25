# drape_pbd.py's cloth on the GPU (MLX), for the garment data engine: no Blender in the loop. The body is a signed
# distance grid (sdf_build.py) instead of a BVH; everything is in glTF's frame (metres, y up, +z ahead).
#   python drape_mlx.py SDF.npz PANELS.json OUT_DIR [FRAMES] [SHIFT_Z_M]
# env as drape_pbd.py: LIFT 0.07, STRETCH 0.9, SQUASH 0.25, BEND 0.05, SUB 10, ITERS 10, FRICTION 0.4, THICK 0.004,
# OMEGA 1.7, REACH 0.05 (only cloth less than this deep in the body is pushed out: a deeper point is on the far side)
# OUT_DIR/<name>.draped.npz holds X (draped, m), F, FLAT (the cut, m), seams, start; <name>.drape.json the report.
import sys, os, json, time
import numpy as np
import mlx.core as mx

sdf_path, panels_path, out_dir = sys.argv[1:4]
FRAMES = int(sys.argv[4]) if len(sys.argv) > 4 else 150
SHIFT_Z = float(sys.argv[5]) if len(sys.argv) > 5 else -0.064
BODY_YAML = sys.argv[6] if len(sys.argv) > 6 else None  # the measurements the pattern was cut to: turns the pins on
E = lambda k, d: float(os.environ.get(k, d))
LIFT, STRETCH, SQUASH, BEND = E("LIFT", 0.07), E("STRETCH", 0.9), E("SQUASH", 0.6), E("BEND", 0.3)  # 0.25 and 0.05 buckled at the mesh scale
# ITERS 60: at 10 the cloth stretched +18% (p95) under gravity, armholes opened and tops slid off the shoulders; 60 gives +5%
SUB, ITERS, FRICTION, THICK = int(E("SUB", 10)), int(E("ITERS", 60)), E("FRICTION", 0.4), E("THICK", 0.004)
OMEGA, REACH = E("OMEGA", 1.7), E("REACH", 0.05)
os.makedirs(out_dir, exist_ok=True)
P = json.load(open(panels_path)); NAME = P["name"]

# the body
S = np.load(sdf_path)
dims = np.array(S["sdf"].shape); origin = mx.array(S["origin"]); cell = float(S["cell"])
SDF = mx.array(S["sdf"].ravel()); GRAD = mx.array(S["grad"].reshape(-1, 3))
hi = mx.array((dims - 1.001).astype(np.float32)); ny, nz = int(dims[1]), int(dims[2])

def body(p):
    """distance to the skin (negative inside) and the way out, trilinear in the grid"""
    u = mx.clip((p - origin) / cell, 0, hi)
    i0 = mx.floor(u); f = u - i0; i0 = i0.astype(mx.int32)
    s = mx.zeros(p.shape[0]); g = mx.zeros(p.shape)
    for dx in (0, 1):
        wx = f[:, 0] if dx else 1 - f[:, 0]
        for dy in (0, 1):
            wy = wx * (f[:, 1] if dy else 1 - f[:, 1])
            for dz in (0, 1):
                w = wy * (f[:, 2] if dz else 1 - f[:, 2])
                idx = ((i0[:, 0] + dx) * ny + (i0[:, 1] + dy)) * nz + (i0[:, 2] + dz)
                s = s + w * mx.take(SDF, idx)
                g = g + w[:, None] * mx.take(GRAD, idx, axis=0)
    return s, g / (mx.sqrt(mx.sum(g * g, axis=1, keepdims=True)) + 1e-9)

# the pieces, placed round the body, and the flat cut
LIFT_LOW = E("LIFT_LOW", 0.01)  # skirts, trousers and waistbands start nearer their place than tops
# sleeves and cuffs start round the arm, where GarmentCode places them: lifted as a top's torso is (to fall onto the
# shoulders), a woman's narrower sleeve (SOMA's woman build, 11-17) began with its under-seam 2 cm below the arm's
# centreline, inside the arm, and draped as two flaps beside it; its seams to the armhole close the 6 cm between
SLEEVE_LIFT = E("SLEEVE_LIFT", 0.01)
X, F, FLAT, start = [], [], [], []
for piece in P["pieces"]:
    start.append(len(X))
    lift = LIFT_LOW if piece["name"].startswith(("skirt", "wb", "pant", "level")) else SLEEVE_LIFT if ("sleeve" in piece["name"] or "cuff" in piece["name"]) else LIFT
    X += [(p[0] / 100, p[1] / 100 + lift, p[2] / 100 + SHIFT_Z) for p in piece["placed"]]
    FLAT += [(q[0] / 100, q[1] / 100) for q in piece["flat"]]
    F += [[start[-1] + i for i in f] for f in piece["faces"]]
X = np.array(X, np.float32); FLAT = np.array(FLAT, np.float32); F = np.array(F, np.int32); n = len(X)
seams = np.array([(start[pa] + ia, start[pb] + ib) for pa, ia, pb, ib in P["seams"]], np.int32).reshape(-1, 2)
seams = seams[seams[:, 0] != seams[:, 1]]
opp = {}
for f in F:
    for k in range(3):
        i, j, o = int(f[k]), int(f[(k + 1) % 3]), int(f[(k + 2) % 3])
        opp.setdefault((min(i, j), max(i, j)), []).append(o)
edges = np.array(list(opp.keys()), np.int32)
bends = np.array([v for v in opp.values() if len(v) == 2], np.int32).reshape(-1, 2)
rest_e = np.linalg.norm(FLAT[edges[:, 0]] - FLAT[edges[:, 1]], axis=1)
rest_b = np.linalg.norm(FLAT[bends[:, 0]] - FLAT[bends[:, 1]], axis=1)
# bends across the seams too, each at its two pieces laid flat across it: with none, a seam was a free hinge and the
# stitch line stood out of the cloth as a fin (11-17: a greatcoat's centre front, 3 cm proud of the cloth beside it
# from the chest to the hem, drawn as a crinkled line down its front once it moved). A pressed seam lies flat.
if E("SEAM_BEND", 1):
    mate = {}
    for i, j in seams:
        mate.setdefault(int(i), set()).add(int(j)); mate.setdefault(int(j), set()).add(int(i))
    def off_line(o, i, j):  # the flat distance of o from the line through i and j, in o's piece's cut
        d = FLAT[j] - FLAT[i]; L = float(np.hypot(*d)) or 1e-9
        return abs(float(d[0] * (FLAT[o][1] - FLAT[i][1]) - d[1] * (FLAT[o][0] - FLAT[i][0]))) / L
    rim = {k: v[0] for k, v in opp.items() if len(v) == 1}
    across, rest_x = [], []
    for (i, i2), o in rim.items():
        for j in mate.get(i, ()):
            for j2 in mate.get(i2, ()):
                k = (min(j, j2), max(j, j2))
                if k in rim and k != (i, i2):
                    across.append((o, rim[k])); rest_x.append(off_line(o, i, i2) + off_line(rim[k], j, j2))
    if across:
        u = {}
        for (a_, b_), r in zip(across, rest_x): u[(min(a_, b_), max(a_, b_))] = r
        bends = np.concatenate([bends, np.array(list(u.keys()), np.int32).reshape(-1, 2)])
        rest_b = np.concatenate([rest_b, np.array(list(u.values()), np.float32)])
        print(f"drape: {len(u)} bends across the seams", flush=True)
cnt = np.bincount(np.concatenate([edges.ravel(), bends.ravel(), seams.ravel()]), minlength=n).astype(np.float32)[:, None]
# pins: one-sided planes on GarmentCode's labelled edges, what elastic, boning or the neck does in a real garment.
# A waistband (lower_interface) stays at or above the waist, a strapless top's edge at or above the armpit, a neckline
# no further out than the side of the neck. Always on: the garment is skinned from this drape and must stay put.
pin_i, pin_p, pin_n = [], [], []
if BODY_YAML:
    import yaml
    b = yaml.safe_load(open(BODY_YAML))["body"]
    waist_y = (b["height"] - b["head_l"] - b["waist_line"]) / 100
    waist_y -= (1 - min(1.0, P.get("rise", 1.0))) * b["hips_line"] / 100  # low-rise bottoms sit between waist and hips
    armpit_y = (b["height"] - b["head_l"] - b["armscye_depth"]) / 100
    neck_x = (b["neck_w"] - 2) / 200
    planes = {"lower_interface": ((0, waist_y, 0), (0, 1, 0)), "strapless_top": ((0, armpit_y, 0), (0, 1, 0)),
              "right_collar": ((-neck_x, 0, 0), (1, 0, 0)), "left_collar": ((neck_x, 0, 0), (-1, 0, 0))}
    for s, piece in zip(start, P["pieces"]):
        for label, idx in piece.get("labels", {}).items():
            if label not in planes: continue
            for i in idx:
                pin_i.append(s + i); pin_p.append(planes[label][0]); pin_n.append(planes[label][1])
PINS = len(pin_i) > 0
if PINS:
    pin_i = mx.array(np.array(pin_i, np.int32)); pin_p = mx.array(np.array(pin_p, np.float32)); pin_n = mx.array(np.array(pin_n, np.float32))
print(f"drape: {NAME}: {n} vertices, {len(edges)} edges, {len(bends)} bends, {len(seams)} seam pairs, "
      f"{len(pin_p) if PINS else 0} pinned", flush=True)

# every link in one list: edges, bends, seams (rest 0); per link a pull and a push stiffness
pairs = mx.array(np.concatenate([edges, bends, seams]))
rest = mx.array(np.concatenate([rest_e, rest_b, np.zeros(len(seams))]).astype(np.float32))
ne, nb, ns = len(edges), len(bends), len(seams)
is_seam = mx.array(np.r_[np.zeros(ne + nb), np.ones(ns)].astype(np.float32))
k_pull0 = mx.array(np.r_[np.full(ne, STRETCH), np.full(nb, BEND), np.zeros(ns)].astype(np.float32))
k_push0 = mx.array(np.r_[np.full(ne, SQUASH), np.full(nb, BEND * 0.5), np.zeros(ns)].astype(np.float32))
a, b = pairs[:, 0], pairs[:, 1]
scale = mx.array(OMEGA / cnt)

def substep(X, V, k_seam, grav, damp):
    V = V * damp + mx.array([0, -9.81, 0]) * grav * (1 / 60 / SUB)
    sp = mx.sqrt(mx.sum(V * V, axis=1, keepdims=True)); V = mx.where(sp > 3, V * 3 / (sp + 1e-9), V)
    p = X + V * (1 / 60 / SUB)
    kp = k_pull0 + is_seam * k_seam; kq = k_push0 + is_seam * k_seam
    for _ in range(ITERS):
        d = p[b] - p[a]; L = mx.sqrt(mx.sum(d * d, axis=1)) + 1e-9
        c = L - rest
        corr = (0.5 * mx.where(c > 0, kp, kq) * c / L)[:, None] * d
        acc = mx.zeros_like(p).at[a].add(corr).at[b].add(-corr)
        p = p + acc * scale
    if PINS:
        q = p[pin_i]; c = mx.sum((q - pin_p) * pin_n, axis=1)
        p = p.at[pin_i].add(-mx.minimum(c, 0)[:, None] * pin_n)
    s, nrm = body(p)
    hit = (s < THICK) & (s > -REACH)
    p = p + mx.where(hit[:, None], nrm * (THICK - s)[:, None], 0)
    Vn = (p - X) * (60 * SUB)
    Vn = mx.where(hit[:, None], Vn * (1 - FRICTION), Vn)
    return p, Vn

step = mx.compile(substep)
SEWN = int(FRAMES * 0.45)
Xm, Vm = mx.array(X), mx.zeros((n, 3))
ea, eb, re = mx.array(edges[:, 0]), mx.array(edges[:, 1]), mx.array(rest_e.astype(np.float32))
sa, sb = mx.array(seams[:, 0]), mx.array(seams[:, 1])
def stats():
    gap = np.array(mx.sqrt(mx.sum((Xm[sa] - Xm[sb]) ** 2, axis=1))) * 1000
    strain = np.array(mx.sqrt(mx.sum((Xm[ea] - Xm[eb]) ** 2, axis=1)) / re - 1)
    return gap, strain
t0 = time.time(); snaps = {}
for f in range(1, FRAMES + 1):
    if os.environ.get("SNAP") and f in (1, SEWN // 2, SEWN, SEWN + 15, SEWN + 40, FRAMES): snaps[f"f{f:03d}"] = np.array(Xm)
    grav = 0.0 if f < SEWN else min(1.0, (f - SEWN) / 20)
    k_seam = min(0.6, 0.05 + 0.55 * f / max(1, SEWN // 2))
    damp = 0.9 if f < SEWN else 0.985
    ks, gr, dm = mx.array(k_seam, mx.float32), mx.array(grav, mx.float32), mx.array(damp, mx.float32)
    for _ in range(SUB):
        Xm, Vm = step(Xm, Vm, ks, gr, dm)
        mx.eval(Xm, Vm)
    if f in (1, SEWN // 2, SEWN, FRAMES) or f % 30 == 0:
        gap, strain = stats()
        print(f"drape: frame {f}: seam gap mm median {np.median(gap):.1f} p95 {np.percentile(gap, 95):.1f}; edge strain p5 "
              f"{np.percentile(strain, 5):+.3f} p95 {np.percentile(strain, 95):+.3f}; {time.time() - t0:.1f} s", flush=True)

SMOOTH = int(E("SMOOTH", 0))  # Taubin passes after the drape: takes out buckling at the mesh's own scale, keeps folds
if SMOOTH:
    nb = np.concatenate([edges, seams]); deg = np.bincount(nb.ravel(), minlength=n).astype(np.float32)[:, None]
    na, nbb = mx.array(nb[:, 0]), mx.array(nb[:, 1]); degm = mx.array(np.maximum(deg, 1))
    def lap(p):
        acc = mx.zeros_like(p).at[na].add(p[nbb]).at[nbb].add(p[na])
        return acc / degm - p
    for _ in range(SMOOTH):
        Xm = Xm + 0.5 * lap(Xm); Xm = Xm - 0.53 * lap(Xm)
        s, nrm = body(Xm); Xm = Xm + mx.where(((s < THICK) & (s > -REACH))[:, None], nrm * (THICK - s)[:, None], 0)
    mx.eval(Xm)
gap, strain = stats()
s, _ = body(Xm); inside = int(np.sum(np.array(s) < -0.001))
Xd = np.array(Xm)
# stayed: a top still hangs from the shoulders (SOMA's are at 1.45 m), anything else from the waist (1.05 m); on a
# body of another build (BODY_YAML's measures) both heights scale with its height to the base of the neck (SOMA's
# 145.8 cm): the woman build's coat (11-17), at her neck at 1.35 m, read as slid off against 1.38
build = 1.0
if BODY_YAML:
    _b = yaml.safe_load(open(BODY_YAML))["body"]; build = (_b["height"] - _b["head_l"]) / 145.77
has_top = any("torso" in p["name"] for p in P["pieces"])
top_y = float(Xd[:, 1].max()); stayed = top_y > (1.38 if has_top else 0.95) * build
# closed: no seam left open more than 2 cm anywhere (a cuff whose side seam never closed hangs open below the elbow,
# and passed on the 95th percentile alone: 23 of the first 139 lean drapes, 12 of them with cuffs)
closed = bool(gap.max() < 20)
ok = bool(np.isfinite(Xd).all() and np.percentile(gap, 95) < 15 and inside < 0.01 * n and stayed and closed)
np.savez_compressed(os.path.join(out_dir, NAME + ".draped.npz"), X=Xd, F=F, FLAT=FLAT, seams=seams, start=np.array(start))
if snaps: np.savez_compressed(os.path.join(out_dir, NAME + ".snaps.npz"), start=np.array(start), **snaps)
report = {"name": NAME, "frames": FRAMES, "vertices": n, "seconds": round(time.time() - t0, 1), "ok": ok,
          "stayed": bool(stayed), "closed": closed, "top_y": round(top_y, 3),
          "seam_gap_mm": {"median": float(np.median(gap)), "p95": float(np.percentile(gap, 95)), "max": float(gap.max())},
          "edge_strain": {"p5": float(np.percentile(strain, 5)), "p95": float(np.percentile(strain, 95))},
          "cloth_vertices_inside_skin": inside}
json.dump(report, open(os.path.join(out_dir, NAME + ".drape.json"), "w"), indent=1)
print("drape:", json.dumps(report), flush=True)
