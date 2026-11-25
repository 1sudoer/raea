// The organs of the chest and belly, placed in the body's own frame (frame.ts): the lungs filling each side of the rib
// cage down to the diaphragm's dome (the left with the heart's notch), the heart turned down, left and forward, the
// aorta and the pulmonary trunk, the windpipe and its bronchi; under the diaphragm the liver, stomach, spleen and
// kidneys, and the bowel. Their places and sizes are an adult's (Gray's Anatomy, 1918, public domain), scaled by the
// body; their shapes simple. Not yet: the oesophagus, the pancreas, the bladder, the great veins.
import { frameOf, interp, type Frame } from "./frame";
import { blob, loft, smoothPath, tube, v, type V3 } from "./mesh";
import { COLORS, piece, type Piece } from "./piece";

const I3: [V3, V3, V3] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];

/** the heart's shape: its base right of the middle behind the sternum, its apex down, left and forward */
function heartShape(F: Frame) {
  const k = F.k, j = F.y.jug, x = F.y.xiph;
  const base: V3 = [0.004 * k, j - 0.5 * (j - x), F.cage(j - 0.5 * (j - x)).mid + 0.005 * k];
  const apex: V3 = [0.078 * k, x - 0.004 * k, F.cage(x).front - 0.026 * k];
  const a2 = v.norm(v.sub(apex, base)), a0 = v.norm(v.cross([0, 1, 0], a2)), a1 = v.cross(a2, a0);
  const ax: [V3, V3, V3] = [a0, a1, a2], rad: V3 = [0.046 * k, 0.037 * k, 0.064 * k], c = v.lerp(base, apex, 0.45);
  const shape = (u: V3) => 1 - 0.3 * Math.max(0, u[2]) ** 1.5 + 0.06 * Math.max(0, -u[2]);
  // a point inside it (with a margin) moved out to its surface along its own direction from the middle
  const outOf = (p: V3, margin = 1.14): V3 => {
    const d = v.sub(p, c), u: V3 = [v.dot(d, a0) / rad[0], v.dot(d, a1) / rad[1], v.dot(d, a2) / rad[2]], r = v.len(u);
    if (r < 1e-6) return p;
    const n: V3 = [u[0] / r, u[1] / r, u[2] / r], lim = shape(n) * margin;
    if (r >= lim) return p;
    let q = c;
    for (let i = 0; i < 3; i++) q = v.add(q, v.mul(ax[i], rad[i] * n[i] * lim));
    return q;
  };
  return { base, apex, ax, rad, c, shape, outOf };
}

function lungs(F: Frame): Piece[] {
  const k = F.k, dT = F.vertebrae.find((q) => q.name === "T1")!.h, out: Piece[] = [], H = heartShape(F);
  const apex = F.y.jug + 0.035 * k, low = F.y.T1 - 11 * dT;
  const heartY = [F.y.xiph - 0.01 * k, F.y.jug - 0.45 * (F.y.jug - F.y.xiph), F.y.jug - 0.2 * (F.y.jug - F.y.xiph)];
  for (const [side, s] of [["left", 1], ["right", -1]] as const) {
    // the diaphragm under it: a dome, highest under the lung's middle (the right's higher, over the liver), its edges
    // falling to the recesses behind and at the side
    const top = F.y.xiph + (s < 0 ? 0.016 : 0.004) * k;
    const dome = (p: V3) => {
      const c = F.cage(top), cx = s * c.half * 0.45, cz = c.mid + 0.01 * k;
      const rho = Math.min(1.3, Math.hypot((p[0] - cx) / (c.half * 0.62), (p[2] - cz) / ((c.front - c.back) * 0.5)));
      const edge = interp([-1, 0, 1], [low, top - 0.05 * k, top - 0.03 * k], (p[2] - cz) / ((c.front - c.back) * 0.5));
      return top - (top - edge) * Math.min(1, rho * rho);
    };
    const rings: V3[][] = [];
    const N = 22;
    for (let q = 0; q <= N; q++) {
      const y = apex - (apex - low) * (q / N), taper = Math.min(1, Math.sqrt(Math.max(0.02, (apex - y) / (0.05 * k))));
      // the mediastinum's side: the heart pushes the left lung out at its levels (the cardiac notch)
      const heart = Math.max(0, 1 - Math.abs(y - heartY[1]) / (heartY[2] - heartY[0]) * 1.6);
      const xm = s > 0 ? (0.028 + 0.05 * heart) * k : (0.026 + 0.012 * heart) * k;
      const pb = (s > 0 ? 0.88 - 0.16 * heart : 0.93) * Math.PI;
      const ring: V3[] = [];
      for (let a = 0; a <= 16; a++) { const phi = 0.13 * Math.PI + (pb - 0.13 * Math.PI) * (a / 16); ring.push(F.wall(y, phi, s, undefined, 0.006 * k)); }
      const zf = ring[ring.length - 1][2], zb = ring[0][2];
      for (let a = 1; a <= 7; a++) ring.push([s * xm, y, zf + (zb - zf) * (a / 8)]);
      // (narrowed to its apex round its own middle)
      const c = v.mul(ring.reduce((t, p) => v.add(t, p), [0, 0, 0] as V3), 1 / ring.length);
      // (over the dome, and clear of the heart: its cardiac impression)
      rings.push(ring.map((p) => { const t = v.add(c, v.mul(v.sub(p, c), taper)); return H.outOf([t[0], Math.max(y, dome(t)), t[2]]); }));
    }
    const c0 = F.cage(top);
    out.push(piece(F, `lung-${side}`, "organ", "Chest", loft(rings, [[s * 0.05 * k, apex + 0.004 * k, F.cage(apex).mid], [s * c0.half * 0.45, top, c0.mid + 0.01 * k]]), COLORS.lung, 0.5));
  }
  return out;
}

