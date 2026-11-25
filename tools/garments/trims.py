# A sewn garment's trims (buttons, shoulder straps, pocket flaps, a belt) as parts of its glb: each one found on the
# draped cloth by casting toward it from outside, built there as its own small mesh (a domed button, a slab that
# follows the cloth, a band round the body), and skinned as the cloth under it is (each vertex the nearest cloth
# vertex's joints and weights), so it moves with the garment wherever <Character> takes it. A trim set made again
# replaces the last. Run after fabric.py's cover when a trim is in the garment's own cloth (it takes the cloth's maps).
#   python tools/garments/trims.py GARMENT.glb TRIMS.json        (add --landmarks: the body's joints, to place by)
#   python tools/garments/trims.py GARMENT.glb --mend            the trims already on it (a worn copy too) re-wound
#                                                                facing out, shaded as sheets (sheet_normals)
# TRIMS.json: {"trims": [ ... ]}, every length in metres, in the garment's frame (glTF: y up, the body facing +z; all
# of raea's sewn garments are cut to SOMA's one body, so a place on one is the same place on another). Each trim:
#   {"kind": "button", "at": [x, y], "from": "front", "d": 0.022, "material": {...}}       from front|back|left|right
#      (or "at": [x, y, z] with "from": [dx, dy, dz], the direction it is cast along); "copies": [[dx, dy], ...]
#      places more of the same, offset; "mirror": true adds the -x twin of each
#   {"kind": "patch", "centre": [x, y, z], "from": [dx, dy, dz], "along": [ux, uy, uz], "size": [length, width],
#      "thick": 0.003, "end": "square"|"round"|"point" (the far end along `along`), "piping": {"w": 0.002, ...material},
#      "mirror": true, "material": {...}}                        a shoulder strap, a pocket flap, a collar patch
# "over": metres a button or patch stands further out (on a strap or a belt, not the cloth under it).
#   {"kind": "band", "y": 1.0, "h": 0.05, "thick": 0.004, "axis": [x, z], "within": 0.32, "material": {...}}
#      round the body at a height, on the outermost cloth nearer the axis than `within` (not the sleeves)
#   {"kind": "sash", "lo": [x, y, z], "hi": [x, y, z], "w": 0.02, "thick": 0.003, "material": {...}}
#      a strap across the body from by one hip (lo) over the other shoulder (hi) and down the back, on the outermost
#      cloth (a bag's strap, a cartridge bandolier, an officer's cross belt)
#   {"kind": "bag", "at": [x, y], "from": [dx, dy, dz], "size": [w, h, d], "material": {...}, "frame": {...}}
#      a framed leather bag hung against the cloth (its frame's top where the ray meets it), rigid, a metal frame and
#      clasp on top (a conductress's money bag; hang its strap as a sash whose low end passes behind it)
#   {"kind": "collar", "stand": 0.035, "fall": 0.03, "gap": 0.02, "thick": 0.003, "ease": 0.006, "drop": 0.05, "neck": {"centre": [x, z],
#      "r": [rx, rz]}, "points": {"w": 0.018, "len": 0.04, "material": {...}, "piping": {...}}, "material": {...}}
#      a stand-and-fall collar from the garment's neckline up round the neck, stiff, an L of braid on each point if asked
#      ("drop": its top at the throat no lower than this below its top at the back, over a neckline cut low in front)
#      (sew the garment with no collar of its own: a sewn one has nothing to hold it up and falls open at the throat)
# material: {"colour": "#rrggbb", "metal": 0..1, "rough": 0..1, "cloth": true (the garment's cloth maps under the
#   colour)}. Any trim may carry "source": where it is attested (printed, kept in the glb's extras).
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from glbio import Glb  # noqa: E402

LIFT = 0.0008     # off the cloth, so a trim never sinks into it where the cloth's triangles bow out between samples
DIRS = {"front": (0, 0, -1), "back": (0, 0, 1), "left": (-1, 0, 0), "right": (1, 0, 0), "top": (0, -1, 0)}


def unit(v):
    v = np.asarray(v, np.float64)
    return v / (np.linalg.norm(v, axis=-1, keepdims=True) + 1e-12)


class Cloth:
    def __init__(self, P, T, N):
        self.P, self.T, self.N = P, T, N
        self.A, self.B, self.C = P[T[:, 0]], P[T[:, 1]], P[T[:, 2]]
        self.e1, self.e2 = self.B - self.A, self.C - self.A

    def hits(self, o, d):
        """Every crossing of the ray o + t d (t > 0): [(t, point, normal)], nearest first (Moller-Trumbore)."""
        p = np.cross(d, self.e2)
        det = (self.e1 * p).sum(1)
        ok = np.abs(det) > 1e-12
        inv = np.where(ok, 1 / np.where(ok, det, 1), 0)
        s = o - self.A
        u = (s * p).sum(1) * inv
        q = np.cross(s, self.e1)
        v = (q * d).sum(1) * inv
        t = (q * self.e2).sum(1) * inv
        k = np.nonzero(ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-6))[0]
        out = []
        for i in k[np.argsort(t[k])]:
            w = np.array([1 - u[i] - v[i], u[i], v[i]])
            n = unit((self.N[self.T[i]] * w[:, None]).sum(0))
            if (n * d).sum() > 0:                         # the side facing where the ray came from
                n = -n
            out.append((t[i], o + t[i] * d, n))
        return out

    def onto(self, at, d):
        """The cloth's first surface met from `at` along `d` (`at` outside it): (point, normal) or None."""
        h = self.hits(np.asarray(at, np.float64), unit(d))
        return (h[0][1], h[0][2]) if h else None


