# A garment's cloth as textures in its cut's UVs (bake_batch.py lays the pattern's panels flat in metres, the grain
# along v): a tileable weave height (a twill's diagonal ribs, or a plain weave's grid), the fibre fuzz over it, the
# colour as a melange of lighter and darker fibres, and from the height a normal map and a roughness map. Made in
# numpy with periodic noise (filtered in Fourier space), so a 1 m tile repeats with no seam. A metre of cloth is
# SIZE pixels (2048: 2 px a millimetre, a coarse twill's rib at 5 px; at a film's distances a pixel on screen covers
# a millimetre or two of cloth). Kinds:
#   wool   fulled broadcloth (uniform cloth, greatcoats, women's coats): the twill barely shows under the felted fuzz
#   serge  a hard-wearing worsted twill (trousers, skirts, tunics): the diagonal shows
#   cotton a plain weave (aprons, shirts, headscarves): a fine grid, smoother, less fuzz
#   felt   no weave at all (hats): fuzz alone
import numpy as np

KINDS = {
    #          rib pitch mm, rib height, plain weave?, fuzz, fuzz streak along the warp, melange, roughness lo, hi
    "wool":   dict(pitch=2.4, rib=0.18, plain=False, fuzz=0.55, streak=2.5, melange=0.07, r0=0.86, r1=0.99),
    "serge":  dict(pitch=2.8, rib=0.65, plain=False, fuzz=0.22, streak=4.0, melange=0.05, r0=0.74, r1=0.92),
    "cotton": dict(pitch=1.2, rib=0.45, plain=True,  fuzz=0.12, streak=1.5, melange=0.03, r0=0.80, r1=0.95),
    "felt":   dict(pitch=2.0, rib=0.0,  plain=False, fuzz=0.85, streak=1.0, melange=0.05, r0=0.90, r1=1.00),
}


def noise(size, rng, scale_px, beta=2.0, stretch=1.0):
    """Periodic noise: white noise shaped by 1/f^beta above a feature size of `scale_px`, stretched along v
    (rows) by `stretch` (fibres lie along the warp). Zero mean, unit spread."""
    f = np.fft.fftfreq(size)
    fu, fv = np.meshgrid(f, f * stretch)
    r = np.sqrt(fu ** 2 + fv ** 2)
    r[0, 0] = 1
    amp = np.where(r < 1 / scale_px, 1.0, (r * scale_px) ** (-beta / 2))
    amp[0, 0] = 0
    w = rng.standard_normal((size, size)) + 1j * rng.standard_normal((size, size))
    n = np.real(np.fft.ifft2(w * amp))
    return (n - n.mean()) / (n.std() + 1e-9)


def weave(kind, rgb, size=2048, seed=1):
    """(albedo uint8 HxWx3 sRGB, normal uint8 HxWx3, roughness-metal uint8 HxWx3 as glTF packs it) for 1 m of cloth
    of `kind` in sRGB colour `rgb` (0..1)."""
    k = KINDS[kind]
    rng = np.random.default_rng(seed)
    u, v = np.meshgrid(np.arange(size) / size, np.arange(size) / size)
    ribs = round(1000 / k["pitch"])                       # ribs a metre: a whole number, so the tile is seamless
    if k["plain"]:
        h = 0.5 * (np.sin(2 * np.pi * ribs * u) * np.sin(2 * np.pi * ribs * v))
    else:                                                  # a 2/2 twill: ribs at 45 degrees, rounded
        h = 0.5 + 0.5 * np.sin(2 * np.pi * ribs * (u + v))
        h = h ** 1.5
    px_mm = size / 1000
    fuzz = noise(size, rng, 0.6 * px_mm, 2.2, k["streak"])         # fibres a little under a millimetre
    clump = noise(size, rng, 6 * px_mm, 2.6, 1.5)                  # the felting's clumps, a few millimetres
    height = k["rib"] * h + k["fuzz"] * (0.35 * fuzz + 0.2 * clump)
    # the colour: a melange (lighter and darker fibres), darker in the weave's hollows, a faint cloud across the tile
    cloud = noise(size, rng, 120 * px_mm, 3.0, 1.0)
    fib = noise(size, rng, 0.8 * px_mm, 1.6, k["streak"] * 1.5)
    lin = np.asarray(rgb, np.float64) ** 2.2
    shade = 1 + k["melange"] * (1.6 * fib + 0.6 * cloud) - 0.12 * k["rib"] * (1 - h)
    albedo = np.clip(lin[None, None, :] * shade[..., None], 0, 1) ** (1 / 2.2)
    # normal from the height (mm of relief: a rib stands about a tenth of a millimetre)
    relief = height * 0.12
    dx = (np.roll(relief, -1, 1) - np.roll(relief, 1, 1)) / 2 * px_mm
    dy = (np.roll(relief, -1, 0) - np.roll(relief, 1, 0)) / 2 * px_mm
    n = np.dstack([-dx, dy, np.ones_like(dx)])
    n /= np.linalg.norm(n, axis=2, keepdims=True)
    normal = (n * 0.5 + 0.5)
    # rougher on the fuzz and in the hollows, a touch smoother on worn rib tops
    t = np.clip(0.5 + 0.35 * fuzz * k["fuzz"] - 0.25 * (h - 0.5) * k["rib"], 0, 1)
    rough = k["r0"] + (k["r1"] - k["r0"]) * t
    mr = np.dstack([np.zeros_like(rough), rough, np.zeros_like(rough)])   # glTF: G roughness, B metal
    to8 = lambda a: (np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)
    return to8(albedo), to8(normal), to8(mr)


