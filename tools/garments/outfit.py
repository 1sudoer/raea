# An outfit from words: the clothes in a character's description ("a long mustard-yellow oilskin coat to the knee,
# worn open, over a navy cable-knit jumper with a roll neck; dark brown wool trousers; black rubber boots to
# mid-calf") as the garments `raea clothes` makes, innermost first, each with how it is made.
#
#   python3 outfit.py "WORDS" [--prefix NAME]     ->  JSON on stdout: {"garments": [...], "unread": [...]}
#
# Each garment is {"name", "kind", "words", "colour" (#rrggbb), "layer", "verb"} and, by its verb:
#   make   sewn from a pattern (CH47a): "set" holds the settings (paths as `raea clothes settings` lists them), on
#          GarmentCode's plain base. The words model (CH48c) is not asked: measured, it is a first guess ("straight
#          trousers" came out culottes, a turtleneck brought trousers along), so a garment named here is cut from
#          the settings its words call for, and the words are kept with it
#   cover  laid on the skin by procedure (`raea clothes cover`): "parts" (a limb's share is how far down it from its
#          top) and "stand" (mm off the skin). Footwear and socks: a shoe is not sewn
# A clause with no garment word this knows is returned in "unread", never guessed.
#
# What words do: the garment's noun picks its kind (the table below); a colour before the noun (a "with ..." clause
# is the garment's trim and does not colour it) picks its colour; length words ("to the knee", "long", "cropped",
# "to mid-calf", "ankle") and neck words ("roll neck", "crew-neck", "v-neck", "hood") change its settings. "X over
# Y" is two garments, Y under X; "worn open" is noted on the garment (GarmentCode cuts no open front: the outer
# top gets a deep V neck to its hem, the nearest the patterns come).
import json, re, sys

COLOURS = {  # sRGB; a compound ("olive-green", "mustard-yellow") is looked up whole first
    "white": "#f0efea", "cream": "#ece3cf", "ivory": "#eee8d6", "black": "#1d1d20", "grey": "#85868a", "gray": "#85868a",
    "silver": "#b8b9bd", "charcoal": "#3b3c40", "navy": "#1f2a46", "blue": "#2f5ea8", "indigo": "#2a3352",
    "denim": "#3d5578", "teal": "#2a8a8a", "green": "#3f6b3a", "olive-green": "#5d6238", "olive": "#5d6238",
    "khaki": "#a99c6c", "mustard-yellow": "#cf9d22", "mustard": "#cf9d22", "yellow": "#e3c13a", "orange": "#e0702c",
    "red": "#b5322c", "burgundy": "#6d2230", "pink": "#e39aa8", "purple": "#5e3f86", "brown": "#6a4a30",
    "tan": "#b48f66", "beige": "#d6c3a2", "camel": "#bf9762", "rust": "#a5502a",
}
SHADES = {"dark": 0.6, "deep": 0.65, "light": 1.35, "pale": 1.45, "bright": 1.1}

# kind: (nouns, verb, layer, settings or cover). On SOMA, shirt.length 1.2 ends at the ribs (a crop top: the first
# tees, 11-16), 1.6 to 1.7 at the hip, 3.0 at the knee. Layer: 0 next to the skin (socks), 1 tops and bottoms, 2 footwear,
# 3 outer tops. A noun is matched whole, the longest first
KINDS = [
    ("socks", ["tube socks", "socks", "stockings"], "cover", 0, {"parts": "toes,foot,shin:0.35", "stand": 0.5}),
    ("tee", ["t-shirt", "tee", "tshirt", "vest top", "tank top"], "make", 1,
     ["meta.upper=FittedShirt", "sleeve.length=0.3", "shirt.length=1.65", "shirt.width=1.05"]),
    ("shirt", ["shirt", "blouse", "polo"], "make", 1, ["meta.upper=Shirt", "sleeve.length=1.05", "shirt.length=1.7"]),
    ("jumper", ["jumper", "sweater", "pullover", "sweatshirt", "hoodie", "cardigan"], "make", 1,
     # (no cuff band: GarmentCode's is a tube narrower than SOMA's forearm, which slid off and split the sleeve
     # open on chars/wren's jumper, as on CH48c's shirt)
     ["meta.upper=Shirt", "sleeve.length=1.05", "shirt.length=1.7", "shirt.width=1.1", "sleeve.end_width=0.65"]),
    ("trousers", ["trousers", "jeans", "pants", "chinos", "slacks", "joggers", "leggings"], "make", 1,
     ["meta.upper=none", "meta.bottom=Pants", "meta.wb=StraightWB", "pants.length=0.85", "pants.width=1.05"]),
    ("shorts", ["shorts"], "make", 1, ["meta.upper=none", "meta.bottom=Pants", "meta.wb=StraightWB", "pants.length=0.3", "pants.width=1.1"]),
    ("skirt", ["skirt"], "make", 1, ["meta.upper=none", "meta.bottom=PencilSkirt", "meta.wb=StraightWB", "pencil-skirt.length=0.5"]),
    ("boots", ["boots", "wellingtons", "wellies"], "cover", 2, {"parts": "toes,foot,shin:0.25", "stand": 4}),
    ("shoes", ["sneakers", "trainers", "shoes", "loafers", "brogues", "sandals"], "cover", 2, {"parts": "toes,foot", "stand": 5}),
    ("coat", ["coat", "overcoat", "raincoat", "parka", "mac", "trench"], "make", 3,
     ["meta.upper=Shirt", "sleeve.length=1.1", "shirt.length=2.6", "shirt.width=1.2", "shirt.flare=1.15", "sleeve.end_width=0.75"]),
    ("jacket", ["field jacket", "bomber jacket", "jacket", "blazer", "bomber", "overshirt", "anorak"], "make", 3,
     ["meta.upper=Shirt", "sleeve.length=1.1", "shirt.length=1.8", "shirt.width=1.2", "sleeve.end_width=0.7"]),
]
NOUNS = sorted(((n, k) for k in KINDS for n in k[1]), key=lambda x: -len(x[0]))


