// Where the inside of a body is, read off its own skeleton and skin (skeleton.json's joints and `surface` samples):
// the spine's levels between its joints, the line its vertebrae stand on, and the rib cage's inner wall at any height,
// so a short broad body gets a short broad rib cage. Metres in the body's frame at rest (+z ahead, +y up, left +x).
import type { Skeleton } from "../character/all";
import type { V3 } from "./mesh";

export interface Frame {
  /** how big the body is against SOMA's 1.75 m man (the trunk's length between Hips and Neck1) */
  k: number;
  joint: (name: string) => V3;
  /** heights: the top of T1, the jugular notch, the sternum's xiphisternal joint and the xiphoid's tip, the bottom of
   *  T12 and of L5; each vertebra's middle (C1..C7, T1..T12, L1..L5 in order) and height */
  y: { T1: number; jug: number; xiph: number; xtip: number; T12: number; L5: number };
  vertebrae: { name: string; y: number; h: number }[];
  /** the skin's front and back at the midline and its half-width, at a height (smoothed from the samples) */
  skin: (y: number) => { front: number; back: number; half: number };
  /** a vertebral body's middle at a height (x 0) */
  spine: (y: number) => V3;
  /** the rib cage's inner wall at a height: its back (behind the ribs' angles), front (behind the sternum), the depth
   *  its widest point is at, and its half-width there (`w`, the rib's own when given) */
  cage: (y: number, w?: number) => { back: number; front: number; mid: number; half: number };
  /** a point on the cage's wall: phi 0 at the back's midline, pi/2 at the side, pi at the front; side +1 left, -1 right */
  wall: (y: number, phi: number, side: number, w?: number, inset?: number) => V3;
}

const TRUNK = /^(chest|torso|waist|hips|neck)$/;
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

/** a curve through (x, y) knots, linear between them and flat past the ends */
export function interp(xs: number[], ys: number[], x: number): number {
  if (x <= xs[0]) return ys[0];
  for (let i = 1; i < xs.length; i++) if (x <= xs[i]) return lerp(ys[i - 1], ys[i], (x - xs[i - 1]) / (xs[i] - xs[i - 1]));
  return ys[ys.length - 1];
}