def dome(p, n, r, h, seg=24):
    """A button: a domed disc of radius r standing h off the cloth at p, facing n."""
    t = unit(np.cross(n, [0, 1, 0] if abs(n[1]) < 0.9 else [1, 0, 0]))
    b = np.cross(n, t)
    rings = [(0.0, h), (0.45, 0.96 * h), (0.78, 0.86 * h), (0.93, 0.74 * h), (1.0, 0.5 * h), (1.0, -0.4 * h)]
    a = np.linspace(0, 2 * np.pi, seg, endpoint=False)
    V, UV = [], []
    for rr, hh in rings:
        for x in a:
            V.append(p + n * (hh + LIFT) + r * rr * (np.cos(x) * t + np.sin(x) * b))
            UV.append([0.5 + 0.5 * rr * np.cos(x), 0.5 + 0.5 * rr * np.sin(x)])
    F = []
    for i in range(len(rings) - 1):
        for k in range(seg):
            a0, a1 = i * seg + k, i * seg + (k + 1) % seg
            F += [[a0, a0 + seg, a1], [a1, a0 + seg, a1 + seg]]
    return np.array(V), np.array(F), np.array(UV)


def slab(grid_p, grid_n, inside, thick):
    """A slab conformed to the cloth: grid points (nu x nv x 3) and normals where it lies, `inside` (nu x nv) the
    cells' corners kept (a rounded or pointed end leaves some out), standing `thick` off it. Returns V, F, UV
    (UV in metres along the slab, so the garment's cloth maps lie on it at their own scale)."""
    nu, nv = inside.shape
    top = grid_p + grid_n * (LIFT + thick)
    bot = grid_p + grid_n * LIFT * 0.5
    V, UV, idx = [], [], -np.ones((2, nu, nv), int)
    du = np.linalg.norm(np.diff(grid_p, axis=0), axis=2).mean() if nu > 1 else 0
    dv = np.linalg.norm(np.diff(grid_p, axis=1), axis=2).mean() if nv > 1 else 0
    for layer, G in ((0, top), (1, bot)):
        for i in range(nu):
            for k in range(nv):
                if inside[i, k]:
                    idx[layer, i, k] = len(V)
                    V.append(G[i, k])
                    UV.append([k * dv, i * du])
    F = []
    for i in range(nu - 1):
        for k in range(nv - 1):
            q = [idx[0, i, k], idx[0, i + 1, k], idx[0, i + 1, k + 1], idx[0, i, k + 1]]
            if min(q) >= 0:
                F += [[q[0], q[2], q[1]], [q[0], q[3], q[2]]]
    # the edge: wherever a kept corner has an unkept (or no) neighbour, a wall from the top down to the cloth
    def kept(i, k):
        return 0 <= i < nu and 0 <= k < nv and inside[i, k]
    for i in range(nu):
        for k in range(nv):
            for (di, dk) in ((1, 0), (0, 1)):
                i2, k2 = i + di, k + dk
                if not (kept(i, k) and kept(i2, k2)):
                    continue
                # the two cells either side of this edge: a wall when only one of them is whole
                if di:
                    sides = [all(kept(i + a, k + b) for a in (0, 1) for b in (s, s + 1)) for s in (-1, 0)]
                else:
                    sides = [all(kept(i + a, k + b) for a in (s, s + 1) for b in (0, 1)) for s in (-1, 0)]
                if sides[0] == sides[1]:
                    continue
                a0, a1, b0, b1 = idx[0, i, k], idx[0, i2, k2], idx[1, i, k], idx[1, i2, k2]
                F += [[a0, b0, a1], [a1, b0, b1]] if sides[1] ^ bool(di) else [[a0, a1, b0], [a1, b1, b0]]
    return np.array(V), np.array(F), np.array(UV)


def patch_grid(cl, centre, d, along, size, end, res=0.006):
    """Cast a grid over a length x width rectangle at `centre` (length along `along`) down `d` onto the cloth."""
    d = unit(d)
    U = unit(np.asarray(along) - (np.asarray(along) @ d) * d)
    W = np.cross(d, U)
    L, w = size
    nu, nv = max(2, int(np.ceil(L / res)) + 1), max(2, int(np.ceil(w / res)) + 1)
    P = np.zeros((nu, nv, 3)); N = np.zeros((nu, nv, 3)); ok = np.zeros((nu, nv), bool)
    for i in range(nu):
        for k in range(nv):
            u, v = -L / 2 + L * i / (nu - 1), -w / 2 + w * k / (nv - 1)
            # the far end: round (a semicircle of the width) or point (a chevron), cut from the rectangle
            far = u - (L / 2 - w / 2)
            if end == "round" and far > 0 and far ** 2 + v ** 2 > (w / 2) ** 2 + 1e-9:
                continue
            if end == "point" and far > 0 and abs(v) > (w / 2) * (1 - far / (w / 2)) + 1e-9:
                continue
            o = np.asarray(centre) + u * U + v * W - d * 0.4
            h = cl.onto(o, d)
            if h is None:
                continue
            P[i, k], N[i, k], ok[i, k] = h[0], h[1], True
    return P, N, ok