def colour_of(text):
    """the first colour named, with a shade word before it ("dark indigo"), as #rrggbb; None if there is none"""
    t = text.lower()
    found = []
    for name in sorted(COLOURS, key=len, reverse=True):
        for m in re.finditer(r"(?<![a-z-])" + re.escape(name) + r"(?![a-z-])", t):
            if not any(a <= m.start() < b for a, b, _ in found):
                found.append((m.start(), m.end(), name))
    if not found:
        return None
    a, _, name = min(found)
    rgb = [int(COLOURS[name][i:i + 2], 16) for i in (1, 3, 5)]
    before = t[:a].split()[-1:] or [""]
    k = SHADES.get(before[0], 1.0)
    return "#" + "".join(f"{max(0, min(255, round(c * k))):02x}" for c in rgb)


def garment(clause, prefix, used):
    """one garment from a clause naming one, or None"""
    low = clause.lower()
    hit = None
    for noun, k in NOUNS:
        m = re.search(r"(?<![a-z])" + re.escape(noun) + r"(?![a-z])", low)
        if m and (hit is None or m.start() < hit[0]):
            hit = (m.start(), noun, k)
    if hit is None:
        return None
    at, noun, (kind, _, verb, layer, spec) = hit
    body = low.split(" with ")[0] if " with " in low[:at + len(noun) + 1] or " with " not in low else low
    head = low[:at + len(noun)]                                       # the colour is named before the noun
    col = colour_of(head) or colour_of(body) or "#8a8a8a"
    name = f"{prefix}-{kind}" if prefix else kind
    n = 2
    while name in used:
        name = f"{prefix}-{kind}{n}" if prefix else f"{kind}{n}"; n += 1
    used.add(name)
    g = {"name": name, "kind": kind, "words": clause.strip(), "colour": col, "layer": layer, "verb": verb}
    notes = []
    if verb == "make":
        s = dict(x.split("=", 1) for x in spec)
        if re.search(r"\bto the knee\b|knee-length|\blong\b", low) and kind in ("coat", "jacket", "shirt", "jumper"):
            s["shirt.length"] = "3.0" if kind == "coat" else "2.2"
        if re.search(r"\bcropped\b|\bshort\b", low) and kind in ("jacket", "jumper", "shirt", "tee"):
            s["shirt.length"] = "1.25"
        if kind == "jacket" and "bomber" in low:
            s["shirt.length"] = "1.5"
        if kind in ("trousers",) and re.search(r"\bslim\b|\bskinny\b", low):
            s["pants.width"] = "1.0"
        if kind in ("trousers",) and re.search(r"\bwide\b|\bbaggy\b|\bloose\b", low):
            s["pants.width"] = "1.35"
        if re.search(r"roll[- ]neck|turtle[- ]?neck|polo[- ]neck", low):
            # GarmentCode's Turtle collar falls in on itself in the drape (CH48c; chars/wren's jumper, 11-16): a high
            # round neck until the drape holds one up
            s["collar.f_collar"] = "CircleNeckHalf"; s["collar.fc_depth"] = "0.3"
            notes.append("roll neck made as a high round neck (a Turtle collar collapses in the drape)")
        if re.search(r"v[- ]neck", low):
            s["collar.f_collar"] = "VNeckHalf"
        if re.search(r"\bhood(ed|ie)?\b", low):
            s["collar.component.style"] = "Hood2Panels"
        if re.search(r"short[- ]sleeved?", low):
            s["sleeve.length"] = "0.3"
        if re.search(r"long[- ]sleeved?", low):
            s["sleeve.length"] = "1.05"
        if re.search(r"\b(worn )?open\b|\bunbuttoned\b|\bunzipped\b", low) and layer == 3:
            s["collar.f_collar"] = "VNeckHalf"; s["collar.fc_depth"] = "2"
            notes.append("worn open: a deep V to the hem (GarmentCode cuts no open front)")
        g["set"] = [f"{k}={v}" for k, v in s.items()]
    else:
        c = dict(spec)
        if kind == "boots":
            if re.search(r"\bankle\b", low):
                c["parts"] = "toes,foot,shin:0.15"
            elif re.search(r"mid[- ]calf", low):
                c["parts"] = "toes,foot,shin:0.5"
            elif re.search(r"\bknee\b", low):
                c["parts"] = "toes,foot,shin:0.95"
        if kind == "socks" and re.search(r"\btube\b|\bknee\b", low):
            c["parts"] = "toes,foot,shin:0.45"
        if kind in ("boots", "shoes"):
            # footwear is a hard piece (`raea clothes cover --hard`): its own shape on a last, a sole under it; a
            # chunky or platform sole is thicker, a boot's rubber or a sneaker's sole its own colour, and the panels
            # named ("with blue, yellow and red panels") a toe cap, a heel and a band in that order
            c["hard"] = True
            c["sole"] = 30 if re.search(r"\bchunky\b|\bplatform\b|\bthick[- ]soled\b", low) else 22 if kind == "shoes" and re.search(r"sneaker|trainer", low) else 12
            if re.search(r"sneaker|trainer", low):
                c["sole_colour"] = "#f4f1ea"
            elif re.search(r"rubber|welly|wellies|wellington", low):
                c["sole_colour"] = "#262422"
            pm = re.search(r"with ([a-z ,]+?) (?:panels|stripes|accents)", low)
            if pm:
                cs = [colour_of(w) for w in re.split(r",\s*|\s+and\s+", pm.group(1))]
                c["panels"] = [x for x in cs if x]
        g.update(c)
    trims = re.findall(r"with (.+)$", re.sub(r"\s+worn open$", "", clause.strip()), flags=re.I)
    made = r"roll[- ]neck|turtle[- ]?neck|polo[- ]neck|v[- ]neck|hood|crew[- ]neck"
    if trims and not re.fullmatch(r"(an? )?(" + made + r")", trims[0].strip().lower()) and not g.get("panels"):
        notes.append("trim not made yet: " + trims[0])
    if notes:
        g["notes"] = notes
    return g