export function frameOf(skel: Skeleton): Frame {
  const J = new Map(skel.joints.map((j) => [j.name, j.head as V3]));
  const joint = (n: string): V3 => {
    const h = J.get(n);
    if (!h) throw new Error(`anatomy: ${skel.name} has no joint ${n} (a SOMA body's skeleton is needed)`);
    return h;
  };
  const hips = joint("Hips"), sp1 = joint("Spine1"), sp2 = joint("Spine2"), chest = joint("Chest"), n1 = joint("Neck1"), n2 = joint("Neck2"), head = joint("Head");
  const k = (n1[1] - hips[1]) / 0.459;

  // the levels: T1's top at Neck1, T12's bottom a little under Chest, L5's under Hips; C1 to C7 up to the skull
  const T1 = n1[1], T12 = chest[1] - 0.02 * k, L5 = hips[1] + 0.02 * k;
  const dT = (T1 - T12) / 12, dL = (T12 - L5) / 5, dC = (head[1] - 0.004 * k - T1) / 7;
  const vertebrae: Frame["vertebrae"] = [];
  for (let i = 0; i < 7; i++) vertebrae.push({ name: `C${i + 1}`, y: head[1] - 0.004 * k - (i + 0.5) * dC, h: dC });
  for (let i = 0; i < 12; i++) vertebrae.push({ name: `T${i + 1}`, y: T1 - (i + 0.5) * dT, h: dT });
  for (let i = 0; i < 5; i++) vertebrae.push({ name: `L${i + 1}`, y: T12 - (i + 0.5) * dL, h: dL });
  const y = { T1, jug: T1 - 2.5 * dT, xiph: T1 - 8.5 * dT, xtip: T1 - 8.5 * dT - 0.035 * k, T12, L5 };

  // the skin in bands 2 cm apart: the front and back near the midline, the half-width; gaps filled, then smoothed
  const pts: V3[] = [];
  for (const x of skel.surface ?? []) if (TRUNK.test(x.part)) { const h = skel.joints[x.j[0]].head; pts.push([h[0] + x.o[0], h[1] + x.o[1], h[2] + x.o[2]]); }
  if (pts.length < 50) throw new Error(`anatomy: ${skel.name}'s skeleton.json has too few skin samples of the trunk (${pts.length})`);
  const y0 = hips[1] - 0.14 * k, y1 = n1[1] + 0.06 * k, step = 0.02, nb = Math.ceil((y1 - y0) / step) + 1;
  const ys = Array.from({ length: nb }, (_, i) => y0 + i * step);
  const band = (f: (p: V3[]) => number) => ys.map((yy) => { const q = pts.filter((p) => Math.abs(p[1] - yy) < 0.03); return q.length >= 3 ? f(q) : NaN; });
  const mid = (q: V3[]) => q.filter((p) => Math.abs(p[0]) < 0.06 * k);
  let front = band((q) => { const m = mid(q); return m.length ? Math.max(...m.map((p) => p[2])) : NaN; });
  let back = band((q) => { const m = mid(q); return m.length ? Math.min(...m.map((p) => p[2])) : NaN; });
  let half = band((q) => { const a = q.map((p) => Math.abs(p[0])).sort((s, t) => s - t); return a[Math.floor(0.92 * (a.length - 1))]; });
  // (a band whose midline samples lie all on one side reads its front and back as one: kept only when 12 cm apart)
  back = back.map((b, i) => (Number.isFinite(b) && Number.isFinite(front[i]) && front[i] - b > 0.12 * k ? b : NaN));
  front = front.map((f, i) => (Number.isFinite(back[i]) ? f : NaN));
  const fill = (a: number[]) => {
    const ok = a.map((x, i) => [i, x] as const).filter(([, x]) => Number.isFinite(x));
    if (!ok.length) throw new Error(`anatomy: ${skel.name}'s trunk could not be measured`);
    const filled = a.map((x, i) => (Number.isFinite(x) ? x : interp(ok.map(([j]) => j), ok.map(([, z]) => z), i)));
    return filled.map((_, i) => { const w = filled.slice(Math.max(0, i - 2), i + 3); return w.reduce((s, t) => s + t, 0) / w.length; });
  };
  front = fill(front); back = fill(back); half = fill(half);
  const skin = (yy: number) => ({ front: interp(ys, front, yy), back: interp(ys, back, yy), half: interp(ys, half, yy) });

  // the spine: through the trunk's joints, set back over the thoracic curve, and never nearer the back than 5.5 cm
  const knots = [hips, sp1, sp2, chest, n1, n2, head];
  const thor = T1 - 5 * dT;
  const spine = (yy: number): V3 => {
    const z = interp(knots.map((p) => p[1]), knots.map((p) => p[2]), yy) - 0.026 * k * Math.exp(-(((yy - thor) / (4 * dT)) ** 2));
    return [0, yy, yy < T1 + 0.01 ? Math.max(z, skin(yy).back + 0.055 * k) : z];
  };

  // the rib cage's wall: its back behind the ribs' angles (3.2 cm in from the back's skin, the muscles there), its
  // front behind the sternum (1.3 cm in), its widest a little behind the middle; its half-width from the chest's
  // skin under the arms (2 cm of muscle at the side), narrowing to the top as ribs 1 to 3 do
  const chestHalf = Math.max(...ys.filter((yy) => yy > y.xiph && yy < T1 - 4 * dT).map((yy) => skin(yy).half)) - 0.02 * k;
  const cage = (yy: number, w?: number) => {
    const s = skin(yy), b = s.back + 0.032 * k, f = s.front - 0.013 * k;
    const profile = interp([T12 - 3 * dL, T12 - dT, T1 - 8 * dT, T1 - 4 * dT, T1 - 1.5 * dT], [0.8, 0.97, 1, 0.86, 0.5], yy);
    return { back: b, front: f, mid: lerp(b, f, 0.45), half: w ?? chestHalf * profile };
  };
  const wall = (yy: number, phi: number, side: number, w?: number, inset = 0): V3 => {
    const c = cage(yy, w), s = Math.sin(phi), co = Math.cos(phi);
    const x = side * Math.max(0, c.half - inset) * Math.pow(Math.abs(s), 0.85);
    const z = co >= 0 ? c.mid - (c.mid - c.back - inset) * Math.pow(co, 1.15) : c.mid + (c.front - c.mid - inset) * Math.pow(-co, 1.15);
    return [x, yy, z];
  };
  return { k, joint, y, vertebrae, skin, spine, cage, wall };
}