function heart(F: Frame): Piece[] {
  const k = F.k, j = F.y.jug, H = heartShape(F), base = H.base;
  const out = [piece(F, "heart", "organ", "Chest", blob(H.c, H.ax, H.rad, H.shape, 28, 20), COLORS.heart, 0.45)];
  const sp = (y: number) => F.spine(y)[2];
  const aorta = smoothPath([v.add(base, [0, 0.02 * k, 0.005 * k]), [0.0, j - 0.022 * k, F.cage(j).mid], [0.012 * k, j - 0.028 * k, sp(j) + 0.03 * k], [0.018 * k, j - 0.07 * k, sp(j - 0.07 * k) + 0.028 * k], [0.014 * k, F.y.T12 - 0.02 * k, sp(F.y.T12) + 0.028 * k]], 40);
  out.push(piece(F, "aorta", "vessel", "Chest", tube(aorta, (t) => [interp([0, 0.3, 1], [0.0135, 0.012, 0.011], t) * k, interp([0, 0.3, 1], [0.0135, 0.012, 0.011], t) * k], 12), COLORS.aorta, 0.4));
  const pt = smoothPath([v.add(base, [0.012 * k, 0.004 * k, 0.025 * k]), [0.014 * k, j - 0.06 * k, F.cage(j).mid + 0.012 * k], [0.002 * k, j - 0.075 * k, F.cage(j).mid - 0.008 * k]], 16);
  out.push(piece(F, "pulmonary-trunk", "vessel", "Chest", tube(pt, () => [0.012 * k, 0.012 * k], 12), "#6f3a5a", 0.4));
  // the windpipe from the neck to its fork, and the two bronchi
  const top: V3 = [0, F.y.T1 + 0.05 * k, sp(F.y.T1 + 0.05 * k) + 0.034 * k], fork: V3 = [0, j - 0.06 * k, sp(j - 0.06 * k) + 0.036 * k];
  out.push(piece(F, "trachea", "airway", "Chest", tube(smoothPath([top, v.lerp(top, fork, 0.5), fork], 12), () => [0.009 * k, 0.009 * k], 12), COLORS.airway, 0.5));
  for (const s of [1, -1]) out.push(piece(F, `bronchus-${s > 0 ? "left" : "right"}`, "airway", "Chest", tube([fork, [s * 0.052 * k, fork[1] - (s > 0 ? 0.035 : 0.025) * k, fork[2] - 0.012 * k]], () => [0.0065 * k, 0.0065 * k], 10), COLORS.airway, 0.5));
  return out;
}

