# A worn hard piece laid on SOMA (docs/character.feature CH49): Idol's mesh of the dressed mannequin is turned, sized
# and placed onto the body (the turn searched, then refined against the body's surface), the body is cut away (every
# face within CUT metres of the skin or inside it), small crumbs are dropped, and each face is given to the bone it lies
# nearest, so each piece can ride that bone rigidly.
#   python -I tools/garments/wear_fit.py IDOL.glb SOMA_BODY.glb SOMA_SKELETON.json OUT.npz [CUT=0.006] [LIFT=0] [KEY=0]
# KEY=1: the mannequin was drawn magenta (wear_mannequin.py KEY=1), so faces that come back magenta are bare body and go,
# and each piece left rides one bone whole.
# CUT: faces nearer the skin than this are the body and go (a negative CUT keeps the whole dressed surface: a full suit).
# LIFT: what is kept and lies nearer the skin than this is set out to it along the skin's normal, so a close-fitting
# piece covers the body instead of meeting it.
# OUT.npz: V (metres, glTF frame), F, C (vertex colours 0..1), bone (each vertex's joint index in the skeleton), and the
# fit's numbers; a report line is printed as JSON.
import json, sys
import numpy as np
import trimesh, igl
from scipy.spatial import cKDTree

idol_path, body_path, skel_path, out = sys.argv[1:5]
CUT = float(sys.argv[5]) if len(sys.argv) > 5 else 0.006
LIFT = float(sys.argv[6]) if len(sys.argv) > 6 else 0.0
KEY = len(sys.argv) > 7 and sys.argv[7] == "1"

def load(path, pick=None):
    sc = trimesh.load(path, force="scene")
    geoms = [(n, g) for n, g in sc.geometry.items() if isinstance(g, trimesh.Trimesh) and (pick is None or pick(n, g))]
    parts = []
    for n, g in geoms:
        m = g.copy()
        nodes = sc.graph.geometry_nodes.get(n, [])
        if nodes: m.apply_transform(sc.graph.get(nodes[0])[0])
        parts.append(m)
    return trimesh.util.concatenate(parts)

body = load(body_path, lambda n, g: len(g.vertices) > 5000)            # the skin (the eyes are small pieces)
dressed = load(idol_path)
skel = json.load(open(skel_path))
H = skel["height"]

def umeyama(src, dst):
    ms, md = src.mean(0), dst.mean(0); S, D = src - ms, dst - md
    U, sv, Vt = np.linalg.svd(D.T @ S / len(src)); d = np.sign(np.linalg.det(U @ Vt)); E = np.diag([1, 1, d])
    R = U @ E @ Vt; k = np.trace(np.diag(sv) @ E) / (S ** 2).sum() * len(src)
    return k, R, md - k * R @ ms

rng = np.random.default_rng(7)
B = body.sample(6000, seed=7) if hasattr(body, "sample") else body.vertices[rng.choice(len(body.vertices), 6000)]
B = np.asarray(B)
P0 = np.asarray(dressed.vertices)
sub = P0[rng.choice(len(P0), min(len(P0), 40000), replace=False)]

def yaw(a):
    c, s = np.cos(a), np.sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])

# the start: Idol's mesh stood up to the body's height, its middle on the body's, turned every 10 degrees; the turn whose
# body points lie nearest the dressed surface is kept
k0 = H / (P0[:, 1].max() - P0[:, 1].min())
best = None
for deg in range(0, 360, 10):
    R = yaw(np.radians(deg)) * k0
    Q = sub @ R.T
    Q += np.array([B[:, 0].mean() - Q[:, 0].mean(), B[:, 1].min() - Q[:, 1].min(), B[:, 2].mean() - Q[:, 2].mean()])
    d, _ = cKDTree(Q).query(B)
    score = np.median(d)
    if best is None or score < best[0]: best = (score, deg)