def band(cl, y, h, thick, axis, within, seg=96):
    """A band round the body: at each angle, the outermost cloth nearer the axis than `within`."""
    rows = 4
    P = np.zeros((rows, seg + 1, 3)); N = np.zeros_like(P); ok = np.zeros((rows, seg + 1), bool)
    for r in range(rows):
        yy = y - h / 2 + h * r / (rows - 1)
        for k in range(seg + 1):
            a = 2 * np.pi * k / seg
            o = np.array([axis[0] + 0.8 * np.cos(a), yy, axis[1] + 0.8 * np.sin(a)])
            hs = cl.hits(o, unit([axis[0] - o[0], 0, axis[1] - o[2]]))
            hs = [x for x in hs if np.hypot(x[1][0] - axis[0], x[1][2] - axis[1]) < within]
            if hs:
                P[r, k], N[r, k], ok[r, k] = hs[0][1], unit(hs[0][2] * [1, 0.3, 1]), True
    # a gap (a ray that met nothing) filled from its neighbours round the ring
    for r in range(rows):
        good = np.nonzero(ok[r])[0]
        if len(good) == 0:
            raise SystemExit(f"trims: no cloth round y={y:.3f} within {within} m of the axis")
        for k in np.nonzero(~ok[r])[0]:
            j = good[np.argmin(np.minimum(np.abs(good - k), seg - np.abs(good - k)))]
            P[r, k], N[r, k] = P[r, j], N[r, j]
        ok[r] = True
    # smooth round the ring (the cloth's folds shouldn't kink a belt pulled tight over them)
    for _ in range(3):
        P[:, 1:-1] = (P[:, :-2] + 2 * P[:, 1:-1] + P[:, 2:]) / 4
    P[:, -1] = P[:, 0]
    return P, N, ok


def sash(cl, lo, hi, w, seg=144):
    """A strap worn across the body, over one shoulder to the other hip: the band's ring in the plane through `lo` (a
    point by the hip it hangs to) and `hi` (one over the shoulder), front to back, each ray cast in from outside
    toward the chest between them so it lies on the outermost cloth (in SOMA's A pose the arm leaves that plane
    nearly square to it, so the rays by the shoulder meet its top, not the sleeve). Rows across its width `w`."""
    lo, hi = np.asarray(lo, float), np.asarray(hi, float)
    c = (lo + hi) / 2
    u = unit(hi - lo); z = unit(np.array([0, 0, 1.0]) - u * u[2])
    n = unit(np.cross(u, z))
    rows = 3
    P = np.zeros((rows, seg + 1, 3)); N = np.zeros_like(P); ok = np.zeros((rows, seg + 1), bool)
    for r in range(rows):
        cr = c + n * (w * (r / (rows - 1) - 0.5))
        for k in range(seg + 1):
            a = 2 * np.pi * k / seg
            d = np.cos(a) * u + np.sin(a) * z
            hs = cl.hits(cr + 0.9 * d, -d)
            if hs:
                P[r, k], N[r, k], ok[r, k] = hs[0][1], hs[0][2], True
    for r in range(rows):
        good = np.nonzero(ok[r])[0]
        if len(good) == 0:
            raise SystemExit("trims: sash: no cloth in its plane")
        for k in np.nonzero(~ok[r])[0]:
            j = good[np.argmin(np.minimum(np.abs(good - k), seg - np.abs(good - k)))]
            P[r, k], N[r, k] = P[r, j], N[r, j]
        ok[r] = True
    for _ in range(4):                  # pulled taut over the folds, as a strap with a bag's weight on it is
        P[:, 1:-1] = (P[:, :-2] + 2 * P[:, 1:-1] + P[:, 2:]) / 4
    P[:, -1] = P[:, 0]
    # (rows across the plane's normal this way round face the slab inward; slab() faces it out with them reversed)
    return P[::-1], N[::-1], ok


def tube_mesh(path, r, n=10):
    """a round tube along points (k, 3), its ends closed: V, F, UV"""
    P = np.asarray(path, float); k = len(P)
    tan = unit(np.gradient(P, axis=0)); a = unit(np.cross(tan, [0, 1.0, 0]) + 1e-9); b = np.cross(tan, a)
    ang = 2 * np.pi * np.arange(n) / n
    V = (P[:, None] + r * (np.cos(ang)[None, :, None] * a[:, None] + np.sin(ang)[None, :, None] * b[:, None])).reshape(-1, 3)
    F = [[i * n + j, i * n + (j + 1) % n, (i + 1) * n + j] for i in range(k - 1) for j in range(n)]
    F += [[i * n + (j + 1) % n, (i + 1) * n + (j + 1) % n, (i + 1) * n + j] for i in range(k - 1) for j in range(n)]
    V = np.vstack([V, P[0], P[-1]]); c0, c1 = len(V) - 2, len(V) - 1
    F += [[c0, (j + 1) % n, j] for j in range(n)] + [[c1, (k - 1) * n + j, (k - 1) * n + (j + 1) % n] for j in range(n)]
    UV = np.zeros((len(V), 2)); UV[:k * n, 0] = np.repeat(np.arange(k), n) * 0.01; UV[:k * n, 1] = np.tile(ang * r, k)
    return V, np.array(F), UV


