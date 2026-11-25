# SOMA's body as a signed distance grid for the GPU drape, signed by fast winding number (libigl) rather than by the
# nearest face's normal (sdf_build.py), which put "inside" patches in the air above the shoulders.
#   python sdf_igl.py BODY.glb OUT.npz [CELL_M] [--under GARMENT.glb ...] [--under-thick M]
# Metres, glTF's frame (y up, +z ahead), negative inside; OUT holds origin, cell, sdf (nx, ny, nz), grad (nx, ny, nz, 3).
# --under: garments already sewn on this body, made part of what the next one drapes over: each one's open edges
# (a skirt's waist and hem, a trouser's waist and legs) capped so it closes round the body, inflated by its cloth's
# thickness (--under-thick, 3 mm) and joined to the body. A coat sewn without it was cut and draped on the bare
# body, so a skirt wider at the knee than the coat's hem stood out through it and its edge showed where hide_under
# had cut its faces under the coat (THE GAMBIT's tram conductress, SOMA's woman build, 11-17).
import sys, time
import numpy as np, trimesh, igl

args = [a for a in sys.argv[1:]]
under, thick = [], 0.003
while "--under" in args:
    i = args.index("--under"); under.append(args[i + 1]); del args[i:i + 2]
if "--under-thick" in args:
    i = args.index("--under-thick"); thick = float(args[i + 1]); del args[i:i + 2]
glb, out = args[:2]
cell = float(args[2]) if len(args) > 2 else 0.007
scene = trimesh.load(glb, process=False)
skin = max(scene.dump() if isinstance(scene, trimesh.Scene) else [scene], key=lambda m: len(m.vertices))
V = np.asarray(skin.vertices, float); F = np.asarray(skin.faces, np.int64)
print(f"sdf: skin {len(V)} vertices, y {V[:, 1].min():.3f}..{V[:, 1].max():.3f}, z {V[:, 2].min():.3f}..{V[:, 2].max():.3f}", flush=True)
lo = np.array([-0.75, -0.05, -0.32]); hi = np.array([0.75, 1.88, 0.34])
n = np.ceil((hi - lo) / cell).astype(int) + 1
g = np.stack(np.meshgrid(*[lo[i] + cell * np.arange(n[i]) for i in range(3)], indexing="ij"), -1).reshape(-1, 3)
t0 = time.time()
S, _, _, _ = igl.signed_distance(g, V, F, sign_type=igl.SIGNED_DISTANCE_TYPE_FAST_WINDING_NUMBER)


def capped(path):
    """the garment's meshes as one, welded, each open edge loop closed by a fan from its centre"""
    sc = trimesh.load(path, force="scene")
    m = trimesh.util.concatenate([g for g in sc.dump() if len(g.faces)])
    m.merge_vertices(digits_vertex=5)                     # (a garment's pieces arrive split at seams and UV islands)
    Vg, Fg = np.asarray(m.vertices, float), np.asarray(m.faces, np.int64)
    E = np.sort(np.concatenate([Fg[:, [0, 1]], Fg[:, [1, 2]], Fg[:, [2, 0]]]), 1)
    u, c = np.unique(E, axis=0, return_counts=True)
    nxt = {}
    for a, b in u[c == 1]:
        nxt.setdefault(a, []).append(b); nxt.setdefault(b, []).append(a)
    seen, caps, add = set(), [], []
    for s in list(nxt):
        if s in seen:
            continue
        loop, prev, cur = [s], None, s
        seen.add(s)
        while True:
            nb = [x for x in nxt[cur] if x != prev and (x not in seen or (x == s and len(loop) > 2))]
            if not nb:
                break
            prev, cur = cur, nb[0]
            if cur == s:
                break
            loop.append(cur); seen.add(cur)
        if len(loop) < 8:                                 # (a pinhole: left)
            continue
        ci = len(Vg) + len(add); add.append(Vg[loop].mean(0))
        caps += [[loop[k], loop[(k + 1) % len(loop)], ci] for k in range(len(loop))]
        print(f"sdf: under {path.rsplit('/', 1)[-1]}: an edge of {len(loop)} vertices closed at y {Vg[loop, 1].mean():.3f}", flush=True)
    return np.vstack([Vg] + ([np.array(add)] if add else [])), np.vstack([Fg] + ([np.array(caps)] if caps else []))


for p in under:
    Vu, Fu = capped(p)
    Su, _, _, _ = igl.signed_distance(g, Vu, Fu, sign_type=igl.SIGNED_DISTANCE_TYPE_FAST_WINDING_NUMBER)
    S = np.minimum(S, Su - thick)
sdf = S.reshape(n).astype(np.float32)
grad = np.stack(np.gradient(sdf, cell), axis=-1).astype(np.float32)
np.savez_compressed(out, origin=lo.astype(np.float32), cell=np.float32(cell), sdf=sdf, grad=grad)
print(f"sdf: {tuple(int(x) for x in n)} points at {cell * 1000:.0f} mm, inside {int((sdf < 0).sum())}, {time.time() - t0:.0f} s -> {out}")