_, deg = best
T_k, T_R = k0, yaw(np.radians(deg))
T_t = np.zeros(3)
Q = sub @ (T_k * T_R).T
T_t = np.array([B[:, 0].mean() - Q[:, 0].mean(), B[:, 1].min() - Q[:, 1].min(), B[:, 2].mean() - Q[:, 2].mean()])

# refined: each body point paired with the nearest dressed point, the nearest 60 per cent kept, the similarity that
# takes those dressed points onto their body points solved, repeated
for it in range(40):
    Q = (T_k * (sub @ T_R.T)) + T_t
    d, idx = cKDTree(Q).query(B)
    keep = d <= np.quantile(d, 0.6)
    k, R, t = umeyama(sub[idx[keep]], B[keep])
    T_k, T_R, T_t = k, R, t
Q = (T_k * (P0 @ T_R.T)) + T_t
d_fit, _ = cKDTree(Q).query(B)

# the body cut away: the signed distance of each dressed vertex to the skin (inside negative)
sd, _, Cp, _ = igl.signed_distance(Q, np.asarray(body.vertices, dtype=np.float64), np.asarray(body.faces, dtype=np.int64), sign_type=igl.SIGNED_DISTANCE_TYPE_FAST_WINDING_NUMBER)
lifted = 0
if LIFT > 0:
    near = sd < LIFT
    off = Q[near] - Cp[near]; ln = np.linalg.norm(off, axis=1, keepdims=True)
    # outward: from the skin's nearest point toward the vertex when outside, away from it when inside
    nrm = np.where(ln > 1e-9, off / np.maximum(ln, 1e-9), 0) * np.sign(sd[near])[:, None]
    Q = Q.copy(); Q[near] = Cp[near] + nrm * LIFT; lifted = int(near.sum())
F = np.asarray(dressed.faces)
keepf = (sd[F] > CUT).all(1)
try:
    cols = np.asarray(dressed.visual.vertex_colors)[:, :3] / 255.0
except Exception:
    cols = np.full((len(Q), 3), 0.6)
keyed = 0
if KEY:
    # magenta: red and blue over green, green well under both (a garment's purple or pink keeps more green, or less
    # red), in shadow as well as lit (a hand in shade comes back as dark as 0.2, 0, 0.19)
    mag = (np.minimum(cols[:, 0], cols[:, 2]) > 0.08) & (cols[:, 1] < 0.55 * np.minimum(cols[:, 0], cols[:, 2])) & (np.abs(cols[:, 0] - cols[:, 2]) < 0.35)
    bare = mag[F].sum(1) >= 2
    # the colour decides, not the distance: a piece sitting tight on the skin (a helmet's crown on the skull, a hat's
    # band on the brow) lies within CUT of it and is kept, set out by LIFT; only a face near the skin with any magenta
    # tint at a corner (the bare edge where the colour bleeds) goes with the keyed ones
    tint = (cols[:, 1] < 0.8 * np.minimum(cols[:, 0], cols[:, 2]))
    keyed = int(bare.sum()); keepf = ~bare & ~(~keepf & tint[F].any(1))
# crumbs dropped: pieces under 0.5 per cent of what is kept, and with KEY a piece mostly magenta (a finger's edge whose
# corners fell either side of the colour test)
faces_kept = F[keepf]
lab = trimesh.graph.connected_component_labels(trimesh.graph.face_adjacency(faces_kept), node_count=len(faces_kept))
sizes = np.bincount(lab)
good = sizes[lab] >= 0.005 * len(faces_kept)
if KEY:
    magf = np.bincount(lab, weights=mag[faces_kept].mean(1), minlength=len(sizes)) / sizes
    good &= magf[lab] < 0.5
    # and a piece that hugs the skin all over is skin the drawing never showed (the soles of the feet, grey from Idol),
    # not something worn: nine tenths of its corners within 1 cm of the skin (the soles come back 4 to 9 mm under it;
    # a helmet, a breastplate, a belt sit 3 to 10 cm out at the median)
    hug = np.array([np.quantile(sd[faces_kept[lab == l]], 0.9) < max(2.5 * CUT, 0.01) for l in range(len(sizes))])
    good &= ~hug[lab]