def outfit(text, prefix=""):
    # sentences, then clauses; "X over Y" is two garments
    clauses = []
    for part in re.split(r";|\.(?:\s|$)", text):
        part = part.strip()
        if not part:
            continue
        bits = re.split(r",?\s+(?:worn\s+(?:open\s+)?)?over\s+", part)
        if len(bits) == 2 and all(any(re.search(r"(?<![a-z])" + re.escape(n) + r"(?![a-z])", b.lower()) for n, _ in NOUNS) for b in bits):
            open_ = re.search(r"\bopen\b", part.lower()) is not None
            outer = bits[0] + (" worn open" if open_ and "open" not in bits[0].lower() else "")
            clauses += [bits[1], outer]
        else:
            clauses.append(part)
    used, out, unread = set(), [], []
    for c in clauses:
        # a clause naming several garments by commas ("dark jeans, brown boots") is split at its commas
        sub = [c]
        if sum(1 for n, _ in NOUNS if re.search(r"(?<![a-z])" + re.escape(n) + r"(?![a-z])", c.lower())) > 1 and " with " not in c.lower():
            sub = [s for s in re.split(r",\s*(?:and\s+)?|\s+and\s+", c) if s.strip()]
        for s in sub:
            g = garment(s, prefix, used)
            if g:
                out.append(g)
            elif s.strip():
                unread.append(s.strip())
    out.sort(key=lambda g: g["layer"])
    # footwear laid on the skin goes over socks laid on it (`raea clothes cover --over`)
    socks = [g["name"] for g in out if g["kind"] == "socks"]
    for g in out:
        if g["verb"] == "cover" and g["kind"] in ("boots", "shoes") and socks:
            g["over"] = socks[-1]
    return {"garments": out, "unread": unread}


if __name__ == "__main__":
    a = sys.argv[1:]
    prefix = a[a.index("--prefix") + 1] if "--prefix" in a else ""
    words = [x for i, x in enumerate(a) if x != "--prefix" and (i == 0 or a[i - 1] != "--prefix")]
    print(json.dumps(outfit(" ".join(words), prefix), indent=1))