def bag(cl, at, d, size, seg=40, rows=24):
    """A framed leather bag hung against the cloth where a ray from outside along `d` meets it at `at` (x, y): `size`
    (width, height, depth) in metres, its frame's top at the point met. The pouch is a rounded box in plan (a
    superellipse), closed flat at the frame, full below it and rounded at the bottom; its back bent round the cloth
    across its width (the leather is soft, the hip round). Returns the pouch (V, F, UV), the frame's path and the
    point the bag hangs from."""
    W, H, D = size
    d = unit(d)
    o = np.array([at[0], at[1], 0.0]) - d * 0.6
    hit = cl.onto(o, d)
    if hit is None:
        raise SystemExit(f"trims: bag: no cloth at {at} from {list(d)}")
    p, n = hit
    out = unit(np.array([n[0], 0, n[2]])); up = np.array([0, 1.0, 0]); right = unit(np.cross(up, out))
    xs = np.linspace(-W / 2, W / 2, 9); back = []
    for x in xs:                        # the cloth's place across the bag's width, at its middle height
        h = cl.onto(p + right * x - up * H / 2 + out * 0.3, -out)
        back.append(((h[0] - p) @ out) if h is not None else 0.0)
    back = np.clip(np.array(back), -0.03, 0.03) + LIFT + 0.006
    t = np.linspace(0, 1, rows)
    tb = np.clip((t - 0.62) / 0.38, 0, 1)
    a = W / 2 * (1 - tb ** 3) ** (1 / 3) * (0.94 + 0.06 * np.minimum(t / 0.2, 1))   # half width: the frame a little narrower
    c = D / 2 * np.sin(np.minimum(t / 0.3, 1) * np.pi / 2) * np.sqrt(np.maximum(a / (W / 2), 0))   # half depth
    ph = 2 * np.pi * np.arange(seg) / seg; e = 2 / 3.0
    cx, cz = np.sign(np.cos(ph)) * np.abs(np.cos(ph)) ** e, np.sign(np.sin(ph)) * np.abs(np.sin(ph)) ** e
    V, UV = [], []
    for i in range(rows):
        for k in range(seg):
            x = a[i] * cx[k]
            z = np.interp(x, xs, back) + D / 2 + c[i] * cz[k]
            V.append(p + right * x + out * z - up * (t[i] * H)); UV.append([ph[k] * (W + D) / np.pi, t[i] * H])
    V.append(p + out * (np.interp(0, xs, back) + D / 2) - up * H); UV.append([0, H])
    F = [[i * seg + k, (i + 1) * seg + k, i * seg + (k + 1) % seg] for i in range(rows - 1) for k in range(seg)]
    F += [[i * seg + (k + 1) % seg, (i + 1) * seg + k, (i + 1) * seg + (k + 1) % seg] for i in range(rows - 1) for k in range(seg)]
    bc = len(V) - 1
    F += [[(rows - 1) * seg + k, bc, (rows - 1) * seg + (k + 1) % seg] for k in range(seg)]
    # (the faces' winding is made to face out below)
    V, F = np.array(V), np.array(F)
    fn = np.cross(V[F[:, 1]] - V[F[:, 0]], V[F[:, 2]] - V[F[:, 0]]); fc = V[F].mean(1)
    mid = p + out * (np.interp(0, xs, back) + D / 2) - up * H / 2
    if ((fn * (fc - mid)).sum(1) < 0).mean() > 0.5:
        F = F[:, [0, 2, 1]]
    fx = np.linspace(-a[0] * 1.02, a[0] * 1.02, 17)
    frame = np.array([p + right * x + out * (np.interp(x, xs, back) + D / 2) + up * (0.008 * (1 - (x / a[0]) ** 2)) for x in fx])
    return (V, F, np.array(UV)), frame, p


def neckline(P, centre, seg, reach=0.16, ylow=1.2):
    """The garment's neckline round the neck: at each of `seg` angles about the vertical axis through `centre` ([x, z];
    angle 0 the front, +z), the highest cloth within `reach` of it and above `ylow`, placed at that angle exactly, its
    height and distance kept. (Not the mesh's open edges: a sewn garment's panels keep their seams open too.)"""
    d = P[:, [0, 2]] - np.asarray(centre)
    r, a = np.hypot(d[:, 0], d[:, 1]), np.arctan2(d[:, 0], d[:, 1])
    near = (r < reach) & (P[:, 1] > ylow)
    out = np.zeros((seg, 2))
    for k in range(seg):
        ak = -np.pi + 2 * np.pi * k / seg
        w = near & (np.abs((a - ak + np.pi) % (2 * np.pi) - np.pi) < 1.5 * np.pi / seg)
        if not w.any():
            raise SystemExit(f"trims: no neckline at {np.degrees(ak):.0f} degrees (no cloth within {reach} m of the neck's axis)")
        i = np.nonzero(w)[0][np.argmax(P[w, 1])]
        out[k] = P[i, 1], r[i]
    for _ in range(2):                                  # a stray high vertex shouldn't notch the collar
        out = (np.roll(out, 1, 0) + 2 * out + np.roll(out, -1, 0)) / 4
    return out


class Body:
    """SOMA's body as raea's sewing measured it (sdf_igl.py's distance grid: metres, glTF's frame, negative inside), to
    keep a trim off the skin: GARMENT_SDF, else raea's models folder's, else the sewing scripts' cache. None if absent."""
    def __init__(self):
        cands = [os.environ.get("GARMENT_SDF", ""), os.path.expanduser("~/.local/share/raea-models/garments/soma_sdf.npz"),
                 os.path.join(os.environ.get("GARMENT_DATA", os.path.expanduser("~/.cache/raea-garments")), "soma_sdf.npz")]
        f = next((c for c in cands if c and os.path.exists(c)), None)
        self.ok = f is not None
        if self.ok:
            S = np.load(f)
            self.o, self.c, self.sdf, self.grad = S["origin"].astype(np.float64), float(S["cell"]), S["sdf"], S["grad"]

    def at(self, p):
        """The distance and its gradient at points p (n x 3), trilinear."""
        from scipy.ndimage import map_coordinates
        u = ((np.asarray(p, np.float64) - self.o) / self.c).T
        d = map_coordinates(self.sdf, u, order=1, mode="nearest")
        g = np.stack([map_coordinates(self.grad[..., k], u, order=1, mode="nearest") for k in range(3)], 1)
        return d, g

    def to(self, p, gap, out_only=False, flat=True, steps=4):
        """p moved (across, keeping its height when flat) until it stands `gap` off the skin; out_only: only if nearer."""
        p = np.array(p, np.float64)
        for _ in range(steps):
            d, g = self.at(p)
            if flat:
                g[:, 1] = 0
            g = unit(g)
            move = gap - d
            if out_only:
                move = np.maximum(move, 0)
            p += g * move[:, None]
        return p


