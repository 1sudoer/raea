# a compact grid: per garment its front and three-quarter views, cropped to the body, captioned
#   python grid.py OUT.jpg COLS DIR...
import sys, glob, os, json, textwrap
from PIL import Image, ImageDraw
out, cols, dirs = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
cells = []
for d in dirs:
    v = [glob.glob(os.path.join(d, f"*_{k}.png")) for k in ("front", "threequarter")]
    if not all(v): continue
    ims = [Image.open(p[0]).convert("RGB").crop((96, 40, 416, 728)).resize((200, 430)) for p in v]
    cell = Image.new("RGB", (400, 470), "white"); [cell.paste(im, (i * 200, 40)) for i, im in enumerate(ims)]
    cap = open(os.path.join(d, "caption.txt")).read().strip()
    r = json.load(open(glob.glob(os.path.join(d, "*.drape.json"))[0]))
    t = f"{os.path.basename(d.rstrip('/'))}: {cap}" + ("" if r["ok"] else " [FAIL]")
    ImageDraw.Draw(cell).multiline_text((4, 2), "\n".join(textwrap.wrap(t, 64)[:3]), fill="black" if r["ok"] else "red")
    cells.append(cell)
rows = (len(cells) + cols - 1) // cols
sheet = Image.new("RGB", (cols * 400, rows * 470), "white")
for i, c in enumerate(cells): sheet.paste(c, ((i % cols) * 400, (i // cols) * 470))
sheet.save(out, quality=86); print(out, sheet.size)
