# A GarmentCode design's settings as one vector and back (docs/character.feature CH48c): the garment model's labels.
# The schema is read from any design.yaml (every design holds the same tree): a float or int is its place in its range
# (0..1), a bool is 0 or 1, a select is one-hot over its range, and a select_null has one more slot for "none".
#   from garment_params import Schema; s = Schema.of(design); x = s.encode(design); d = s.decode(x)
import copy, json
import numpy as np


def _leaves(d, path=()):
    for k, v in d.items():
        if isinstance(v, dict) and "type" in v and "v" in v: yield path + (k,), v
        elif isinstance(v, dict): yield from _leaves(v, path + (k,))


def _get(d, path):
    for k in path: d = d[k]
    return d


class Schema:
    def __init__(self, fields, template):
        self.fields, self.template = fields, template  # fields: (path, kind, range, slot, width)
        self.width = sum(f[4] for f in fields)

    @staticmethod
    def of(designs):
        """from one design or several: a choice's options are every listed and every value seen (a design can hold
        GarmentCode's default outside its own range, a front collar of CircleNeckHalf), a number's range takes in its
        values"""
        designs = designs if isinstance(designs, list) else [designs]
        design = designs[0]
        fields, at = [], 0
        for path, leaf in _leaves(design):
            kind, rng = leaf["type"], list(leaf["range"])
            seen = [_get(d, path)["v"] for d in designs]
            if kind in ("select", "select_null"):
                for v in seen:
                    if v is not None and v not in rng: rng.append(v)
            elif kind in ("float", "int"):
                rng = [min([float(rng[0])] + [float(v) for v in seen]), max([float(rng[-1])] + [float(v) for v in seen])]
            if kind in ("float", "int"): w = 1
            elif kind == "bool": w = 1
            elif kind == "select": w = len(rng)
            elif kind == "select_null": w = len(rng) + 1
            else: raise ValueError(f"{'.'.join(path)}: a setting of type {kind}")
            fields.append((path, kind, rng, at, w)); at += w
        return Schema(fields, copy.deepcopy(design))

    def groups(self):
        """slices of the vector by how each is learned: ('scalar', slot) or ('choice', slot, width)"""
        return [("scalar", f[3]) if f[1] in ("float", "int", "bool") else ("choice", f[3], f[4]) for f in self.fields]

    def encode(self, design):
        x = np.zeros(self.width, np.float32)
        for path, kind, rng, at, w in self.fields:
            v = _get(design, path)["v"]
            if kind in ("float", "int"):
                lo, hi = float(rng[0]), float(rng[-1]); x[at] = 0.0 if hi == lo else np.clip((float(v) - lo) / (hi - lo), 0, 1)
            elif kind == "bool": x[at] = 1.0 if v else 0.0
            else: x[at + (rng.index(v) if v in rng else len(rng))] = 1.0
        return x

    def decode(self, x):
        """a design from a vector: scalars clamped into range (ints rounded), each choice its highest slot"""
        d = copy.deepcopy(self.template)
        for path, kind, rng, at, w in self.fields:
            leaf = _get(d, path)
            if kind in ("float", "int"):
                lo, hi = float(rng[0]), float(rng[-1]); v = lo + float(np.clip(x[at], 0, 1)) * (hi - lo)
                leaf["v"] = int(round(v)) if kind == "int" else v
            elif kind == "bool": leaf["v"] = bool(x[at] > 0.5)
            else:
                i = int(np.argmax(x[at:at + w])); leaf["v"] = rng[i] if i < len(rng) else None
        return d

    def dump(self):
        return json.dumps({"fields": [[list(p), k, r, a, w] for p, k, r, a, w in self.fields], "template": self.template})

    @staticmethod
    def load(s):
        j = json.loads(s)
        return Schema([(tuple(p), k, r, a, w) for p, k, r, a, w in j["fields"]], j["template"])

    def name(self, at):
        for path, kind, rng, a, w in self.fields:
            if a <= at < a + w: return ".".join(path)