def collar(P, t, seg=192):
    """A stand-and-fall collar, stiff as a real one is (a sewn collar has nothing to hold it up and falls open): the stand
    from the garment's neckline up round the neck, `stand` m tall, its top `ease` off the skin (SOMA's measured body;
    without it the ellipse `neck`: {"centre": [x, z], "r": [rx, rz]}), closed at the front (hooks); the fall turned down
    over it, `fall` m deep, open `gap` m at the front; `points` ({"w", "len", "thick", "material", "piping"}): an L of
    braid on each front point of the fall, up its front edge and `len` along its foot. Returns [(grid, normals,
    inside, label, material, thick)], each grid rows x columns round the neck."""
    nk = t.get("neck", {})
    c, (rx, rz) = np.asarray(nk.get("centre", [0.0, -0.039])), nk.get("r", [0.06, 0.059])
    stand, fall, thick = t.get("stand", 0.035), t.get("fall", 0.03), t.get("thick", 0.003)
    ease, gap, rows = t.get("ease", 0.006), t.get("gap", 0.02), 6
    coarse = neckline(P, c, 96)                  # (the cloth's vertices are too far apart for a finer reading)
    a96 = -np.pi + 2 * np.pi * np.arange(96) / 96
    af = -np.pi + 2 * np.pi * np.arange(seg) / seg
    ring = np.stack([np.interp(af, a96, coarse[:, i], period=2 * np.pi) for i in range(2)], 1)
    A = -np.pi + 2 * np.pi * np.arange(seg + 1) / seg            # the last column the first again: a closed ring
    ring = np.vstack([ring, ring[:1]])
    D = np.stack([np.sin(A), np.zeros_like(A), np.cos(A)], 1)
    rn = 1 / np.sqrt((np.sin(A) / rx) ** 2 + (np.cos(A) / rz) ** 2) + ease
    base = np.stack([c[0] + ring[:, 1] * D[:, 0], ring[:, 0], c[1] + ring[:, 1] * D[:, 2]], 1)
    top_y = ring[:, 0] + stand
    if t.get("drop") is not None:
        # `drop`: the stand's top closes at the throat no lower than `drop` below its top at the back of the neck, its
        # height growing where the garment's neckline scoops lower in front (GarmentCode cuts no higher than its
        # fc_depth 0.3: the woman build's coat scooped 10 cm below the back of its neckline, 11-17, a collar on it a
        # ring round her collarbones)
        top_y = np.maximum(top_y, top_y.max() - t["drop"] * (1 + np.cos(A)) / 2)
    top = np.stack([c[0] + rn * D[:, 0], top_y, c[1] + rn * D[:, 2]], 1)
    body = Body()
    if body.ok:                         # the top hugs the neck as it is (it slopes forward at the throat), `ease` off it
        top = body.to(top, ease + LIFT)
    else:
        print("trims: collar: no body distance grid (raea clothes make measures it): the neck taken as an ellipse")
    S = np.stack([base + (top - base) * i / (rows - 1) for i in range(rows)])          # rows x columns
    if body.ok:                         # and nowhere between nearer the skin than that
        S[1:] = body.to(S[1:].reshape(-1, 3), ease + LIFT, out_only=True).reshape(S[1:].shape)
    S[:, -1] = S[:, 0]

    def normals(G):
        up = np.gradient(G, axis=0)
        ar = (np.roll(G, -1, 1) - np.roll(G, 1, 1))
        ar[:, 0] = ar[:, -1] = G[:, 1] - G[:, -2]
        n = unit(np.cross(up, ar))
        return np.where(((n * D[None]).sum(2) < 0)[..., None], -n, n)
    Ns = normals(S)
    # (slab() faces its outer layer by the grid's order: round the neck this way, the rows run down for it to face out)
    mat = t.get("material", {})
    out = [(S[::-1], Ns[::-1], np.ones(S.shape[:2], bool), "stand", mat, thick)]
    # the fall: from the stand's top edge, folded over it, down its outside to `fall` below the top, flaring a little
    lift = LIFT + thick + 0.0015
    f = np.linspace(0, 1, 9)
    at = (rows - 1) * (1 - np.clip(f * fall / stand, 0, 1))      # the stand's row each of the fall's rows lies over
    lo = np.floor(at).astype(int).clip(0, rows - 2); w = (at - lo)[:, None, None]
    over = S[lo] * (1 - w) + S[lo + 1] * w
    nov = unit(Ns[lo] * (1 - w) + Ns[lo + 1] * w)
    Fg = over + nov * (lift + 0.004 * f[:, None, None]) + np.array([0, 1, 0]) * (0.0012 * (1 - f))[:, None, None]
    rf = rn[np.argmin(np.abs(A))]
    open_ = np.abs(A) < (gap / 2) / max(rf, 1e-3)
    inside = np.broadcast_to(~open_, Fg.shape[:2]).copy()
    out.append((Fg, nov, inside, "fall", mat, thick))
    pt = t.get("points")
    if pt:
        w, ln, pth = pt.get("w", 0.018), pt.get("len", 0.04), pt.get("thick", 0.002)
        dk = (np.abs(A) * rf - gap / 2)[None, :]                    # along the foot from the point's front edge
        vj = ((1 - f) * fall)[:, None]                              # up from the foot
        on = Fg + nov * (thick + LIFT + 0.0003)
        def L_(extra):
            return inside & ((dk <= w + extra) | ((vj <= w + extra) & (dk <= ln + extra)))
        if "piping" in pt:
            pw = pt["piping"].get("w", 0.002)
            out.append((on, nov, L_(pw), "points-piping", pt["piping"], pth * 0.85))
        out.append((on, nov, L_(0.0), "points", pt.get("material", mat), pth))
    return out


