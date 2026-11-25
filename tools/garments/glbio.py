# A garment's glb read and rewritten in place (fabric.py's cloth, trims.py's buttons and straps): accessors as numpy
# arrays, new ones and pictures added, and on save the buffer laid out again from what is still referenced, so a
# cover or a set of trims made again replaces the last one instead of piling up behind it.
import json
import struct

import numpy as np

CT = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
NC = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
TYPE = {1: "SCALAR", 2: "VEC2", 3: "VEC3", 4: "VEC4", 16: "MAT4"}
CODE = {np.dtype(v): k for k, v in CT.items()}


class Glb:
    def __init__(self, path):
        b = open(path, "rb").read()
        assert b[:4] == b"glTF", f"{path}: not a glb"
        n = struct.unpack("<I", b[12:16])[0]
        self.j = json.loads(b[20:20 + n])
        rest = b[20 + n:]
        blob = rest[8:8 + struct.unpack("<I", rest[:4])[0]] if len(rest) >= 8 else b""
        self.views = [blob[v.get("byteOffset", 0):v.get("byteOffset", 0) + v["byteLength"]] for v in self.j.get("bufferViews", [])]
        assert not any("sparse" in a for a in self.j.get("accessors", [])), "sparse accessors aren't read"

    def read(self, i):
        a = self.j["accessors"][i]
        v = self.j["bufferViews"][a["bufferView"]]
        dt, nc = np.dtype(CT[a["componentType"]]), NC[a["type"]]
        raw, off, stride = self.views[a["bufferView"]], a.get("byteOffset", 0), v.get("byteStride", 0) or dt.itemsize * nc
        x = np.ndarray((a["count"], nc), dt, raw, off, (stride, dt.itemsize)).copy()
        if a.get("normalized") and dt.kind in "ui":
            x = x.astype(np.float32) / np.iinfo(dt).max
        return x

    def view(self, data, target=None):
        self.j.setdefault("bufferViews", []).append({"buffer": 0, "byteLength": len(data), **({"target": target} if target else {})})
        self.views.append(bytes(data))
        return len(self.views) - 1

    def accessor(self, arr, target=None, bounds=False):
        arr = np.ascontiguousarray(arr)
        a = arr.reshape(len(arr), -1)
        acc = {"bufferView": self.view(a.tobytes(), target), "componentType": CODE[a.dtype], "count": len(a), "type": TYPE[a.shape[1]]}
        if bounds:
            acc["min"], acc["max"] = a.min(0).tolist(), a.max(0).tolist()
        self.j.setdefault("accessors", []).append(acc)
        return len(self.j["accessors"]) - 1

    def image(self, data, mime):
        self.j.setdefault("images", []).append({"bufferView": self.view(data), "mimeType": mime})
        return len(self.j["images"]) - 1

    def save(self, path):
        j = self.j
        used = sorted({a["bufferView"] for a in j.get("accessors", []) if "bufferView" in a} | {im["bufferView"] for im in j.get("images", []) if "bufferView" in im})
        remap, blob, views = {}, bytearray(), []
        for i in used:
            while len(blob) % 4:
                blob.append(0)
            v = dict(j["bufferViews"][i], byteOffset=len(blob), byteLength=len(self.views[i]), buffer=0)
            remap[i] = len(views)
            views.append(v)
            blob.extend(self.views[i])
        while len(blob) % 4:
            blob.append(0)
        for a in j.get("accessors", []):
            if "bufferView" in a:
                a["bufferView"] = remap[a["bufferView"]]
        for im in j.get("images", []):
            if "bufferView" in im:
                im["bufferView"] = remap[im["bufferView"]]
        j["bufferViews"], j["buffers"] = views, [{"byteLength": len(blob)}]
        self.views = [bytes(blob[v["byteOffset"]:v["byteOffset"] + v["byteLength"]]) for v in views]
        # accessors nothing uses any more (a trim's, dropped) go too, the rest renumbered where meshes and skins name them
        js = json.dumps(j, separators=(",", ":")).encode()
        js += b" " * (-len(js) % 4)
        out = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(blob))
        out += struct.pack("<II", len(js), 0x4E4F534A) + js + struct.pack("<II", len(blob), 0x004E4942) + bytes(blob)
        open(path, "wb").write(out)
        return len(blob)

    def drop_accessors(self, keep):
        """Keep only the accessors `keep` says (a set of indices), renumbering every reference to them."""
        j = self.j
        old = j.get("accessors", [])
        remap = {i: n for n, i in enumerate(sorted(keep))}
        j["accessors"] = [old[i] for i in sorted(keep)]
        for m in j.get("meshes", []):
            for p in m["primitives"]:
                p["attributes"] = {k: remap[v] for k, v in p["attributes"].items()}
                if "indices" in p:
                    p["indices"] = remap[p["indices"]]
                for t in p.get("targets", []):
                    for k in t:
                        t[k] = remap[t[k]]
        for s in j.get("skins", []):
            if "inverseBindMatrices" in s:
                s["inverseBindMatrices"] = remap[s["inverseBindMatrices"]]
        for an in j.get("animations", []):
            for s in an.get("samplers", []):
                s["input"], s["output"] = remap[s["input"]], remap[s["output"]]

    def used_accessors(self):
        j, used = self.j, set()
        for m in j.get("meshes", []):
            for p in m["primitives"]:
                used |= set(p["attributes"].values())
                if "indices" in p:
                    used.add(p["indices"])
                for t in p.get("targets", []):
                    used |= set(t.values())
        for s in j.get("skins", []):
            if "inverseBindMatrices" in s:
                used.add(s["inverseBindMatrices"])
        for an in j.get("animations", []):
            for s in an.get("samplers", []):
                used |= {s["input"], s["output"]}
        return used
