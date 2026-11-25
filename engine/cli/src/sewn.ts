// Sewn garments for `raea clothes` (docs/character.feature CH45c): a garment is a sewing pattern (GarmentCode's
// components, MIT) cut to SOMA's measurements, draped on the body by raea's own cloth on the GPU and baked to a
// skinned glb, by tools/garments/sew.sh. Its design comes from words (the garment model, when raea's models folder
// has one) or from the plain base, with any setting changed by path. A project's sewn garment is
// assets/clothes/NAME.json (kind "sewn") with its files in assets/clothes/NAME/.
//
// What it runs on, in raea's models folder under garments/ (or named by GARMENTCODE, GARMENT_PY, GARMENT_MODEL):
// GarmentCode at a fixed commit (cloned the first time and checked by that commit), a Python 3.11 environment made
// by uv from a pinned list, the body as a distance grid (made once from SOMA's body), and the words model.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";

type Env = Record<string, string | undefined>;

const GARMENTCODE = "d449629979028123a5c4dc9e732a2ec19b7fce31";
/** the environment's packages, as the first one was made (uv pip freeze), less what they bring with them */
const PACKAGES = ["numpy==1.26.4", "scipy==1.17.1", "pyyaml==6.0.3", "svgwrite==1.4.3", "svgpathtools==1.8.0", "cairosvg==2.9.1", "matplotlib==3.11.2", "cgal==6.0.1.post202410241521", "trimesh==5.2.1", "psutil==7.2.2", "networkx==3.6.1", "shapely==2.2.0", "rtree==1.4.1", "libigl==2.6.3", "pillow==13.0.0", "mlx==0.32.4", "onnxruntime==1.32.0", "tokenizers==0.23.3"];

export type Sewn = { name: string; kind: "sewn"; layer: number; words: string; made: string; colour: string; set: string[]; plain: boolean; glb: string; vertices: number; ok: boolean };

const run = (cmd: string, args: string[], env?: Env) => spawnSync(cmd, args, { encoding: "utf8", maxBuffer: 64 << 20, env: env as NodeJS.ProcessEnv });
const tail = (r: { stdout?: string; stderr?: string }) => `${r.stdout ?? ""}${r.stderr ?? ""}`.trim().split("\n").slice(-6).join("\n");

/** tools/garments: in RAEA_HOME, beside a release's bin/raea (share/raea/tools), or in the tree a built raea is in */
function tools(raea: string, env: Env): string {
  const bin = dirname(realpathSync(raea));
  const found = [env.RAEA_HOME && join(env.RAEA_HOME, "tools/garments"), join(bin, "../share/raea/tools/garments"), join(bin, "../../tools/garments")].find((p) => p && existsSync(join(p, "sew.sh")));
  if (!found) throw new Error(`no tools/garments/sew.sh beside ${bin} (a release keeps it in share/raea/tools/garments)`);
  return found;
}

/** What sewing runs on, set up the first time; `made` says what was fetched or built */
function setup(raea: string, env: Env, models: string, body: string): { env: Env; model: string | undefined; made: string[] } {
  const dir = join(models, "garments"), made: string[] = [], t = tools(raea, env);
  const gc = env.GARMENTCODE ?? join(dir, "GarmentCode"), py = env.GARMENT_PY ?? join(dir, "venv/bin/python");
  if (!existsSync(join(gc, "assets/design_params/t-shirt.yaml"))) {
    mkdirSync(dir, { recursive: true });
    rmSync(gc, { recursive: true, force: true });
    const c = run("git", ["clone", "--quiet", "https://github.com/maria-korosteleva/GarmentCode", gc]);
    if (c.status !== 0) throw new Error(`couldn't fetch GarmentCode (the sewing patterns): ${tail(c)}`);
    const o = run("git", ["-C", gc, "checkout", "--quiet", GARMENTCODE]), at = run("git", ["-C", gc, "rev-parse", "HEAD"]).stdout?.trim();
    if (o.status !== 0 || at !== GARMENTCODE) {
      rmSync(gc, { recursive: true, force: true });
      throw new Error(`GarmentCode has no commit ${GARMENTCODE}; nothing was kept`);
    }
    made.push(`fetched GarmentCode (MIT) at ${GARMENTCODE.slice(0, 7)}: ${gc}`);
  }
  if (!existsSync(py)) {
    const venv = dirname(dirname(py));
    const v = run("uv", ["venv", "--quiet", "--python", "3.11", venv]);
    if (v.error) throw new Error("sewing needs uv to make its Python environment (https://docs.astral.sh/uv: brew install uv), or GARMENT_PY naming one that has tools/garments/README.md's packages");
    if (v.status !== 0) throw new Error(`couldn't make the sewing environment: ${tail(v)}`);
    const p = run("uv", ["pip", "install", "--quiet", "--python", py, ...PACKAGES]);
    if (p.status !== 0) {
      rmSync(venv, { recursive: true, force: true });
      throw new Error(`couldn't install the sewing environment's packages: ${tail(p)}`);
    }
    made.push(`made the sewing environment (Python 3.11, ${PACKAGES.length} packages): ${venv}`);
  }
  const sdf = join(dir, "soma_sdf.npz");
  if (!existsSync(sdf)) {
    mkdirSync(dir, { recursive: true });
    const r = run(py, ["-I", join(t, "sdf_igl.py"), join(body, "body.glb"), sdf]);
    if (r.status !== 0 || !existsSync(sdf)) throw new Error(`couldn't measure SOMA's body for the drape: ${tail(r)}`);
    made.push(`measured SOMA's body as a distance grid for the drape: ${sdf}`);
  }
  const model = env.GARMENT_MODEL ?? join(dir, "garment-words.npz");
  return { env: { ...env, GARMENTCODE: gc, GARMENT_PY: py, GARMENT_SDF: sdf, SOMA_BODY: join(body, "body.glb"), RAEA_BIN: raea, PATH: `${dirname(realpathSync(raea))}:${env.PATH ?? ""}` }, model: existsSync(model) ? model : undefined, made };
}

