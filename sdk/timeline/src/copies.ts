// A composition's footage or sound played from a copy the host made of its file (raea docs/timeline.feature TL8d,
// TL7k, TL7n): a sound with its noise reduced or its voice changed, footage held steady, footage with its faces
// retouched. The element's src is the copy and its data-* props say what it was made from and how; its clip's props are
// a film's for the same (its own file, the copy and the settings), and a set of those is the host's edit back. Footage
// cut out (TL7l, TL7m) wears mattes made of what it plays, as raea's Video matte prop.
import { find, type Timeline, type Value } from "./model"
import type { Op } from "./ops"

/** What a copy was made from (`of`, a path in the composition's media folder) and how, as the host reads its element. */
export interface Copied {
  /** what it plays is a copy of `of` with its noise reduced or its voice changed (an <Audio>'s) */
  heard?: { of: string; denoise?: boolean; voice?: string }
  /** what it plays is a copy of `of` held steady at `level` (a <Video>'s) */
  steady?: { of: string; level: string }
  /** what it plays is a copy of `of` with its faces retouched, each amount 0 to 1 (a <Video>'s): `of` is its file, or
   *  its steadied copy when it is held steady too (the retouch over the steadying) */
  retouch?: { of: string; smooth?: number; brighten?: number; slim?: number; eyes?: number }
  /** it wears a cutout's mattes (a <Video>'s): the folder in the media folder, the file's rate, one each `every` frames,
   *  how many; what they are of (person, subject or brushed) and, brushed, the second brushed at and the strokes' JSON */
  cutout?: CutoutMatte & { kind: string; at?: number; strokes?: string }
}

/** A cutout's mattes as a <Video>'s matte prop holds them. */
export interface CutoutMatte {
  dir: string
  fps: number
  every: number
  count: number
}

/** The host's edits that set what an element plays: `file`, a copy of `of` made so; no `of`, `file` as it is. A
 *  cutout's sets the mattes it wears; with none, it wears none. */
export type CopyEdit =
  | { op: "heard"; i: number; file: string; of?: string; denoise?: boolean; voice?: string }
  | { op: "steady"; i: number; file: string; of?: string; level?: string }
  | { op: "retouch"; i: number; file: string; of?: string; smooth?: number; brighten?: number; slim?: number; eyes?: number }
  | { op: "cutout"; i: number; kind?: string; matte?: CutoutMatte; at?: number; strokes?: string }

const RETOUCH = { smooth: "retouchSmooth", brighten: "retouchBrighten", slim: "retouchSlim", eyes: "retouchEyes" } as const

/** A media element's clip props: its own file, and the copy it plays with what was done to it (steadied under a
 *  retouch: the copy the retouch was made from). */
export function copyProps(file: string, r: Copied): Record<string, Value | boolean> {
  const own = r.heard?.of ?? r.steady?.of ?? r.retouch?.of ?? file
  return {
    file: own,
    ...(r.heard && { voiced: file, denoise: !!r.heard.denoise, voice: r.heard.voice ?? "" }),
    ...(r.steady && { stabilise: r.steady.level, steadied: r.retouch?.of ?? file }),
    ...(r.retouch && { retouched: file, ...Object.fromEntries(Object.entries(RETOUCH).map(([k, p]) => [p, r.retouch![k as keyof typeof RETOUCH] ?? 0])) }),
    ...(r.cutout && {
      matte: r.cutout.kind,
      matted: `${r.cutout.dir}/index.json`,
      matteFps: r.cutout.fps,
      matteEvery: r.cutout.every,
      matteCount: r.cutout.count,
      ...(r.cutout.at !== undefined && { brushAt: r.cutout.at }),
      ...(r.cutout.strokes && { brushStrokes: r.cutout.strokes }),
    }),
  }
}

/** The host's edits for a set of copy props on media clips `ids` (each `i` its element's place), or undefined when `op`
 *  isn't one: a sound's voice (TL8d), footage held steady (TL7k), retouched (TL7n) or cut out (TL7l, TL7m). */
export function copyEdits(t: Timeline, op: Op, ids: string[], place: (id: string) => number | undefined): CopyEdit[] | undefined {
  const p = op.op === "set" && Object.keys(op.set).join() === "props" ? op.set.props : undefined
  if (!p) return undefined
  const of = (id: string) => find(t, id)?.clip.props?.file
  const has = (kind: string, keys: string[]) => Object.keys(p).every((k) => keys.includes(k)) && ids.every((id) => find(t, id)?.clip.kind === kind && typeof of(id) === "string" && place(id) !== undefined)
  const each = (f: (i: number, own: string) => CopyEdit) => ids.map((id) => f(place(id)!, of(id) as string))
  // a retouch is made from what plays under it: its steadied copy, when it is held steady
  const under = (id: string) => {
    const s = find(t, id)?.clip.props?.steadied
    return typeof s === "string" && s ? s : (of(id) as string)
  }
  if (has("audio", ["denoise", "voice", "voiced"]))
    return each((i, own) => (p.voiced ? { op: "heard", i, file: String(p.voiced), of: own, ...(p.denoise === true && { denoise: true }), ...(p.voice && { voice: String(p.voice) }) } : { op: "heard", i, file: own }))
  if (has("video", ["stabilise", "steadied", "stabiliseZoom"]))
    return each((i, own) => (p.steadied ? { op: "steady", i, file: String(p.steadied), of: own, level: String(p.stabilise ?? "") } : { op: "steady", i, file: own }))
  if (has("video", [...Object.values(RETOUCH), "retouched", "retouchFrom", "retouchTo"]))
    return ids.map((id) => {
      const i = place(id)!, from = under(id)
      if (!p.retouched) return { op: "retouch", i, file: from }
      const amounts = Object.entries(RETOUCH).flatMap(([k, prop]) => (typeof p[prop] === "number" && (p[prop] as number) > 0 ? [[k, p[prop] as number]] : []))
      return { op: "retouch", i, file: String(p.retouched), of: from, ...Object.fromEntries(amounts) }
    })
  if (has("video", ["matte", "matted", "matteFps", "matteEvery", "matteCount", "brushAt", "brushStrokes"]))
    return each((i) => {
      const dir = typeof p.matted === "string" ? p.matted.replace(/\/index\.json$/, "") : ""
      if (!dir) return { op: "cutout", i }
      const brushed = p.matte === "brushed"
      return {
        op: "cutout",
        i,
        kind: String(p.matte ?? ""),
        matte: { dir, fps: Number(p.matteFps), every: Number(p.matteEvery ?? 1), count: Number(p.matteCount) },
        ...(brushed && typeof p.brushAt === "number" && { at: p.brushAt }),
        ...(brushed && typeof p.brushStrokes === "string" && { strokes: p.brushStrokes }),
      }
    })
  return undefined
}
