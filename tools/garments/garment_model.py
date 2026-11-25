# The garment model's first form (docs/character.feature CH48c): words to a GarmentCode design, through SigLIP 2's
# shared space, trained with no captions. A small network learns each design's settings (garment_params.py) from
# SigLIP 2 embeddings of its pictures on the mannequin; asked with words, it is given the words' text embedding instead.
# The two kinds of embedding sit apart in SigLIP's space by a near-constant offset, so each is centred on its own kind's
# mean (the pictures', and a bank of garment lines') and set to unit length, and noise is added to the pictures'
# while training so the network answers for the neighbourhood of a picture, where a line's embedding lands (LAFITE).
#   python -I garment_model.py train DATA MODEL.npz     DATA holds embeds.npz (garment_embed.py) and g*/design.yaml
#   python -I garment_model.py eval DATA MODEL.npz      held-out designs from their pictures, and lines of words
#   python -I garment_model.py make MODEL.npz "words" OUT_DIR    OUT_DIR/design.yaml, ready for GarmentCode
import json, os, sys, zlib
import numpy as np, yaml
import mlx.core as mx, mlx.nn as nn, mlx.optimizers as optim
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from garment_params import Schema, _get
from garment_embed import text_embed

H, NOISE, STEPS = 512, float(os.environ.get("NOISE", "0.6")), int(os.environ.get("STEPS", "3000"))
# lines a garment can be asked for: the text side's mean is taken over these and the designs' own captions
BANK = ["a dress", "a skirt", "trousers", "a shirt", "a top", "a blouse", "a gown", "a long skirt", "a short skirt",
        "a sleeveless dress", "a fitted top", "a loose shirt", "wide trousers", "a hooded top", "a turtleneck",
        "a strapless dress", "a pleated skirt", "a pencil skirt", "a circle skirt", "a tiered skirt", "shorts"]
# lines of words with what each must give (a value, a list of values that count, or a test), for eval
PROBES = [
    ("trousers", {"meta.bottom": "Pants"}),
    ("a pair of trousers and a fitted shirt", {"meta.bottom": "Pants", "meta.upper": ["FittedShirt", "Shirt"]}),
    ("a pencil skirt", {"meta.bottom": "PencilSkirt"}),
    ("a full circle skirt", {"meta.bottom": ["SkirtCircle", "AsymmSkirtCircle"]}),
    ("a tiered skirt with layers", {"meta.bottom": "SkirtLevels"}),
    ("a skirt and no top", {"meta.upper": None, "meta.bottom": ["SkirtCircle", "AsymmSkirtCircle", "GodetSkirt", "Skirt2", "SkirtManyPanels", "PencilSkirt", "SkirtLevels"]}),
    ("a top with a hood", {"collar.component.style": "Hood2Panels"}),
    ("a turtleneck", {"collar.component.style": "Turtle"}),
    ("a sleeveless top", {"sleeve.sleeveless": True}),
    ("a long-sleeved shirt", {"sleeve.sleeveless": False, "sleeve.length": lambda v: v > 0.7}),
    ("a strapless top", {"shirt.strapless": True}),
    ("a long dress", {"meta.upper": ["FittedShirt", "Shirt"], "meta.bottom": ["SkirtCircle", "AsymmSkirtCircle", "GodetSkirt", "Skirt2", "SkirtManyPanels", "PencilSkirt", "SkirtLevels"]}),
]


def unit(x): return x / np.maximum(np.linalg.norm(x, axis=-1, keepdims=True), 1e-9)


def hung_right(data, n):
    """a design whose drape passed and left no seam open over 2 cm (an open cuff would teach the look of a fault)"""
    try: r = json.load(open(f"{data}/{n}/{n}.drape.json"))
    except OSError: return False
    return r["ok"] and r["seam_gap_mm"]["max"] < 20


def load(data):
    Z = dict(np.load(f"{data}/embeds.npz"))
    good = {n for n in {str(n) for n in Z["names"]} if hung_right(data, n)}
    keep = np.array([str(n) in good for n in Z["names"]])
    Z = {k: v[keep] for k, v in Z.items()}
    names = sorted(good)
    designs = {n: yaml.safe_load(open(f"{data}/{n}/design.yaml"))["design"] for n in names}
    captions = [open(f"{data}/{n}/caption.txt").read().strip() for n in names if os.path.exists(f"{data}/{n}/caption.txt")]
    return Z, names, designs, captions


