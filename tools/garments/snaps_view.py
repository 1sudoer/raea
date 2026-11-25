# the drape's snapshots (drape_mlx.py with SNAP=1) over the body's outline, front and side, one column a snapshot
#   python snaps_view.py SDF.npz SNAPS.npz OUT.png
import sys
import numpy as np
import matplotlib; matplotlib.use("Agg")
import matplotlib.pyplot as plt
S = np.load(sys.argv[1]); Z = np.load(sys.argv[2]); out = sys.argv[3]
sdf, o, c = S["sdf"], S["origin"], float(S["cell"])
front = (sdf < 0).any(axis=2).T  # (y, x)
side = (sdf < 0).any(axis=0)  # (y, z)
keys = sorted(k for k in Z.files if k.startswith("f")); start = list(Z["start"])
fig, ax = plt.subplots(2, len(keys), figsize=(3 * len(keys), 8))
for col, k in enumerate(keys):
    X = Z[k]; piece = np.searchsorted(start, np.arange(len(X)), side="right") - 1
    for row, (mask, h, ext) in enumerate([(front, 0, (o[0], o[0] + c * sdf.shape[0])), (side, 2, (o[2], o[2] + c * sdf.shape[2]))]):
        a = ax[row, col]
        a.imshow(mask, origin="lower", extent=(*ext, o[1], o[1] + c * sdf.shape[1]), cmap="Greys", alpha=0.35)
        a.scatter(X[:, h], X[:, 1], s=0.2, c=piece, cmap="tab10", vmin=0, vmax=9)
        a.set_xlim(*(ext if row else (-0.6, 0.6))); a.set_ylim(0.6, 1.75); a.set_aspect("equal"); a.set_xticks([]); a.set_title(k if row == 0 else "")
plt.tight_layout(); plt.savefig(out, dpi=90); print(out)