GREY = 0.8   # a cover's albedo map's mean (linear): a material's baseColorFactor carries the colour over it


def lin(colour):
    return [(int(colour[i:i + 2], 16) / 255) ** 2.2 for i in (1, 3, 5)]


def cover(glb, kind, size=2048, seed=None, colour=None):
    """Re-cover a sewn garment's glb (bake_batch.py: its UVs are the cut laid flat in metres) in cloth of `kind`: the
    weave's albedo (a grey melange, the colour kept in baseColorFactor so a <MeshStandardMaterial color> still
    recolours it), normal and roughness maps, 1 m of cloth a tile, repeated. Its own colour unless `colour` (#rrggbb).
    Trims in the garment's cloth (trims.py) keep their colours over the new maps."""
    import io, os, sys, zlib
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from glbio import Glb
    from PIL import Image
    g = Glb(glb)
    j = g.j
    mats = j.setdefault("materials", [])
    if not mats:
        mats.append({})
    m0 = mats[0]
    if colour:
        own = lin(colour)
    elif m0.get("extras", {}).get("colour"):
        own = lin(m0["extras"]["colour"])
    else:
        f = (m0.get("pbrMetallicRoughness", {}).get("baseColorFactor") or [0.2, 0.2, 0.2, 1])[:3]
        own = [c * GREY for c in f] if "baseColorTexture" in m0.get("pbrMetallicRoughness", {}) else f
    seed = zlib.crc32(os.path.basename(glb).encode()) % 100000 if seed is None else seed
    a, n, r = weave(kind, [GREY ** (1 / 2.2)] * 3, size, seed)
    j.pop("images", None); j.pop("textures", None)
    j["samplers"] = [{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}]

    def add(img):
        buf = io.BytesIO()
        Image.fromarray(img).save(buf, "JPEG", quality=92, subsampling=0)
        j.setdefault("textures", []).append({"sampler": 0, "source": g.image(buf.getvalue(), "image/jpeg")})
        return len(j["textures"]) - 1

    ta, tn, tr = add(a), add(n), add(r)
    hexed = lambda c: "#" + "".join(f"{round(255 * x ** (1 / 2.2)):02x}" for x in c)
    def own_of(m):           # a panel's own colour (bake_batch.py PANELS): kept, as the garment's is
        ex, pbr = m.get("extras", {}), m.get("pbrMetallicRoughness", {})
        if ex.get("colour"):
            return lin(ex["colour"])
        f = (pbr.get("baseColorFactor") or [0.2, 0.2, 0.2, 1])[:3]
        return [c * GREY for c in f] if "baseColorTexture" in pbr else f
    for i, m in enumerate(mats):
        ex = m.setdefault("extras", {})
        trim = ex.get("trim")
        if trim and not ex.get("cloth"):
            continue
        c = lin(ex["colour"]) if trim else own if i == 0 else own_of(m)
        pbr = m.setdefault("pbrMetallicRoughness", {})
        pbr["baseColorFactor"] = [min(1.0, x / GREY) for x in c] + [1.0]
        pbr["baseColorTexture"] = {"index": ta}
        pbr["metallicRoughnessTexture"] = {"index": tr}
        pbr["metallicFactor"], pbr["roughnessFactor"] = 0.0, 1.0
        m["normalTexture"] = {"index": tn, "scale": 1.0}
        ex["cloth"] = kind
        if not trim:
            ex["colour"] = hexed(c)
    return g.save(glb)

if __name__ == "__main__":
    # python fabric.py GARMENT.glb KIND [--size PX] [--colour #rrggbb]   (KIND: wool, serge, cotton or felt)
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("glb"); ap.add_argument("kind", choices=sorted(KINDS))
    ap.add_argument("--size", type=int, default=2048); ap.add_argument("--colour"); ap.add_argument("--seed", type=int)
    x = ap.parse_args()
    size = cover(x.glb, x.kind, x.size, x.seed, x.colour)
    print(f"cloth: {x.glb} covered in {x.kind} ({x.size} px a metre), {size / 1e6:.1f} MB")
