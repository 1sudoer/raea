// What a straight path through a posed body passes through: each piece of its anatomy (anatomyOf) the ray goes in and
// out of, in order along it, and where it enters and leaves the skin (the body's surface samples, skeleton.json's
// `surface`, nearest the ray). For a bullet's path in a kill cam, its wounds, and what the shot hit.
import { pose as posed, qinv, qrot, qyaw, skinJoints, surface, toBody, toScene, type Placement, type Pose, type Skeleton } from "../character/all";
import type { V3 } from "./mesh";
import type { Kind, Piece } from "./piece";

export interface Hit {
  /** the piece's name ("rib-5-left", "lung-left", "heart"), or "skin" */
  part: string;
  kind: Kind | "skin";
  /** where the path goes into it and out of it, in the scene */
  entry: V3;
  exit: V3;
  /** how far along the path each is, metres in the scene */
  t0: number;
  t1: number;
}

const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

const bounds = new WeakMap<Piece, { c: V3; r: number }>();
function boundOf(p: Piece) {
  let b = bounds.get(p);
  if (!b) {
    const P = p.geo.positions, n = P.length / 3, c: V3 = [0, 0, 0];
    for (let i = 0; i < n; i++) { c[0] += P[3 * i] / n; c[1] += P[3 * i + 1] / n; c[2] += P[3 * i + 2] / n; }
    let r = 0;
    for (let i = 0; i < n; i++) r = Math.max(r, Math.hypot(P[3 * i] - c[0], P[3 * i + 1] - c[1], P[3 * i + 2] - c[2]));
    bounds.set(p, (b = { c, r }));
  }
  return b;
}

/** each crossing of a ray with a closed mesh: its distance along the ray and whether it goes in (the face looks back) */
function crossings(p: Piece, o: V3, d: V3): { t: number; inward: boolean }[] {
  const b = boundOf(p), oc = sub(b.c, o), tc = dot(oc, d);
  if (dot(oc, oc) - tc * tc > b.r * b.r) return [];
  const P = p.geo.positions, I = p.geo.indices, out: { t: number; inward: boolean }[] = [];
  const at = (k: number): V3 => [P[3 * k], P[3 * k + 1], P[3 * k + 2]];
  for (let i = 0; i < I.length; i += 3) {
    const a = at(I[i]), e1 = sub(at(I[i + 1]), a), e2 = sub(at(I[i + 2]), a), h = cross(d, e2), det = dot(e1, h);
    if (Math.abs(det) < 1e-12) continue;
    const f = 1 / det, s = sub(o, a), u = f * dot(s, h);
    if (u < 0 || u > 1) continue;
    const q = cross(s, e1), w = f * dot(d, q);
    if (w < 0 || u + w > 1) continue;
    const t = f * dot(e2, q);
    if (t > 0) out.push({ t, inward: dot(cross(e1, e2), d) < 0 });
  }
  return out.sort((x, y) => x.t - y.t);
}

/**
 * The pieces of `anatomy` a ray from `origin` along `dir` (in the scene) passes through on the body posed `pose` and
 * placed `at`, and the skin it enters and leaves, in order along the ray. A piece the ray crosses twice (a rib's both
 * sides, a lung in and out and in again) is listed once each time through.
 */
export function hits(skel: Skeleton, pose: Pose, anatomy: { pieces: Piece[] }, origin: V3, dir: V3, at: Placement = {}): Hit[] {
  const k = at.scale ?? 1, yaw = qyaw(-(at.yaw ?? 0));
  const o = toBody(at, origin) as V3, dl = Math.hypot(...dir), d = qrot(yaw, [dir[0] / dl, dir[1] / dl, dir[2] / dl]) as V3;
  const w = posed(skel, pose), J = new Map(skinJoints(skel, w).map((j) => [j.name, j]));
  const scene = (t: number): V3 => toScene(at, [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t]) as V3;
  const out: Hit[] = [];
  for (const p of anatomy.pieces) {
    const j = J.get(p.joint);
    if (!j) continue;
    const inv = qinv(j.rot), ol = qrot(inv, sub(o, j.pos as V3)) as V3, dl_ = qrot(inv, d) as V3;
    let open: number | null = null;
    for (const c of crossings(p, ol, dl_)) {
      if (c.inward) { if (open === null) open = c.t; }
      else {
        const t0 = open ?? 0;
        out.push({ part: p.name, kind: p.kind, entry: scene(t0), exit: scene(c.t), t0: t0 * k, t1: c.t * k });
        open = null;
      }
    }
  }
  // the skin: its samples near the ray, the first facing back along it and the last facing on
  const near = surface(skel, w).map((s) => ({ s, t: dot(sub(s.at as V3, o), d) })).filter(({ s, t }) => {
    const q = sub(s.at as V3, [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t]);
    return t > 0 && dot(q, q) < 0.035 * 0.035;
  });
  const into = near.filter(({ s }) => dot(s.normal as V3, d) < 0).sort((a, b) => a.t - b.t)[0];
  const outOf = near.filter(({ s }) => dot(s.normal as V3, d) > 0).sort((a, b) => b.t - a.t)[0];
  if (into || outOf) {
    const t0 = into?.t ?? Math.min(...out.map((h) => h.t0 / k)), t1 = outOf?.t ?? Math.max(...out.map((h) => h.t1 / k));
    if (Number.isFinite(t0) && Number.isFinite(t1)) out.push({ part: "skin", kind: "skin", entry: scene(t0), exit: scene(t1), t0: t0 * k, t1: t1 * k });
  }
  return out.sort((a, b) => a.t0 - b.t0);
}