def material(spec, base, name):
    c = spec.get("colour", "#808080")
    lin = [(int(c[i:i + 2], 16) / 255) ** 2.2 for i in (1, 3, 5)]
    pbr = base.get("pbrMetallicRoughness", {})
    m = {"name": name, "extras": {"trim": True, "colour": c}}
    if spec.get("cloth") and "baseColorTexture" in pbr:
        m["pbrMetallicRoughness"] = {"baseColorFactor": [min(1, x / 0.8) for x in lin] + [1], "baseColorTexture": pbr["baseColorTexture"],
                                     "metallicRoughnessTexture": pbr["metallicRoughnessTexture"], "metallicFactor": 0.0, "roughnessFactor": 1.0}
        m["normalTexture"] = base["normalTexture"]
        m["extras"]["cloth"] = base.get("extras", {}).get("cloth", True)
    else:
        m["pbrMetallicRoughness"] = {"baseColorFactor": lin + [1], "metallicFactor": float(spec.get("metal", 0)), "roughnessFactor": float(spec.get("rough", 0.6))}
    return m


def wound(V, F, cloth_tree=None, cloth_n=None):
    """F wound one way round over each connected piece, facing out: a slab's walls were wound for the stand's rows and
    came out facing into the fall collar's slab (gambit's Schaffnerin, 11-17): its edge vertices averaged opposing
    faces to normals near zero, drawn as bright lines along the collar. Each piece is walked from its first face (a
    slab's top comes first), every neighbour turned to cross their shared edge the other way; then a closed piece
    faces out by its volume, an open one (a slab) away from the cloth under it."""
    F = np.array(F, np.int64)
    edges = {}
    for f, t in enumerate(F):
        for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])):
            edges.setdefault((min(a, b), max(a, b)), []).append(f)
    seen = np.zeros(len(F), bool)
    for s0 in range(len(F)):
        if seen[s0]:
            continue
        piece, stack, seen[s0] = [s0], [s0], True
        while stack:
            f = stack.pop()
            t = F[f]
            for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])):
                for g_ in edges[(min(a, b), max(a, b))]:
                    if seen[g_]:
                        continue
                    u = F[g_]
                    same = any((u[i] == a and u[(i + 1) % 3] == b) for i in range(3))
                    if same:
                        F[g_] = u[[0, 2, 1]]
                    seen[g_] = True; piece.append(g_); stack.append(g_)
        P_ = np.array(piece)
        tri = V[F[P_]]
        fn = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
        closed = all(len(edges[(min(a, b), max(a, b))]) == 2 for t in F[P_] for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])))
        if closed or cloth_tree is None:
            face_out = (tri[:, 0] * np.cross(tri[:, 1], tri[:, 2])).sum() >= 0
        else:
            _, k = cloth_tree.query(tri.mean(1))
            face_out = (fn * cloth_n[k]).sum() >= 0
        if not face_out:
            F[P_] = F[P_][:, [0, 2, 1]]
    return F


def sheet_normals(V, F, cloth_tree=None, cloth_n=None):
    """A trim's vertex normals. A slab (open: its top and the walls round its edge) is shaded as the sheet it is: every
    vertex takes the normal of the top faces round it (those facing away from the cloth), the walls' own bottom
    vertices that of the top vertex nearest them. Averaged with the 3 mm walls, the edges' normals leaned out and the
    walls showed edge-on: bright white outlines round every collar and strap under raea's even ambient light (gambit's
    Schaffnerin, 11-17, Shane: "some issues here with stitching?"). A closed trim (a button, a tube) is area-weighted."""
    V, F = np.asarray(V, np.float64), np.asarray(F, np.int64)
    fn = np.cross(V[F[:, 1]] - V[F[:, 0]], V[F[:, 2]] - V[F[:, 0]])
    e = np.sort(np.r_[F[:, [0, 1]], F[:, [1, 2]], F[:, [2, 0]]], 1)
    _, cnt = np.unique(e, axis=0, return_counts=True)
    top = np.ones(len(F), bool)
    if cloth_tree is not None and (cnt == 1).any():
        _, k = cloth_tree.query(V[F].mean(1))
        top = (unit(fn) * cloth_n[k]).sum(1) > 0.5
    Nv = np.zeros_like(V)
    for c in range(3):
        np.add.at(Nv, F[top, c], fn[top])
    has = np.linalg.norm(Nv, axis=1) > 0
    if (~has).any() and has.any():
        from scipy.spatial import cKDTree
        _, near = cKDTree(V[has]).query(V[~has])
        Nv[~has] = Nv[has][near]
    Nv[np.linalg.norm(Nv, axis=1) == 0] = [0, 1, 0]
    return unit(Nv).astype(np.float32)


