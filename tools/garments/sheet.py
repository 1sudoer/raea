# a row of pictures per garment, labelled: python sheet.py OUT.jpg DIR...
import sys, glob, os
from PIL import Image, ImageDraw
out, dirs = sys.argv[1], sys.argv[2:]
rows = []
for d in dirs:
    views = [p for v in ("front", "threequarter", "side", "back") for p in glob.glob(os.path.join(d, f"*_{v}.png"))]
    if not views: continue
    ims = [Image.open(p).convert("RGB") for p in views]
    w, h = ims[0].size; row = Image.new("RGB", (w * len(ims), h + 36), "white")
    for i, im in enumerate(ims): row.paste(im, (i * w, 36))
    cap = open(os.path.join(d, "caption.txt")).read().strip() if os.path.exists(os.path.join(d, "caption.txt")) else ""
    if os.path.exists(os.path.join(d, "words.txt")):  # asked in words (words.sh): the words, then what was made
        cap = f'asked "{open(os.path.join(d, "words.txt")).read().strip()}", made: {cap}'
        if os.path.exists(os.path.join(d, "fallback.txt")): cap += " (refused as decoded: the nearest training design)"
    rep = glob.glob(os.path.join(d, "*.drape.json")); r = __import__("json").load(open(rep[0])) if rep else {}
    flag = "" if r.get("ok", True) else "  [FAILED: " + ("slid" if not r.get("stayed", True) else "seams/inside") + "]"
    ImageDraw.Draw(row).text((8, 10), f"{os.path.basename(d.rstrip('/'))}: {cap}{flag}", fill="red" if flag else "black")
    rows.append(row)
W = max(r.width for r in rows); sheet = Image.new("RGB", (W, sum(r.height for r in rows)), "white"); y = 0
for r in rows: sheet.paste(r, (0, y)); y += r.height
sheet.thumbnail((2048, 100000)); sheet.save(out, quality=88); print(out, sheet.size)
