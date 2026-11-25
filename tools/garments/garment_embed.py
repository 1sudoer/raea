# SigLIP 2 (shipped in raea's models, siglip2-base-p16-256) for the garment model (docs/character.feature CH48c): each
# design's pictures on the mannequin (bake_batch.py PICTURES=1) as image embeddings, and a line of words as a text
# embedding in the same space. ONNX Runtime on the CPU: Core ML's GPU computes SigLIP 2 wrong on this Mac. The text is
# lower-cased and padded to 64 tokens as raea's own index does (systems/index/src/text.rs).
#   python -I garment_embed.py OUT_DIR        embeds every OUT_DIR/g*/<name>_<view>.jpg not yet in OUT_DIR/embeds.npz
#   python -I garment_embed.py --text "a long pencil skirt" ...    prints each line's first values (a check)
import glob, os, sys
import numpy as np

M = os.environ.get("SIGLIP_DIR", os.path.expanduser("~/.local/share/raea-models/siglip2-base-p16-256"))
_s = {}


def _session(name):
    if name not in _s:
        import onnxruntime as ort
        o = ort.SessionOptions(); o.intra_op_num_threads = int(os.environ.get("SIGLIP_THREADS", "4"))
        _s[name] = ort.InferenceSession(f"{M}/onnx/{name}.onnx", o, providers=["CPUExecutionProvider"])
    return _s[name]


def _unit(x):
    return x / np.maximum(np.linalg.norm(x, axis=-1, keepdims=True), 1e-9)


def image_embed(paths, batch=16):
    """unit image embeddings (N, 768): each picture padded square with its own corner colour, then 256 px"""
    from PIL import Image
    out = []
    for i in range(0, len(paths), batch):
        px = []
        for p in paths[i:i + batch]:
            im = Image.open(p).convert("RGB"); w, h = im.size; s = max(w, h)
            sq = Image.new("RGB", (s, s), im.getpixel((0, 0))); sq.paste(im, ((s - w) // 2, (s - h) // 2))
            a = np.asarray(sq.resize((256, 256), Image.BILINEAR), np.float32) / 255.0
            px.append(((a - 0.5) / 0.5).transpose(2, 0, 1))
        out.append(_session("vision_model").run(["pooler_output"], {"pixel_values": np.stack(px)})[0])
    return _unit(np.concatenate(out).astype(np.float32))


def text_embed(lines):
    """unit text embeddings (N, 768)"""
    from tokenizers import Tokenizer
    tok = Tokenizer.from_file(f"{M}/tokenizer.json"); pad = tok.token_to_id("<pad>") or 0
    ids = np.full((len(lines), 64), pad, np.int64)
    for i, s in enumerate(lines):
        e = tok.encode(s.lower(), add_special_tokens=True).ids[:64]; ids[i, :len(e)] = e
    return _unit(np.concatenate([_session("text_model").run(["pooler_output"], {"input_ids": ids[i:i + 32]})[0] for i in range(0, len(lines), 32)]).astype(np.float32))


if __name__ == "__main__":
    if sys.argv[1] == "--text":
        for s, e in zip(sys.argv[2:], text_embed(sys.argv[2:])): print(f"{s!r}: {np.round(e[:4], 4)}")
        sys.exit()
    out = sys.argv[1]; path = f"{out}/embeds.npz"
    have = dict(np.load(path)) if os.path.exists(path) else {"names": np.array([], str), "views": np.array([], str), "E": np.zeros((0, 768), np.float32)}
    done = set(zip(have["names"].tolist(), have["views"].tolist()))
    todo = []
    for p in sorted(glob.glob(f"{out}/g*/*_*.jpg")):
        n, v = os.path.basename(p)[:-4].rsplit("_", 1)
        if (n, v) not in done and v != "pattern": todo.append((n, v, p))
    if todo:
        E = image_embed([p for _, _, p in todo])
        have = {"names": np.r_[have["names"], [n for n, _, _ in todo]], "views": np.r_[have["views"], [v for _, v, _ in todo]], "E": np.r_[have["E"], E]}
        np.savez(path, **have)
    print(f"embed: {len(todo)} new pictures, {len(have['names'])} in {path}", flush=True)
