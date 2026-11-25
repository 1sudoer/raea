// A farm job's media by hash, both sides (docs/cloud-render.feature CR23): server/media.ts on a box's clone of a raea
// repo, deploy/modal/farm_media.py standing in for the farm in a folder of its own. raea-api's test/repos.sh runs it
// against a local raea-api, its LFS on a stand-in R2:
//   node test/media.ts CLONE      with RAEA_AGENT_KEY (or RAEA_TOKEN) signed in for the clone's project
// Checked: every committed media file goes by hash (none in a tar), the farm's copy of each is its hash, the farm's
// new render goes to R2 by a signed PUT and is verified, and the clone gets a pointer for it, not its bytes. Prints
// one JSON line: {inputs, fetched, render: {path, oid, size}} for repos.sh to push and pull.

import { spawnSync } from "node:child_process"
import { createHash, randomBytes } from "node:crypto"
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { excluded, inputs, lfsOf, outputs } from "../server/media.ts"

const clone = process.argv[2]
if (!clone) throw new Error("node test/media.ts CLONE")
const modal = path.join(import.meta.dirname, "..", "deploy", "modal")
const fail = (m: string): never => {
  console.error(`FAIL ${m}`)
  process.exit(1)
}
// farm_media.FN(*args) in Python, its result as JSON
const farm = (fn: string, ...args: unknown[]) => {
  const py = `import json, sys; sys.path.insert(0, ${JSON.stringify(modal)}); import farm_media as fm; a = json.load(sys.stdin); print(json.dumps(fm.${fn}(*a, log=lambda _: None) if "${fn}" != "outputs" else fm.outputs(*a)))`
  const r = spawnSync("python3", ["-I", "-c", py], { input: JSON.stringify(args), encoding: "utf8" })
  if (r.status !== 0) fail(`farm_media.${fn}: ${r.stderr.trim().split("\n").pop()}`)
  return JSON.parse(r.stdout)
}
const sha = (f: string) => createHash("sha256").update(readFileSync(f)).digest("hex")

// the tar's excludes leave the same media out of the hash list
const skip = excluded(["./.git", "./out/*", "./.raea-cloud-out"])
if (!skip("out/a.mp4") || !skip("out/x/b.png") || !skip(".git/lfs/x") || skip("media/plate.png") || skip("outro.mp4")) fail("excluded() doesn't match tar's excludes")

const lfs = lfsOf(clone)
if (typeof lfs === "string") fail(`the clone's media would go in the tar: ${lfs}`)
const lfsOk = lfs as Exclude<typeof lfs, string>
const sent = await inputs(clone, lfsOk)
if (!sent.length) fail("no media went by hash")

const work = mkdtempSync(path.join(tmpdir(), "farm-job-"))
const cache = mkdtempSync(path.join(tmpdir(), "farm-blobs-"))
writeFileSync(path.join(work, ".raea-cloud-media.json"), JSON.stringify({ objects: sent }))
const got = farm("fetch", work, cache)
for (const m of sent) if (sha(path.join(work, m.path)) !== m.oid) fail(`the farm's ${m.path} isn't its hash`)

// the farm's render: 1.5 MB, new to R2
mkdirSync(path.join(work, "out"), { recursive: true })
writeFileSync(path.join(work, "out", "cut.mp4"), randomBytes(1_500_000))
const made = farm("outputs", work, ["out/cut.mp4"])
const o = await outputs(clone, lfsOk, made)
if (!o.send.every((m) => m.href)) fail("raea's LFS gave no upload URL for the new render")
const up = farm("upload", work, o.send)
if (up.uploaded !== 1) fail(`the farm sent ${up.uploaded} files to R2, not 1`)
await o.done()
const pointer = readFileSync(path.join(clone, "out", "cut.mp4"), "utf8")
if (!pointer.startsWith("version https://git-lfs.github.com/spec/v1") || !pointer.includes(made[0].oid)) fail("the clone has no pointer for the render")
console.log(JSON.stringify({ inputs: sent.length, fetched: got.fetched, render: made[0] }))