def held_out(name): return zlib.crc32(name.encode()) % 7 == 0


class Net(nn.Module):
    def __init__(self, width):
        super().__init__()
        self.l1, self.l2, self.out = nn.Linear(768, H), nn.Linear(H, H), nn.Linear(H, width)
        self.drop = nn.Dropout(0.1)

    def __call__(self, x):
        return self.out(self.drop(nn.gelu(self.l2(self.drop(nn.gelu(self.l1(x)))))))


def parts(schema):
    """the vector's slots by how they are learned: numbers (squared error on a sigmoid), bools (cross entropy), choices"""
    num = [f[3] for f in schema.fields if f[1] in ("float", "int")]
    bol = [f[3] for f in schema.fields if f[1] == "bool"]
    cho = [(f[3], f[4]) for f in schema.fields if f[1] in ("select", "select_null")]
    return np.array(num), np.array(bol), cho


def predict(net, schema, X):
    z = np.array(net(mx.array(X)))
    num, bol, cho = parts(schema); y = np.zeros_like(z)
    y[:, num] = 1 / (1 + np.exp(-z[:, num])); y[:, bol] = (z[:, bol] > 0).astype(np.float32)
    for a, w in cho: y[np.arange(len(z)), a + z[:, a:a + w].argmax(1)] = 1
    return y


def train(data, model_path):
    Z, names, designs, captions = load(data)
    schema = Schema.of([designs[n] for n in names])
    keep = np.array([not held_out(str(n)) for n in Z["names"]])
    mu_i = Z["E"][keep].mean(0); mu_t = text_embed(BANK + captions).mean(0)
    X = unit(Z["E"][keep] - mu_i); Y = np.stack([schema.encode(designs[str(n)]) for n in Z["names"][keep]])
    num, bol, cho = parts(schema)
    net = Net(schema.width); opt = optim.AdamW(1e-3, weight_decay=1e-2)
    numa, bola = mx.array(num), mx.array(bol)

    def loss(net, x, y):
        z = net(x)
        l = mx.mean((mx.sigmoid(z[:, numa]) - y[:, numa]) ** 2) * 4
        l = l + mx.mean(nn.losses.binary_cross_entropy(z[:, bola], y[:, bola], with_logits=True))
        for a, w in cho: l = l + mx.mean(nn.losses.cross_entropy(z[:, a:a + w], mx.argmax(y[:, a:a + w], axis=1))) / len(cho) * 4
        return l

    step = nn.value_and_grad(net, loss); rng = np.random.default_rng(0)
    for s in range(STEPS):
        b = rng.integers(0, len(X), 128); e = rng.standard_normal((len(b), 768)).astype(np.float32)
        x = unit(X[b] + NOISE * unit(e))  # a picture's neighbourhood, where a line's embedding lands
        l, g = step(net, mx.array(x), mx.array(Y[b])); opt.update(net, g); mx.eval(net.parameters(), opt.state)
        if s % 1000 == 0 or s == STEPS - 1: print(f"train: step {s} loss {float(l):.3f}", flush=True)
    net.eval()
    w = {k: np.array(v) for k, v in __import__("mlx.utils", fromlist=["tree_flatten"]).tree_flatten(net.parameters())}
    # each training design with its pictures' mean embedding: the nearest one stands in when GarmentCode refuses
    # what the network decodes
    tn = np.array([str(n) for n in Z["names"][keep]]); kept = sorted(set(tn))
    near_E = unit(np.stack([X[tn == n].mean(0) for n in kept])); near_Y = np.stack([schema.encode(designs[n]) for n in kept])
    np.savez(model_path, mu_i=mu_i, mu_t=mu_t, schema=schema.dump(), near_E=near_E, near_Y=near_Y, **{f"w:{k}": v for k, v in w.items()})
    print(f"train: {len(names)} designs ({int(keep.sum())} pictures kept for training), {schema.width} slots -> {model_path}")


def restore(model_path):
    M = np.load(model_path)
    schema = Schema.load(str(M["schema"]))
    net = Net(schema.width); net.load_weights([(k[2:], mx.array(M[k])) for k in M.files if k.startswith("w:")]); net.eval()
    return M, net, schema


