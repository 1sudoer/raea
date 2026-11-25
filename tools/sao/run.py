"""Stable Audio Open Small on Core ML (a Mac) or ONNX Runtime (Linux, Windows, raea's cloud GPUs): one sound for a
prompt and a seed, as a 44.1 kHz stereo WAV peak-normalised to full scale. rdb's asset store runs it for
make("sao", prompt, seed, {seconds}) and keeps the bytes under their hash (sdk/react/docs/audio-stack.md, "Generated
sources"); tools/sao/setup.py installs it beside the models.
    run.py --prompt TEXT --wav OUT.wav [--seed 1] [--seconds 4] [--steps 8] [--sampler pingpong|euler]
           [--backend coreml|onnx] [--dit-units cpu|gpu|all] [--device auto|cpu|cuda] [--times] [--dir DIR]

--backend is what the install has: Core ML on a Mac, else ONNX. Both run the same sampler from the same seed (one
numpy generator draws the start and every re-noise), so a backend changes only how each network is computed. ONNX is
the same weights as three fp32 graphs (atauser001/stable-audio-open-small-onnx: t5-base and the number embedder as
`cond`, the DiT as `dit`, the Oobleck decoder as `dec`), on CUDA when onnxruntime has it (--device), else the CPU;
`cond` always on the CPU, where it's small enough to be faster. --times prints each stage's seconds as JSON on stderr.

The pipeline is the Core ML packages' (mlboydaisuke/Stable-Audio-Open-Small-CoreML), driven the way its Swift sample
app drives them: T5 -> number embedder -> DiT x steps -> VAE decoder, at a fixed 256 latent frames (11.9 s), the output
trimmed to the asked length. T5 is raea's own fp32 conversion (t5_to_coreml.py), because the published fp16 one
overflows (NaN on the GPU, inf on the CPU). Compute units are as published: T5, the number embedder and the decoder on
CPU+GPU, the DiT on the CPU alone (fp16 overflows in it). The sampler is pingpong at 8 steps, what the model was
distilled for, on the exact schedule stable-audio-tools uses.

This is ~/.cache/raea-sao/coreml_run.py (the 2026-10-15 spike) cut to what inference needs: numpy, coremltools and
Hugging Face's `tokenizers` reading t5-base's tokenizer.json from the install, so a run needs neither torch nor
stable-audio-tools nor the network. Same prompt and seed, same samples as the spike's runner (checked 2026-10-19)."""
import argparse, json, os, shutil, sys, time, wave

SR, LAT, AUDIO = 44100, 256, 524288
TOKENS = 64
# the 8-step schedule stable-audio-tools uses with the pingpong sampler (captured; the same at 44 and 256 frames)
PINGPONG8 = [1.0, 0.994376, 0.98448, 0.957912, 0.890903, 0.745547, 0.512497, 0.273885, 0.0]


def schedule(steps, np):
    """Their logSNR schedule, for any other step count or the Euler sampler."""
    t = 1 / (1 + np.exp(-6.0 + np.arange(steps + 1) / steps * 8.0))
    t[0], t[-1] = 1.0, 0.0
    return t.astype(np.float32)


def tokenize(tok_path, prompt, np):
    """t5-base's fast tokenizer as transformers runs it: </s> appended, cut to 64 tokens, padded with id 0."""
    from tokenizers import Tokenizer
    tok = Tokenizer.from_file(tok_path)
    tok.enable_truncation(TOKENS)
    tok.enable_padding(length=TOKENS, pad_id=0, pad_token="<pad>")
    e = tok.encode(prompt)
    return np.array([e.ids], np.int32), np.array([e.attention_mask], np.int32)


def coreml(a, np):
    """The Core ML packages: T5 (raea's fp32), the number embedder and the decoder on CPU+GPU, the DiT on --dit-units."""
    import coremltools as ct

    units = {"cpu": ct.ComputeUnit.CPU_ONLY, "gpu": ct.ComputeUnit.CPU_AND_GPU, "all": ct.ComputeUnit.ALL}
    ref = f"{a.dir}/coreml-ref"

    # Each package is compiled once to a .mlmodelc beside it and loaded from there. MLModel(.mlpackage) compiles to a
    # new temp path every run, and Core ML caches a compiled bundle per path for good: 116 GB of them by 10-26.
    def compiled(pkg, u):
        out = pkg[: -len(".mlpackage")] + ".mlmodelc"
        if not os.path.isdir(out) or os.path.getmtime(out) < os.path.getmtime(pkg):
            tmp = ct.models.utils.compile_model(pkg)
            shutil.rmtree(out, ignore_errors=True)
            shutil.move(tmp, out)
        return ct.models.CompiledMLModel(out, compute_units=units[u])

    load = lambda n, u: compiled(f"{ref}/StableAudio{n}.mlpackage", u)
    t5 = compiled(f"{a.dir}/coreml/T5EncoderFP32.mlpackage", "gpu")
    num, dit, dec = load("NumberEmbedder", "gpu"), load("DiT", a.dit_units), load("VAEDecoder", "gpu")

    def cond(ids, mask, secs):
        emb = t5.predict({"input_ids": ids, "attention_mask": mask})["text_embeddings"].astype(np.float32)
        emb = emb * mask[..., None]  # padding zeroed, as the PyTorch conditioner does
        sec = num.predict({"normalized_seconds": np.array([min(secs, 256) / 256], np.float32)})["seconds_embedding"].astype(np.float32)
        return np.concatenate([emb, sec[:, None, :]], axis=1), sec

    def step(x, t, cross, sec):
        return dit.predict({"latent": x, "timestep": np.array([t], np.float32), "cross_attn_cond": cross, "global_embed": sec})["velocity"].astype(np.float32)

    def decode(x):
        return dec.predict({"latent": x})["audio"][0].astype(np.float32)

    return cond, step, decode, {"dit_units": a.dit_units}