/** every setting of a design: its path, its value in the plain base, what it may be */
export function settings(raea: string, env: Env, models: string, body: string): { said: string } {
  const s = setup(raea, env, models, body), r = run(s.env.GARMENT_PY!, ["-I", join(tools(raea, env), "design.py"), join(s.env.GARMENTCODE!, "assets/design_params/t-shirt.yaml")]);
  if (r.status !== 0) throw new Error(tail(r));
  return { said: [...s.made, "A sewn garment's settings, as the plain base has them (raea clothes make NAME --plain --set PATH=VALUE,...):", r.stdout.trim()].join("\n") + "\n" };
}

/** One sewn garment made in the project: assets/clothes/NAME.json and assets/clothes/NAME/ */
export function sew(raea: string, cwd: string, env: Env, models: string, body: string, a: { name: string; words?: string; set?: string[]; plain?: boolean; colour?: string; layer?: number }): { said: string } {
  const s = setup(raea, env, models, body), clothes = join(cwd, "assets/clothes"), own = join(clothes, a.name), file = join(clothes, `${a.name}.json`);
  const plain = a.plain || !a.words;
  if (!plain && !s.model) throw new Error(`no words model here yet (${join(models, "garments/garment-words.npz")}): make it from the plain base, raea clothes make ${a.name} --plain --set PATH=VALUE,... (raea clothes settings lists the settings)`);
  mkdirSync(clothes, { recursive: true });
  const args = [clothes, a.name, a.words ?? "", ...(plain ? ["--plain"] : []), ...(a.set ?? []).flatMap((x) => ["--set", x]), ...(a.colour ? ["--colour", a.colour] : [])];
  const r = run("bash", [join(tools(raea, env), "sew.sh"), ...args], { ...s.env, ...(s.model ? { GARMENT_MODEL: s.model } : {}) });
  const lines = `${r.stdout ?? ""}`.split("\n").filter((l) => l.startsWith("sew: ")).map((l) => l.slice(5));
  if (r.status !== 0 || !existsSync(join(own, `${a.name}.glb`))) {
    const why = `${r.stderr ?? ""}`.split("\n").filter((l) => l.startsWith("sew: FAILED") || l.startsWith("design: ")).map((l) => l.replace(/^(sew: FAILED |design: )/, "")).join("; ");
    if (!existsSync(file)) rmSync(own, { recursive: true, force: true }); // (nothing half-made left beside the project's garments)
    throw new Error(`${a.name} wasn't sewn: ${why || tail(r)}`);
  }
  const bake = JSON.parse(readFileSync(join(own, `${a.name}.bake.json`), "utf8")) as { colour: string; vertices: number };
  const drape = JSON.parse(readFileSync(join(own, `${a.name}.drape.json`), "utf8")) as { ok: boolean };
  const made = existsSync(join(own, "caption.txt")) ? readFileSync(join(own, "caption.txt"), "utf8").trim() : a.name;
  // (the drape's working files are not the garment: its pattern, its design and its pictures are kept)
  for (const f of [`${a.name}.draped.npz`, `${a.name}.panels.json`, "make.log", "nearest.yaml"]) rmSync(join(own, f), { force: true });
  const g: Sewn = { name: a.name, kind: "sewn", layer: a.layer ?? 0, words: a.words ?? "", made, colour: bake.colour, set: a.set ?? [], plain, glb: `${a.name}/${a.name}.glb`, vertices: bake.vertices, ok: drape.ok };
  writeFileSync(file, `${JSON.stringify(g, null, 1)}\n`);
  return {
    said: [...s.made, ...lines, `${relative(cwd, file)}, its pictures in ${relative(cwd, own)} (${a.name}_front.png ...): look at them, and change it with --set (raea clothes settings).`, `raea clothes wear assets/characters/NAME ${a.name} puts it on SOMA's body.`].join("\n") + "\n",
  };
}

/** Sewn garments put on a body folder already written at `out`: each one's glb beside the body, and the body's skin
 * wholly under them left out (tools/garments/hide_under.py); the lines said */
export function wearSewn(raea: string, cwd: string, env: Env, out: string, files: string[]): string[] {
  const glbs = files.map((f) => {
    const g = JSON.parse(readFileSync(f, "utf8")) as Sewn, to = join(out, `${g.name}.glb`);
    copyFileSync(join(dirname(f), g.glb), to);
    return to;
  });
  const body = join(out, "body.glb"), r = run(raea, ["blender", "run", join(tools(raea, env), "hide_under.py"), body, body, ...glbs]);
  const said = `${r.stdout ?? ""}${r.stderr ?? ""}`.split("\n").filter((l) => l.startsWith("hide_under: ")).map((l) => `the body under ${files.length > 1 ? "the sewn garments" : "the sewn garment"}: ${l.slice(12)}`);
  if (r.status !== 0 || !said.length) throw new Error(`couldn't leave out the skin under the sewn garments: ${tail(r)}`);
  return [...glbs.map((g) => `wore ${relative(cwd, g)}`), ...said];
}