def mend(path):
    """trims.py GARMENT.glb --mend: the trims already on a garment (a worn one too) wound as wound() winds them, and
    their normals made again from the faces"""
    g = Glb(path)
    j = g.j
    from scipy.spatial import cKDTree
    cloth = [p for me in j["meshes"] for p in me["primitives"] if not p.get("extras", {}).get("trim")]
    CP = np.concatenate([g.read(p["attributes"]["POSITION"]).astype(np.float64) for p in cloth])
    CN = np.concatenate([g.read(p["attributes"]["NORMAL"]).astype(np.float64) for p in cloth])
    tree = cKDTree(CP)
    n = 0
    for me in j["meshes"]:
        for p in me["primitives"]:
            if not p.get("extras", {}).get("trim") and not j["materials"][p.get("material", 0)].get("extras", {}).get("trim"):
                continue
            V = g.read(p["attributes"]["POSITION"]).astype(np.float64)
            F0 = g.read(p["indices"]).reshape(-1, 3).astype(np.int64)
            F = wound(V, F0, tree, CN)
            Nv = sheet_normals(V, F, tree, CN)
            p["indices"] = g.accessor(F.astype(np.uint32).reshape(-1), 34963)
            p["attributes"]["NORMAL"] = g.accessor(Nv, 34962)
            n += 1
            print(f"trims: mended {j['materials'][p.get('material', 0)].get('name', '?')}: {int((F != F0).any(1).sum())} of {len(F)} faces turned")
    g.drop_accessors(g.used_accessors())
    size = g.save(path)
    print(f"trims: {n} trims mended on {os.path.basename(path)} ({size / 1e6:.1f} MB)")