def onnx(a, np):
    """The ONNX graphs on ONNX Runtime: the DiT and the decoder on CUDA when there is one (--device), cond on the CPU."""
    import onnxruntime as ort

    have = ort.get_available_providers()
    device = a.device if a.device != "auto" else ("cuda" if "CUDAExecutionProvider" in have else "cpu")
    if device == "cuda":
        if "CUDAExecutionProvider" not in have:
            raise SystemExit(f"sao: this onnxruntime has no CUDA ({', '.join(have)}): install SAO again with RAEA_SAO_CUDA=1 (raea setup-audio sao) on a machine with an NVIDIA GPU")
        # the CUDA and cuDNN libraries from their wheels (onnxruntime-gpu[cuda,cudnn]), not the system's
        if hasattr(ort, "preload_dlls"):
            ort.preload_dlls()
    cpu = ["CPUExecutionProvider"]
    main = ["CUDAExecutionProvider", "CPUExecutionProvider"] if device == "cuda" else cpu
    o = ort.SessionOptions()
    o.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    o.log_severity_level = 3
    graph = lambda n, p: ort.InferenceSession(f"{a.dir}/onnx/{n}.onnx", o, providers=p)
    cnd, dit, dec = graph("cond", cpu), graph("dit", main), graph("dec", main)
    if device == "cuda" and "CUDAExecutionProvider" not in dit.get_providers():
        raise SystemExit("sao: onnxruntime couldn't start CUDA for the DiT (its CUDA or cuDNN libraries are missing)")

    def cond(ids, mask, secs):
        cross, glob = cnd.run(["cross", "global"], {"input_ids": ids.astype(np.int64), "attention_mask": mask.astype(np.int64),
                                                    "seconds_total": np.array([secs], np.float32)})
        return cross.astype(np.float32), glob.astype(np.float32)

    def step(x, t, cross, sec):
        return dit.run(["v"], {"x": x, "t": np.array([t], np.float32), "cross": cross, "global": sec})[0].astype(np.float32)

    def decode(x):
        return dec.run(["audio"], {"z": x})[0][0].astype(np.float32)

    return cond, step, decode, {"device": device, "onnxruntime": ort.__version__}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--prompt", required=True)
    ap.add_argument("--wav", required=True)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--seconds", type=float, default=4.0)
    ap.add_argument("--steps", type=int, default=8)
    ap.add_argument("--sampler", default="pingpong", choices=["pingpong", "euler"])
    ap.add_argument("--backend", choices=["coreml", "onnx"])
    ap.add_argument("--dit-units", default="cpu", choices=["cpu", "gpu", "all"])
    ap.add_argument("--device", default="auto", choices=["auto", "cpu", "cuda"])
    ap.add_argument("--times", action="store_true")
    ap.add_argument("--dir", default=os.path.dirname(os.path.abspath(__file__)))
    a = ap.parse_args()
    backend = a.backend or ("coreml" if sys.platform == "darwin" and os.path.isdir(f"{a.dir}/coreml-ref") else "onnx")

    import numpy as np

    t0 = time.time()
    cond, step, decode, info = (coreml if backend == "coreml" else onnx)(a, np)
    times = {"backend": backend, **info, "load_s": time.time() - t0}

    secs = min(a.seconds, AUDIO / SR)
    t = time.time()
    ids, mask = tokenize(f"{a.dir}/t5-tokenizer/tokenizer.json", a.prompt, np)
    cross, sec = cond(ids, mask, secs)
    times["cond_s"] = time.time() - t

    t = time.time()
    rng = np.random.default_rng(a.seed)
    x = rng.standard_normal((1, 64, LAT)).astype(np.float32)
    ts = np.array(PINGPONG8, np.float32) if a.sampler == "pingpong" and a.steps == 8 else schedule(a.steps, np)
    for s in range(a.steps):
        v = step(x, ts[s], cross, sec)
        if a.sampler == "pingpong":  # predict the clean latent, then re-noise to the next level
            den = x - ts[s] * v
            x = (1 - ts[s + 1]) * den + ts[s + 1] * rng.standard_normal(x.shape).astype(np.float32)
        else:
            x = x + (ts[s + 1] - ts[s]) * v
    times["dit_s"] = time.time() - t
    t = time.time()
    audio = decode(x)[:, : int(secs * SR)]
    times["dec_s"] = time.time() - t
    audio = np.nan_to_num(audio)
    audio = audio / max(np.abs(audio).max(), 1e-6)
    pcm = (audio.T * 32767).astype("<i2").tobytes()
    # 16-bit stereo PCM, written by Python's own wave module (until 0.1.8 an installed ffmpeg wrote it, whose header
    # also named the encoder: the samples are the same)
    with wave.open(a.wav, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm)
    if a.times:
        print(json.dumps({k: round(v, 3) if isinstance(v, float) else v for k, v in times.items()}), file=sys.stderr)


if __name__ == "__main__":
    sys.exit(main())
