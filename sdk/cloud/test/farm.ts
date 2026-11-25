// raea's render farm, live (docs/cloud-render.feature CR9): a job sent the way a CPU box sends one, timed, and its
// video checked. RAEA_CLOUD_GPU_URL and RAEA_CLOUD_GPU_TOKEN name the farm (~/.config/raea-cloud/farm.env):
//
//   node test/farm.ts raea [GPUS]                     test/gpu/edit.tsx to out/edit.mp4
//   node test/farm.ts blender DIR FILE.blend FRAMES [GPUS] [SAMPLES]
//                                                     a Blender scene to out/render.mp4, frames A-B
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"
import { RaeaCloud } from "../src/index.ts"

const [kind = "raea", ...rest] = process.argv.slice(2)
const url = process.env.RAEA_CLOUD_GPU_URL
const token = process.env.RAEA_CLOUD_GPU_TOKEN
if (!url || !token) throw new Error("set RAEA_CLOUD_GPU_URL and RAEA_CLOUD_GPU_TOKEN (the farm's)")

const here = path.dirname(new URL(import.meta.url).pathname)
const [dir, args, gpus] =
  kind === "blender"
    ? [rest[0], ["blender", rest[1], "-o", "out/render.mp4", "--frames", rest[2], ...(rest[4] ? ["--samples", rest[4]] : [])], rest[3]]
    : [path.join(here, "gpu"), ["render", "edit.tsx", "-o", "out/edit.mp4"], rest[0]]

const tmp = mkdtempSync(path.join(os.tmpdir(), "raea-farm-test-"))
try {
  // a Mac's tar: gzip, and no AppleDouble files
  const sent = path.join(tmp, "in.tgz")
  const pack = spawnSync("tar", ["-czf", sent, "--no-mac-metadata", "-C", dir, "."], { env: { ...process.env, COPYFILE_DISABLE: "1" }, encoding: "utf8" })
  if (pack.status) throw new Error(`the folder didn't pack: ${pack.stderr}`)
  const folder = new Uint8Array(readFileSync(sent))
  const cloud = new RaeaCloud({ url, token })
  const t0 = performance.now()
  const at = () => `${((performance.now() - t0) / 1000).toFixed(1).padStart(6)} s`
  console.log(`${at()}  ${(folder.length / 1e6).toFixed(1)} MB to ${new URL(url).host}: raea ${args.join(" ")}`)
  const { exit, files } = await cloud.job(args, folder, {
    as: "gz",
    farm: gpus ? { gpus: Number(gpus) } : {},
    onOutput: (t) => t.split("\n").filter(Boolean).forEach((l) => console.log(`${at()}  ${l}`)),
  })
  const back = path.join(tmp, "out.tgz")
  writeFileSync(back, files)
  spawnSync("tar", ["-xzf", back, "-C", tmp])
  const video = path.join(tmp, args[args.indexOf("-o") + 1])
  const probe = spawnSync("ffprobe", ["-v", "error", "-count_frames", "-select_streams", "v", "-show_entries", "stream=nb_read_frames,width,height", "-of", "csv=p=0", video], { encoding: "utf8" })
  console.log(`${at()}  exit ${exit}; ${path.basename(video)}: ${probe.stdout.trim() || probe.stderr.trim()} (width,height,frames)`)
  const keep = process.env.RAEA_FARM_KEEP
  if (keep) spawnSync("cp", [video, keep])
  process.exitCode = exit
} finally {
  rmSync(tmp, { recursive: true, force: true })
}