def main():
    if "--mend" in sys.argv:
        return mend(sys.argv[1])
    if len(sys.argv) < 2:
        raise SystemExit(__doc__ or "trims.py GARMENT.glb TRIMS.json")
    path = sys.argv[1]
    g = Glb(path)
    j = g.j
    if "--landmarks" in sys.argv:
        # the joints' bind-pose places: each joint's world matrix is the inverse of its inverse bind matrix
        sk = j["skins"][0]
        ibm = g.read(sk["inverseBindMatrices"]).reshape(-1, 4, 4).transpose(0, 2, 1)
        for n, m in zip(sk["joints"], ibm):
            w = np.linalg.inv(m)
            name = j["nodes"][n].get("name", n)
            if any(s in name for s in ("Neck", "Spine", "Chest", "Hips", "Shoulder", "Clavicle", "Arm", "Head", "Leg")) and "Twist" not in name:
                print(f"{name:24s} {w[0, 3]:+.3f} {w[1, 3]:+.3f} {w[2, 3]:+.3f}")
        return
    spec = json.load(open(sys.argv[2]))
    mesh = j["meshes"][0]
    # the last trims out: their primitives, materials and accessors
    keep = [p for p in mesh["primitives"] if not p.get("extras", {}).get("trim")]
    mesh["primitives"] = keep
    mats = j.get("materials", [])
    live = [i for i, m in enumerate(mats) if not m.get("extras", {}).get("trim")]
    remap = {o: n for n, o in enumerate(live)}
    j["materials"] = [mats[i] for i in live]
    for p in keep:
        if "material" in p:
            p["material"] = remap[p["material"]]
    g.drop_accessors(g.used_accessors())
    # the cloth is every primitive the garment had (a collar in its own colour is a primitive of its own)
    base = keep[0]
    Ps, Ns, Ts, Js, Ws, at = [], [], [], [], [], 0
    for p in keep:
        Ps.append(g.read(p["attributes"]["POSITION"]).astype(np.float64)); Ns.append(g.read(p["attributes"]["NORMAL"]).astype(np.float64))
        Ts.append(g.read(p["indices"]).reshape(-1, 3).astype(np.int64) + at); at += len(Ps[-1])
        Js.append(g.read(p["attributes"]["JOINTS_0"]).astype(np.uint16)); Ws.append(g.read(p["attributes"]["WEIGHTS_0"]).astype(np.float32))
    P, Nr, T, Jn, Wt = (np.concatenate(x) for x in (Ps, Ns, Ts, Js, Ws))
    cl = Cloth(P, T, Nr)
    from scipy.spatial import cKDTree
    tree = cKDTree(P)
    cloth_mat = j["materials"][base.get("material", 0)] if j.get("materials") else {}
    made = []

    def emit(V, F, UV, mspec, label, src, rigid=None):
        if len(F) == 0:
            print(f"trims: {label}: nothing of it lies on the cloth (missed)")
            return
        V, F = np.asarray(V, np.float64), np.asarray(F, np.int64)
        F = wound(V, F, tree, Nr)
        Nv = sheet_normals(V, F, tree, Nr)
        _, nn = tree.query(V)
        if rigid is not None:           # every vertex skinned as the one cloth vertex it hangs from: it moves whole
            nn = np.full(len(V), tree.query(np.asarray(rigid, np.float64))[1])
        j.setdefault("materials", []).append(material(mspec, cloth_mat, label))
        prim = {"attributes": {"POSITION": g.accessor(V.astype(np.float32), 34962, bounds=True), "NORMAL": g.accessor(Nv, 34962),
                               "TEXCOORD_0": g.accessor(np.asarray(UV, np.float32), 34962), "JOINTS_0": g.accessor(Jn[nn], 34962),
                               "WEIGHTS_0": g.accessor(Wt[nn], 34962)},
                "indices": g.accessor(F.astype(np.uint32).reshape(-1), 34963), "material": len(j["materials"]) - 1,
                "extras": {"trim": label, **({"source": src} if src else {})}}
        mesh["primitives"].append(prim)
        made.append(label)

    for n, t in enumerate(spec["trims"]):
        kind, src = t["kind"], t.get("source", "")
        label = t.get("name", f"{kind}-{n}")
        mirrors = [1, -1] if t.get("mirror") else [1]
        if kind == "button":
            d = DIRS[t["from"]] if isinstance(t["from"], str) else t["from"]
            d = unit(d)
            spots = [t["at"]] + [[t["at"][0] + o[0], t["at"][1] + o[1]] + list(t["at"][2:]) for o in t.get("copies", [])]
            Vs, Fs, UVs = [], [], []
            for sx in mirrors:
                for at in spots:
                    at = np.array([sx * at[0], at[1], at[2] if len(at) > 2 else 0.0])
                    dd = d * [sx, 1, 1]
                    if len(t["at"]) == 2:          # from outside along d: start well clear
                        at = at - dd * 0.6
                        if t["from"] in ("front", "back"):
                            at[2] = -dd[2] * 0.6
                    h = cl.onto(at, dd)
                    if h is None:
                        print(f"trims: {label}: no cloth at {np.round(at, 3).tolist()}")
                        continue
                    V, F, UV = dome(h[0] + h[1] * t.get("over", 0), h[1], t.get("d", 0.022) / 2, t.get("h", t.get("d", 0.022) * 0.18))
                    Fs.append(F + sum(len(x) for x in Vs)); Vs.append(V); UVs.append(UV)
            if Vs:
                emit(np.concatenate(Vs), np.concatenate(Fs), np.concatenate(UVs), t.get("material", {}), label, src)
        elif kind == "patch":
            for sx in mirrors:
                c = np.array(t["centre"]) * [sx, 1, 1]
                d = np.array(DIRS[t["from"]] if isinstance(t["from"], str) else t["from"], np.float64) * [sx, 1, 1]
                u = np.array(t["along"], np.float64) * [sx, 1, 1]
                L, w = t["size"]
                if "piping" in t:
                    pw = t["piping"].get("w", 0.002)
                    Pg, Ng, ok = patch_grid(cl, c, d, u, [L + 2 * pw, w + 2 * pw], t.get("end", "square"))
                    V, F, UV = slab(Pg, Ng, ok, t.get("thick", 0.003) * 0.85)
                    emit(V, F, UV, t["piping"], f"{label}-piping{'-r' if sx < 0 else ''}", src)
                Pg, Ng, ok = patch_grid(cl, c, d, u, [L, w], t.get("end", "square"))
                V, F, UV = slab(Pg + Ng * t.get("over", 0), Ng, ok, t.get("thick", 0.003))
                emit(V, F, UV, t.get("material", {}), f"{label}{'-r' if sx < 0 else ''}", src)
        elif kind == "band":
            Pg, Ng, ok = band(cl, t["y"], t.get("h", 0.05), t.get("thick", 0.004), t.get("axis", [0, -0.02]), t.get("within", 0.32))
            V, F, UV = slab(Pg.transpose(1, 0, 2), Ng.transpose(1, 0, 2), ok.T, t.get("thick", 0.004))
            emit(V, F, UV, t.get("material", {}), label, src)
        elif kind == "sash":
            Pg, Ng, ok = sash(cl, t["lo"], t["hi"], t.get("w", 0.02))
            V, F, UV = slab(Pg.transpose(1, 0, 2), Ng.transpose(1, 0, 2), ok.T, t.get("thick", 0.003))
            emit(V, F, UV, t.get("material", {}), label, src)
        elif kind == "bag":
            (V, F, UV), frame, hang = bag(cl, t["at"], t["from"], t.get("size", [0.2, 0.22, 0.07]))
            emit(V, F, UV, t.get("material", {}), label, src, rigid=hang)
            fm = t.get("frame", {"colour": "#b9b6ad", "metal": 0.85, "rough": 0.35})
            V, F, UV = tube_mesh(frame, t.get("frame_r", 0.005))
            emit(V, F, UV, fm, f"{label}-frame", src, rigid=hang)
            mid = frame[len(frame) // 2]                   # the clasp: two knobs crossed over the frame's middle
            out_ = unit(np.cross(frame[-1] - frame[0], [0, 1.0, 0]))
            if (out_ * (mid - hang)).sum() < 0:
                out_ = -out_
            for dx in (-0.006, 0.006):
                kn = mid + unit(frame[-1] - frame[0]) * dx + np.array([0, 0.012, 0])
                V, F, UV = tube_mesh([kn - out_ * 0.004, kn + out_ * 0.004], 0.0055, 12)
                emit(V, F, UV, fm, f"{label}-clasp", src, rigid=hang)
        elif kind == "collar":
            for G, Ng, ok, part, mspec, th in collar(P, t):
                V, F, UV = slab(G.transpose(1, 0, 2), Ng.transpose(1, 0, 2), ok.T, th)
                emit(V, F, UV, mspec, f"{label}-{part}", src)
        else:
            raise SystemExit(f"trims: {label}: kind {kind} is button, patch, band, sash, bag or collar")
        if src:
            print(f"trims: {label} ({src})")
    size = g.save(path)
    print(f"trims: {len(made)} parts on {os.path.basename(path)} ({size / 1e6:.1f} MB)")


if __name__ == "__main__":
    main()
