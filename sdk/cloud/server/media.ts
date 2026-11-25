// A GPU job's media by hash (docs/cloud-render.feature CR23), on the box's side. When the project is a raea repo (its
// .lfsconfig points at raea's LFS) and this box is signed in (RAEA_AGENT_KEY, an agent key with the repos scope, or
// RAEA_TOKEN), its media doesn't go to the farm in the tar:
//   inputs   each LFS file whose copy here is its committed one (or still a pointer) goes by its object id, with a
//            download URL raea's LFS signs for it; the farm fetches it from R2 itself (farm_media.py). A media file
//            changed here and not pushed goes in the tar as before.
//   outputs  the job's media stays on the farm; raea's LFS signs an upload URL for each, the farm PUTs it to R2, the
//            upload is verified (so it counts in the account's storage), and here it becomes a pointer file, so its
//            bytes never pass through this box. `git lfs pull PATH` brings one here when something wants it.

import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"
import { createReadStream, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Media } from "../src/index.ts"

const LFS = "application/vnd.git-lfs+json"
const POINTER = "version https://git-lfs.github.com/spec/v1"

interface Action {
  href: string
  header?: Record<string, string>
}
interface BatchObject {
  oid: string
  size: number
  actions?: { download?: Action; upload?: Action; verify?: Action }
  error?: { code: number; message: string }
}

export interface Lfs {
  url: string
  token: string
}

/** The project's raea LFS and this box's sign-in, or why its media goes in the tar. */
export function lfsOf(root: string): Lfs | string {
  if (process.env.RAEA_CLOUD_GPU_MEDIA === "0") return "RAEA_CLOUD_GPU_MEDIA=0"
  const token = process.env.RAEA_AGENT_KEY ?? process.env.RAEA_TOKEN
  if (!token) return "no raea sign-in here (RAEA_AGENT_KEY)"
  const cfg = path.join(root, ".lfsconfig")
  if (!existsSync(cfg)) return "the project has no .lfsconfig"
  const url = spawnSync("git", ["config", "-f", cfg, "--get", "lfs.url"], { encoding: "utf8" }).stdout.trim()
  if (!url) return "the project's .lfsconfig names no LFS"
  if (spawnSync("git", ["lfs", "version"]).status !== 0) return "git-lfs isn't here"
  return { url, token }
}

async function batch(lfs: Lfs, operation: "download" | "upload", objects: { oid: string; size: number }[]): Promise<BatchObject[]> {
  if (!objects.length) return []
  const r = await fetch(`${lfs.url}/objects/batch`, {
    method: "POST",
    headers: { authorization: `Bearer ${lfs.token}`, accept: LFS, "content-type": LFS },
    body: JSON.stringify({ operation, transfers: ["basic"], objects }),
  })
  if (!r.ok) throw new Error(`raea's LFS answered ${r.status} for a ${operation}: ${(await r.text()).slice(0, 200)}`)
  return ((await r.json()) as { objects: BatchObject[] }).objects
}

function sha256(file: string): Promise<string> {
  return new Promise((ok, fail) => {
    const h = createHash("sha256")
    createReadStream(file).on("data", (b) => h.update(b)).on("end", () => ok(h.digest("hex"))).on("error", fail)
  })
}

const isPointer = (file: string) => {
  try {
    return readFileSync(file).subarray(0, POINTER.length).toString() === POINTER
  } catch {
    return false
  }
}

/** tar --exclude patterns (./out/*, ./.git) as a test of a path in the project, the way tar applies them: its
 *  wildcards cross /, and a folder left out takes its files with it */
export function excluded(patterns: string[]): (p: string) => boolean {
  const res = patterns.map((e) => new RegExp("^" + e.replace(/^\.\//, "").replace(/[.+^${}()|\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".") + "$"))
  return (p) => {
    const parts = p.split("/")
    for (let i = 1; i <= parts.length; i++) if (res.some((r) => r.test(parts.slice(0, i).join("/")))) return true
    return false
  }
}

/** The project's media that can go by hash, each with its signed download URL: all but what `skip` leaves out (the
 *  tar's excludes, so an old render in out/ isn't fetched for a new one). */
export async function inputs(root: string, lfs: Lfs, skip: (p: string) => boolean = () => false): Promise<Media[]> {
  const ls = spawnSync("git", ["-C", root, "lfs", "ls-files", "--json"], { encoding: "utf8", maxBuffer: 64 << 20 })
  if (ls.status !== 0) return []
  const files = ((JSON.parse(ls.stdout || "{}") as { files?: { name: string; oid: string; size: number }[] }).files ?? [])
  const same: { path: string; oid: string; size: number }[] = []
  for (const f of files) {
    if (skip(f.name)) continue
    const p = path.join(root, f.name)
    if (!existsSync(p)) continue
    if (isPointer(p) || (await sha256(p)) === f.oid) same.push({ path: f.name, oid: f.oid, size: f.size })
  }
  const got = new Map((await batch(lfs, "download", same.map(({ oid, size }) => ({ oid, size })))).map((o) => [o.oid, o]))
  return same.flatMap((f) => {
    const d = got.get(f.oid)?.actions?.download
    return d ? [{ ...f, href: d.href, header: d.header ?? {} }] : []
  })
}

/** The job's media outputs with where in R2 each goes; `done` then verifies them and writes their pointers here. */
export async function outputs(root: string, lfs: Lfs, made: Media[]): Promise<{ send: Media[]; done: () => Promise<number> }> {
  const asked = new Map((await batch(lfs, "upload", made.map(({ oid, size }) => ({ oid, size })))).map((o) => [o.oid, o]))
  const send = made.map((m) => {
    const up = asked.get(m.oid)?.actions?.upload
    return up ? { ...m, href: up.href, header: up.header ?? {} } : m
  })
  const done = async () => {
    let bytes = 0
    for (const m of made) {
      const v = asked.get(m.oid)?.actions?.verify
      if (v) {
        const r = await fetch(v.href, { method: "POST", headers: { ...v.header, accept: LFS, "content-type": LFS }, body: JSON.stringify({ oid: m.oid, size: m.size }) })
        if (!r.ok) throw new Error(`raea's LFS didn't verify ${m.path}: ${r.status}`)
      }
      const p = path.join(root, m.path)
      mkdirSync(path.dirname(p), { recursive: true })
      writeFileSync(p, `${POINTER}\noid sha256:${m.oid}\nsize ${m.size}\n`)
      const attr = spawnSync("git", ["-C", root, "check-attr", "filter", "--", m.path], { encoding: "utf8" }).stdout
      if (!/: lfs$/m.test(attr)) spawnSync("git", ["-C", root, "lfs", "track", "--filename", m.path])
      bytes += m.size
    }
    // its objects are in R2, not in this clone's LFS store: a push sends the pointers and git-lfs leaves the rest
    if (made.length) spawnSync("git", ["-C", root, "config", "lfs.allowincompletepush", "true"])
    return bytes
  }
  return { send, done }
}

/** tar's exclude-from lines for paths that go by hash: each matched as written (glob characters escaped) */
export const excludes = (media: Media[]) => media.map((m) => "./" + m.path.replace(/[\\*?[\]]/g, (c) => "\\" + c)).join("\n") + "\n"