faces_kept, lab = faces_kept[good], lab[good]
used = np.unique(faces_kept)
remap = -np.ones(len(Q), int); remap[used] = np.arange(len(used))
V, Fo, C = Q[used], remap[faces_kept], cols[used]

# each vertex to the bone it lies nearest (the body's bones, not its helpers or bone ends; fingers count as the hand)
J = skel["joints"]
names = [j["name"] for j in J]
def owner(n):
    if "Twist" in n or n.endswith("End") or n in ("LeftEye", "RightEye", "Jaw"): return None
    for side in ("Left", "Right"):
        if n.startswith(side + "Hand") and n != side + "Hand": return side + "Hand"
    return n
segs = [(i, np.array(J[i]["head"]), np.array(J[i]["tail"])) for i, n in enumerate(names) if owner(n) == n]
def seg_d(P, a, b):
    ab = b - a; t = np.clip(((P - a) @ ab) / max(ab @ ab, 1e-12), 0, 1)
    return np.linalg.norm(P - (a + t[:, None] * ab), axis=1)
D = np.stack([seg_d(V, a, b) for _, a, b in segs], 1)
bone = np.array([segs[i][0] for i in D.argmin(1)])
if KEY:
    # a piece that leaves skin bare is hard and moves as one: each rides the bone nearest its centre (a helmet split
    # between the head and the neck would shear when the head turns; the bone most of a breastplate lies nearest can
    # be an arm, under its shoulder plates, and a belt's the leg its thigh plate hangs over). Nothing hard rides a neck
    # bone alone (a helmet's cheek guards ride the head, a gorget the chest), and a piece across the middle of the body
    # rides a bone down its middle (a belt the hips, not a leg).
    vlab = np.zeros(len(V), int); vlab[Fo.ravel()] = np.repeat(lab, 3)
    tri = V[Fo]; area = 0.5 * np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1)
    riders = [s for s in segs if not names[s[0]].startswith("Neck")]
    middle = [s for s in riders if not names[s[0]].startswith(("Left", "Right"))]
    for l in np.unique(lab):
        f = lab == l; xs = tri[f][..., 0]
        c = (tri[f].mean(1) * area[f, None]).sum(0) / max(area[f].sum(), 1e-12)
        pool = middle if xs.min() < -0.05 and xs.max() > 0.05 else riders
        bone[vlab == l] = pool[int(np.argmin([seg_d(c[None], a, b)[0] for _, a, b in pool]))][0]
# a face goes to the bone most of its corners have, so pieces are whole faces
fb = bone[Fo]
fbone = np.where(fb[:, 1] == fb[:, 2], fb[:, 1], fb[:, 0])
counts = {names[b]: int((fbone == b).sum()) for b in np.unique(fbone)}
report = {"turned": deg, "scale": round(float(T_k / k0), 4), "body_to_dressed_mm": {"median": round(float(np.median(d_fit) * 1000), 1), "p90": round(float(np.quantile(d_fit, 0.9) * 1000), 1)},
          "kept_faces": int(len(Fo)), "of": int(len(F)), "cut_m": CUT, "pieces_by_bone": dict(sorted(counts.items(), key=lambda kv: -kv[1])),
          "inside_skin_after_cut": int((sd[used] < 0).sum()) if LIFT <= 0 else 0, "lifted_vertices": lifted, "keyed_faces": keyed}
np.savez_compressed(out, V=V.astype(np.float32), F=Fo.astype(np.int32), C=C.astype(np.float32), bone=bone.astype(np.int32), fbone=fbone.astype(np.int32), names=np.array(names))
print("fit:", json.dumps(report), flush=True)
