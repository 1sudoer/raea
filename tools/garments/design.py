# A GarmentCode design from a base design and settings by path: what `raea clothes make --set` writes.
#   python design.py BASE.yaml OUT.yaml meta.upper=Shirt sleeve.length=1.0 collar.component.style=Hood2Panels ...
#   python design.py BASE.yaml            prints every setting: its path, its value, and what it may be
# A choice must be one of its options and a number within its range; a wrong path or value says what is allowed.
import sys, yaml

base, *rest = sys.argv[1:]
d = yaml.safe_load(open(base))


def leaves(node, at=""):
    for k, v in node.items():
        if isinstance(v, dict) and "v" in v: yield at + k, v
        elif isinstance(v, dict): yield from leaves(v, at + k + ".")


def allowed(n):
    t, r = n.get("type", ""), n.get("range")
    if t == "bool": return "true or false"
    if t.startswith("select"): return "one of " + ", ".join("none" if x is None else str(x) for x in r)
    return f"{r[0]} to {r[1]}" if r else "any"


if not rest:
    for path, n in leaves(d["design"]):
        if not path.startswith("left."): print(f"{path} = {'none' if n['v'] is None else n['v']}   ({allowed(n)})")
    sys.exit(0)
out, *sets = rest
known = dict(leaves(d["design"]))
for s in sets:
    path, _, val = s.partition("=")
    if path not in known:
        near = [p for p in known if p.split(".")[0] == path.split(".")[0] and not p.startswith("left.")]
        sys.exit(f"design: no setting {path}" + (f"; under {path.split('.')[0]}: {', '.join(near)}" if near else "; `raea clothes settings` lists them"))
    node, v = known[path], yaml.safe_load(val)
    if isinstance(v, str) and v.lower() == "none": v = None
    t, r = node.get("type", ""), node.get("range")
    ok = (isinstance(v, bool) if t == "bool" else v in r if t.startswith("select") else
          isinstance(v, (int, float)) and not isinstance(v, bool) and (not r or r[0] <= v <= r[1]))
    if not ok: sys.exit(f"design: {path}={val}: it is {allowed(node)}")
    node["v"] = v
yaml.safe_dump(d, open(out, "w"), sort_keys=False)
print("design:", out, " ".join(sets))