def ok(want, got):
    if callable(want): return got is not None and want(got)
    return got in want if isinstance(want, list) else got == want


def evaluate(data, model_path):
    Z, names, designs, captions = load(data)
    M, net, schema = restore(model_path)
    E, N = Z["E"], [str(n) for n in Z["names"]]
    test = np.array([held_out(n) for n in N]); train_ = ~test
    Xt = unit(E[test] - M["mu_i"]); P = predict(net, schema, Xt)
    Yt = np.stack([schema.encode(designs[n]) for n in np.array(N)[test]])
    Ytr = np.stack([schema.encode(designs[n]) for n in np.array(N)[train_]])
    # the nearest training picture's design, for comparison
    near = Ytr[(Xt @ unit(E[train_] - M["mu_i"]).T).argmax(1)]
    print(f"eval: {int(test.sum())} held-out pictures of {len({n for n, t in zip(N, test) if t})} designs")
    num, bol, cho = parts(schema)
    for label, path in [("upper", "meta.upper"), ("bottom", "meta.bottom"), ("waistband", "meta.wb"), ("collar", "collar.component.style"), ("sleeveless", "sleeve.sleeveless")]:
        f = next(f for f in schema.fields if ".".join(f[0]) == path); a, w = f[3], f[4]
        acc = lambda Y: float((Y[:, a:a + w].argmax(1) == Yt[:, a:a + w].argmax(1)).mean()) if w > 1 else float((Y[:, a] == Yt[:, a]).mean())
        base = np.bincount(Ytr[:, a:a + w].argmax(1), minlength=w).argmax() if w > 1 else round(float(Ytr[:, a].mean()))
        prior = float((Yt[:, a:a + w].argmax(1) == base).mean()) if w > 1 else float((Yt[:, a] == base).mean())
        print(f"  {label:<11} model {acc(P):.2f}  nearest picture {acc(near):.2f}  most common {prior:.2f}")
    mae = lambda Y: float(np.abs(Y[:, num] - Yt[:, num]).mean())
    print(f"  numbers     model {mae(P):.3f}  nearest picture {mae(near):.3f}  mean {float(np.abs(Ytr[:, num].mean(0) - Yt[:, num]).mean()):.3f}  (mean error, 0..1 of each range)")
    T = unit(text_embed([p for p, _ in PROBES]) - M["mu_t"]); PT = predict(net, schema, T)
    NT = Ytr[(T @ unit(E[train_] - M["mu_i"]).T).argmax(1)]
    hits = {"model": 0, "nearest picture": 0}
    for (words, want), pm, pn in zip(PROBES, PT, NT):
        dm, dn = schema.decode(pm), schema.decode(pn)
        r = {k: all(ok(w, _get(d, tuple(p.split(".")))["v"]) for p, w in want.items()) for k, d in (("model", dm), ("nearest picture", dn))}
        for k in hits: hits[k] += r[k]
        print(f"  {words!r:<42} model {'yes' if r['model'] else 'no ':<3}  nearest picture {'yes' if r['nearest picture'] else 'no'}")
    print(f"  lines of words: model {hits['model']} of {len(PROBES)}, nearest picture {hits['nearest picture']} of {len(PROBES)}")


def make(model_path, words, out):
    """OUT/design.yaml from the network, and OUT/nearest.yaml, the training design nearest the words (the fallback)"""
    M, net, schema = restore(model_path)
    t = unit(text_embed([words]) - M["mu_t"])
    d = schema.decode(predict(net, schema, t)[0])
    os.makedirs(out, exist_ok=True); yaml.safe_dump({"design": d}, open(f"{out}/design.yaml", "w"), sort_keys=False)
    if "near_E" in M.files:
        yaml.safe_dump({"design": schema.decode(M["near_Y"][int((t @ M["near_E"].T).argmax())])}, open(f"{out}/nearest.yaml", "w"), sort_keys=False)
    print(f"make: {words!r} -> {out}/design.yaml (upper {_get(d, ('meta', 'upper'))['v']}, bottom {_get(d, ('meta', 'bottom'))['v']})")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "train": train(sys.argv[2], sys.argv[3])
    elif cmd == "eval": evaluate(sys.argv[2], sys.argv[3])
    elif cmd == "make": make(sys.argv[2], sys.argv[3], sys.argv[4])
    else: raise SystemExit(__doc__)
