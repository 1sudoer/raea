// The host's side of `raea clothes` (clothes.ts) on the native raea. SOMA-X's two body files are fetched once at a
// fixed commit (NVlabs/SOMA-X, Apache-2.0, checked by their SHA-256 and kept with the licence in raea's models
// folder, soma/) and SOMA's body is made from them once by Blender (tools/clothes/body.py). A garment is sewn
// (sewn.ts: a pattern cut to SOMA, draped and baked) or made by procedure on the skin (tools/clothes/garment.py
// cover); both are worn by `wear`. A project's garments are assets/clothes/NAME.json, their files in
// assets/clothes/NAME/. (Garments measured from four drawn pictures were taken out on 11-15: paint on a
// skin-tight shell showed no cut, hem or fold, and four pictures never agreed.)
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative } from "node:path";
import { gradeToHead } from "./grade";
import { settings, sew, wearSewn } from "./sewn";

type Env = Record<string, string | undefined>;
type Said = { said: string };

const COMMIT = "01ab5b5c0876058bd27d34b1607d74b2e55e0cf4";
const SOMA_X: Record<string, string> = {
  "SOMA_template_rig.usda": "a952cb7bdf4ccd801e355d53122d395b985b69d0d9181439edad5081bb171856",
  "SOMA_neutral.npz": "5829af4a5333abdba5ad449360b868faacee99d395971e62f35c0e7169432b63",
};
/** raea's models folder (raea_io_home::models): ~/.local/share/raea-models, %LOCALAPPDATA%\raea\models on Windows */
const modelsDir = (env: Env) => (process.platform === "win32" && env.LOCALAPPDATA ? join(env.LOCALAPPDATA, "raea", "models") : join(env.HOME ?? "", ".local/share/raea-models"));

/** A script of tools/clothes: in RAEA_HOME, beside a release's bin/raea (share/raea/tools), or in the tree a built raea is in */
function tool(raea: string, env: Env, file: string): string {
  const bin = dirname(realpathSync(raea));
  const found = [env.RAEA_HOME && join(env.RAEA_HOME, "tools/clothes", file), join(bin, "../share/raea/tools/clothes", file), join(bin, "../../tools/clothes", file)].find((p) => p && existsSync(p));
  if (!found) throw new Error(`no tools/clothes/${file} beside ${bin} (a release keeps it in share/raea/tools/clothes)`);
  return found;
}

/** A tools/clothes script in a windowless Blender (`raea blender run`); the lines it says for raea ("clothes: ...") */
function blender(raea: string, env: Env, file: string, args: string[]): string[] {
  const r = spawnSync(raea, ["blender", "run", tool(raea, env, file), ...args], { encoding: "utf8", maxBuffer: 256 << 20 });
  const all = `${r.stdout ?? ""}${r.stderr ?? ""}`;
  if (r.status !== 0) throw new Error(`tools/clothes/${file} failed in Blender:\n${all.trim().split("\n").slice(-12).join("\n")}`);
  return all.split("\n").filter((l) => l.startsWith("clothes: ")).map((l) => l.slice(9));
}

/** SOMA-X's body files and licence in raea's models folder, fetched the first time and checked by their SHA-256 */
function somaX(dir: string): { rig: string; shapes: string } {
  mkdirSync(dir, { recursive: true });
  const get = (url: string, to: string) => {
    const r = spawnSync("curl", ["-fsSL", "--retry", "2", "-o", to, url], { encoding: "utf8" });
    if (r.status !== 0) throw new Error(`couldn't fetch SOMA's body from ${url}: ${(r.stderr ?? "").trim()}`);
  };
  for (const [file, sha] of Object.entries(SOMA_X)) {
    const to = join(dir, file);
    if (existsSync(to)) continue;
    get(`https://media.githubusercontent.com/media/NVlabs/SOMA-X/${COMMIT}/assets/${file}`, `${to}.part`);
    const got = createHash("sha256").update(readFileSync(`${to}.part`)).digest("hex");
    if (got !== sha) {
      rmSync(`${to}.part`, { force: true });
      throw new Error(`SOMA-X's ${file} came back changed (SHA-256 ${got}, not ${sha}); nothing was kept`);
    }
    renameSync(`${to}.part`, to);
  }
  if (!existsSync(join(dir, "LICENSE"))) get(`https://raw.githubusercontent.com/NVlabs/SOMA-X/${COMMIT}/LICENSE`, join(dir, "LICENSE"));
  return { rig: join(dir, "SOMA_template_rig.usda"), shapes: join(dir, "SOMA_neutral.npz") };
}

/** SOMA's body (soma/body: body.glb, skeleton.json), made once */
function soma(raea: string, env: Env): { rig: string; body: string; made: string[] } {
  const dir = join(modelsDir(env), "soma"), x = somaX(dir), made: string[] = [];
  const body = join(dir, "body");
  if (!["body.glb", "skeleton.json", "joint.glb"].every((f) => existsSync(join(body, f)))) made.push(...blender(raea, env, "body.py", [x.rig, x.shapes, body]));
  return { rig: x.rig, body, made };
}

type Garment = { name: string; layer: number; vertices: number[]; stand: number[]; over?: string; topology?: string; kind?: string; made?: string };
const readGarment = (file: string) => JSON.parse(readFileSync(file, "utf8")) as Garment;