function belly(F: Frame): Piece[] {
  const k = F.k, x = F.y.xiph, out: Piece[] = [];
  const at = (y: number) => F.cage(y);
  // the liver: under the right dome, from the right wall across past the middle, thinning to the left
  const ly = x - 0.045 * k, lc = at(ly), right = -lc.half + 0.008 * k, left = 0.06 * k;
  out.push(piece(F, "liver", "organ", "Spine2", blob([(right + left) / 2, ly, lc.mid + 0.008 * k], I3, [(left - right) / 2, 0.062 * k, (lc.front - lc.back) * 0.4], (u) => (1 - 0.42 * Math.max(0, u[0]) ** 1.4) * (1 - 0.22 * Math.max(0, -u[1])) * (1 - 0.45 * Math.max(0, -u[2]) * Math.max(0, u[0] + 0.25)), 30, 20), COLORS.liver, 0.4));
  // the stomach: a J down the left from its top under the heart to the pylorus right of the middle
  const m = (y: number) => at(y).mid + 0.012 * k;
  const st = smoothPath([[0.025 * k, x - 0.03 * k, m(x) - 0.012 * k], [0.065 * k, x - 0.026 * k, m(x) - 0.01 * k], [0.082 * k, x - 0.05 * k, m(x) + 0.012 * k], [0.06 * k, x - 0.09 * k, m(x) + 0.03 * k], [0.015 * k, x - 0.096 * k, m(x) + 0.034 * k], [-0.022 * k, x - 0.08 * k, m(x) + 0.024 * k]], 40);
  out.push(piece(F, "stomach", "organ", "Spine2", tube(st, (t) => { const r = interp([0, 0.15, 0.4, 0.65, 0.85, 1], [0.018, 0.034, 0.037, 0.03, 0.02, 0.011], t) * k; return [r, r * 0.85]; }, 14), COLORS.stomach, 0.45));
  out.push(piece(F, "spleen", "organ", "Spine2", blob([0.088 * k, x - 0.062 * k, at(x).back + 0.04 * k], I3, [0.021 * k, 0.045 * k, 0.032 * k], () => 1, 18, 12), COLORS.spleen, 0.45));
  for (const s of [1, -1]) {
    const y = F.y.T12 - (s > 0 ? 0.012 : 0.034) * k;
    out.push(piece(F, `kidney-${s > 0 ? "left" : "right"}`, "organ", "Spine2", blob([s * 0.058 * k, y, F.spine(y)[2] - 0.004 * k], I3, [0.027 * k, 0.052 * k, 0.02 * k], (u) => 1 - 0.28 * Math.max(0, -s * u[0]) ** 3, 18, 14), COLORS.kidney, 0.45));
  }
  // the bowel: the small intestine coiled in the middle of the belly, framed by the colon
  const y0 = F.y.L5 - 0.025 * k, y1 = F.y.T12 - 0.055 * k, ym = (y0 + y1) / 2;
  const zc = (y: number) => (F.skin(y).front + F.spine(y)[2]) / 2 + 0.012 * k, zf = (y: number) => F.skin(y).front - 0.022 * k;
  const coil: V3[] = [];
  for (let q = 0; q <= 260; q++) {
    const t = q / 260, y = ym - 0.006 * k + (y1 - y0) * 0.34 * Math.sin(2 * Math.PI * (3.1 * t) + 0.4) * (0.75 + 0.25 * Math.cos(2 * Math.PI * 11 * t));
    const z = zc(y) + (zf(y) - zc(y)) * (0.45 + 0.42 * Math.sin(2 * Math.PI * 7.7 * t + 1));
    coil.push([0.05 * k * Math.sin(2 * Math.PI * 5.3 * t + 0.3) * (0.8 + 0.2 * Math.sin(2 * Math.PI * 2.3 * t)), y, z]);
  }
  out.push(piece(F, "small-intestine", "organ", "Spine1", tube(smoothPath(coil, 520), () => [0.0105 * k, 0.0105 * k], 8), COLORS.bowel, 0.5));
  const colon = smoothPath([[-0.072 * k, y0 + 0.005 * k, zc(y0)], [-0.086 * k, y1 - 0.01 * k, zc(y1) - 0.012 * k], [-0.07 * k, y1 + 0.012 * k, zc(y1)], [0, y1 - 0.012 * k, zf(y1) - 0.006 * k], [0.08 * k, y1 + 0.022 * k, zc(y1) - 0.012 * k], [0.09 * k, y0 + 0.01 * k, zc(y0) - 0.016 * k], [0.04 * k, y0 - 0.035 * k, zc(y0)], [0.004 * k, y0 - 0.06 * k, F.spine(F.y.L5)[2] + 0.01 * k]], 90);
  out.push(piece(F, "colon", "organ", "Spine1", tube(colon, (t) => { const r = interp([0, 0.15, 0.5, 0.8, 1], [0.025, 0.022, 0.02, 0.017, 0.016], t) * k; return [r, r]; }, 12), COLORS.colon, 0.5));
  return out;
}

export function organsOf(skel: Parameters<typeof frameOf>[0], F: Frame = frameOf(skel)): Piece[] {
  return [...lungs(F), ...heart(F), ...belly(F)];
}