function list(cwd: string) {
  const dir = join(cwd, "assets/clothes");
  const files = existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith(".json")).sort() : [];
  const garments = files.map((f) => readGarment(join(dir, f))).filter((g) => g.kind === "sewn" || g.topology?.startsWith("SOMA"));
  return { garments: garments.map((g) => (g.kind === "sewn" ? { name: g.name, layer: g.layer, sewn: g.made ?? "" } : { name: g.name, layer: g.layer, vertices: g.vertices.length, over: g.over })) };
}

function cover(raea: string, cwd: string, env: Env, a: { name: string; parts: string; colour: string; stand: number; over?: string; hard?: { sole?: number; soleColour?: string; panels?: string[] } }): Said {
  const clothes = join(cwd, "assets/clothes"), file = join(clothes, `${a.name}.json`), under = a.over ? join(clothes, `${a.over}.json`) : undefined;
  if (under && !existsSync(under)) throw new Error(`no garment ${a.over} (assets/clothes/${a.over}.json): raea clothes lists the ones made`);
  if (a.over === a.name) throw new Error("a garment can't be laid over itself");
  const s = soma(raea, env);
  mkdirSync(clothes, { recursive: true });
  const hard = a.hard ? ["--hard", ...(a.hard.sole !== undefined ? ["--sole", String(a.hard.sole)] : []), ...(a.hard.soleColour ? ["--sole-colour", a.hard.soleColour] : []), ...(a.hard.panels?.length ? ["--panels", a.hard.panels.join(",")] : [])] : [];
  const said = blender(raea, env, "garment.py", ["cover", join(s.body, "body.glb"), s.rig, file, a.parts, a.colour, String(a.stand), ...(under ? [under] : []), ...hard]);
  if (!existsSync(file)) throw new Error("no garment was written");
  return { said: [...s.made, ...said, `${relative(cwd, file)}. raea clothes wear assets/characters/NAME ${a.over ? `${a.over} ` : ""}${a.name} puts it on SOMA's body.`].join("\n") + "\n" };
}

function wear(raea: string, cwd: string, env: Env, a: { out: string; garments: string[]; body?: string; head?: string }): Said {
  const at = (p: string) => (isAbsolute(p) ? p : join(cwd, p));
  const files = a.garments.map((g) => (g.endsWith(".json") ? at(g) : join(cwd, "assets/clothes", `${g}.json`)));
  for (const f of files) if (!existsSync(f)) throw new Error(`no garment ${relative(cwd, f)}: raea clothes lists the ones made`);
  // sewn garments are their own meshes; the ones made on the skin are laid on it innermost first
  const sewn = files.filter((f) => readGarment(f).kind === "sewn"), onSkin = files.filter((f) => !sewn.includes(f));
  const layers = onSkin.map((f) => readGarment(f).layer);
  if (layers.some((l, k) => k > 0 && l < layers[k - 1])) throw new Error(`put the garments innermost first (their layers are ${layers.join(", ")})`);
  const s = soma(raea, env), body = a.body ? at(a.body) : s.body, out = at(a.out);
  if (!existsSync(join(body, "body.glb"))) throw new Error(`${a.body}: not a body folder (no body.glb)`);
  mkdirSync(out, { recursive: true });
  const said = blender(raea, env, "garment.py", ["wear", join(body, "body.glb"), s.rig, out, ...onSkin]);
  if (sewn.length) said.push(...wearSewn(raea, cwd, env, out, sewn));
  // the dressed body is a body folder of its own: its skeleton, its joints' stand-in and a headless body's collar
  for (const f of ["skeleton.json", "joint.glb", "collar.json"]) if (existsSync(join(body, f))) copyFileSync(join(body, f), join(out, f));
  // a head that joins the body at the collar: the body's skin graded to the head's (grade.ts)
  if (a.head) {
    const json = join(at(a.head), "head.json");
    if (!existsSync(json)) throw new Error(`${a.head}: not a head folder (no head.json)`);
    said.push(gradeToHead(join(out, "body.glb"), json));
  }
  const served = relative(join(cwd, "assets"), out);
  const use = served.startsWith("..")
    ? ""
    : `\nIn a composition: registerBody(skeleton, staticFile("${served}")), then <Character body=... parts={[${files.map((f) => `{ src: staticFile("${served}/${basename(f, ".json")}.glb") }`).join(", ")}]} />`;
  return { said: [...s.made, ...said].join("\n") + use + "\n" };
}

/** host op "clothes": list, settings, make, cover or wear */
export function clothesHost(raea: string, cwd: string, env: Env, a: Record<string, unknown>): unknown {
  switch (a.op) {
    case "list":
      return list(cwd);
    case "settings": {
      const s = soma(raea, env), r = settings(raea, env, modelsDir(env), s.body);
      return { said: [...s.made, r.said].join("\n") };
    }
    case "make": {
      const s = soma(raea, env), r = sew(raea, cwd, env, modelsDir(env), s.body, a as Parameters<typeof sew>[5]);
      return { said: [...s.made, r.said].join("\n") };
    }
    case "cover":
      return cover(raea, cwd, env, a as Parameters<typeof cover>[3]);
    case "wear":
      return wear(raea, cwd, env, a as Parameters<typeof wear>[3]);
    default:
      throw new Error(`raea clothes: no step ${String(a.op)}`);
  }
}
