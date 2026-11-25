// The skeleton as bones, made from the body's own frame (frame.ts): the spine's 24 vertebrae and their discs, the
// sacrum, twelve pairs of ribs with their cartilages, the sternum, the clavicles and shoulder blades, the hip bones,
// the skull and jaw, and the long bones of the arms and legs. Shapes are simple (tubes, blobs), proportions an adult's
// (Gray's Anatomy, 1918, public domain: the ribs' slope, the sternum's levels, the vertebrae's sizes by region).
// Not yet: the bones of the hands and feet, the hyoid, the teeth.
import { frameOf, interp, type Frame } from "./frame";
import { blob, merge, smoothPath, tube, v, type Geo, type V3 } from "./mesh";
import { COLORS, piece, type Piece } from "./piece";

const I3: [V3, V3, V3] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
const ball = (c: V3, r: V3, n = 16): Geo => blob(c, I3, r, () => 1, n, Math.round(n * 0.6));
const sides = [["Left", 1], ["Right", -1]] as const;

function spine(F: Frame): Piece[] {
  const k = F.k, out: Piece[] = [];
  for (const [i, vt] of F.vertebrae.entries()) {
    const reg = vt.name[0] as "C" | "T" | "L", c = F.spine(vt.y), h = vt.h;
    const [bx, bz] = reg === "L" ? [0.021, 0.016] : reg === "T" ? [0.0135, 0.0125] : [0.009, 0.008];
    const W = (r: number) => r * k * (reg === "T" ? 1 + 0.25 * (+vt.name.slice(1) / 12) : 1);
    const parts: Geo[] = [tube([v.add(c, [0, -0.36 * h, 0]), v.add(c, [0, 0.36 * h, 0])], () => [W(bx), W(bz)], 12, [1, 0, 0])];
    // the arch round the canal, its spinous process back (and down in the chest) and the transverse processes
    const canal = W(reg === "C" ? 0.007 : 0.008), cz = c[2] - W(bz) - canal - 0.002 * k;
    const arch: V3[] = [[canal * 1.1, c[1], c[2] - W(bz) * 0.6]];
    for (let a = 0; a <= 8; a++) { const t = Math.PI * (a / 8); arch.push([canal * 1.1 * Math.cos(t), c[1], cz - canal * Math.sin(t)]); }
    arch.push([-canal * 1.1, c[1], c[2] - W(bz) * 0.6]);
    parts.push(tube(arch, () => [0.0035 * k, 0.0035 * k], 6));
    const back = cz - canal, room = back - (F.skin(vt.y).back + 0.012 * k);
    const sp = reg === "L" ? [0.03, -0.004] : reg === "T" ? [0.03, -0.022] : [0.016, -0.006];
    const len = Math.min(sp[0] * k, Math.max(0.006 * k, room));
    parts.push(tube([[0, c[1], back], [0, c[1] + (sp[1] * k * len) / (sp[0] * k), back - len]], (t) => [0.006 * k * (1 - 0.4 * t), 0.0025 * k], 6, [0, 1, 0]));
    const tp = (reg === "L" ? 0.034 : reg === "T" ? 0.026 : 0.017) * k;
    for (const s of [1, -1]) parts.push(tube([[s * canal, c[1], cz], [s * (canal + tp), c[1] + (reg === "T" ? 0.004 * k : 0), cz - (reg === "T" ? 0.012 * k : 0.002 * k)]], (t) => [0.0045 * k * (1 - 0.3 * t), 0.003 * k], 6));
    const joint = reg === "C" ? (+vt.name.slice(1) <= 4 ? "Neck2" : "Neck1") : reg === "T" ? (+vt.name.slice(1) <= 10 ? "Chest" : "Spine2") : (+vt.name.slice(1) <= 2 ? "Spine2" : "Spine1");
    out.push(piece(F, `vertebra-${vt.name}`, "bone", joint, merge(...parts), COLORS.bone, 0.55));
    // the disc under it
    const next = F.vertebrae[i + 1];
    if (next && vt.name !== "C1") {
      const d = F.spine(vt.y - 0.5 * h);
      out.push(piece(F, `disc-${vt.name}-${next.name}`, "cartilage", joint, tube([v.add(d, [0, -0.13 * h, 0]), v.add(d, [0, 0.13 * h, 0])], () => [W(bx) * 0.92, W(bz) * 0.92], 12, [1, 0, 0]), COLORS.cartilage, 0.4));
    }
  }
  // the sacrum and coccyx: curving back and down from L5, narrowing
  const s0 = F.spine(F.y.L5), path = smoothPath([s0, v.add(s0, [0, -0.04 * k, -0.012 * k]), v.add(s0, [0, -0.08 * k, -0.03 * k]), v.add(s0, [0, -0.115 * k, -0.052 * k])], 14);
  out.push(piece(F, "sacrum", "bone", "Hips", tube(path, (t) => [interp([0, 0.4, 0.7, 1], [0.05, 0.042, 0.022, 0.008], t) * k, interp([0, 1], [0.013, 0.005], t) * k], 12, [1, 0, 0]), COLORS.bone, 0.55));
  return out;
}

/** where each true rib meets the sternum, as a fraction of the way from the jugular notch to the xiphisternal joint */
const STERNAL = [0.08, 0.32, 0.5, 0.65, 0.79, 0.92, 1.0];
/** how far each true rib's cartilage climbs from its rib's end to the sternum */
const RISE = [0, 0, 0.005, 0.012, 0.02, 0.03, 0.045];
/** where each rib's bone ends round its loop (pi at the front's middle) */
const BONY_END = [0.8, 0.82, 0.8, 0.78, 0.76, 0.74, 0.72, 0.72, 0.7, 0.68, 0.58, 0.45];
const RIB_R = [0.0048, 0.0058, 0.0062, 0.0065, 0.0066, 0.0066, 0.0065, 0.0063, 0.006, 0.0056, 0.005, 0.0045];

function sternum(F: Frame) {
  const k = F.k, j = F.y.jug, x = F.y.xiph;
  const ys = [j, j - 0.25 * (j - x), x, F.y.xtip];
  const half = (y: number) => interp([F.y.xtip, x, j - 0.27 * (j - x), j - 0.2 * (j - x), j], [0.006, 0.014, 0.017, 0.014, 0.026], y) * k;
  const at = (y: number): V3 => [0, y, F.cage(y).front - 0.002 * k];
  return { half, at, geo: tube(smoothPath(ys.map(at), 16), (t) => [half(interp([0, 1], [j, F.y.xtip], t)), 0.0055 * k], 12, [1, 0, 0]) };
}

function ribs(F: Frame): Piece[] {
  const k = F.k, out: Piece[] = [], st = sternum(F), dT = F.vertebrae.find((q) => q.name === "T1")!.h;
  out.push(piece(F, "sternum", "bone", "Chest", st.geo, COLORS.bone, 0.55));
  const joinAt: Record<number, V3> = {};
  for (const [side, s] of sides) {
    for (let i = 1; i <= 12; i++) {
      const yh = F.y.T1 - (i - 0.5) * dT, sp = F.spine(yh);
      const ys = i <= 7 ? F.y.jug - STERNAL[i - 1] * (F.y.jug - F.y.xiph) : F.y.xiph - (i - 7) * 0.026 * k;
      const yEnd = ys - (i <= 7 ? RISE[i - 1] : 0.045) * k;
      const pe = BONY_END[i - 1] * Math.PI, w = F.cage(yh - 0.4 * (yh - yEnd)).half * (i >= 11 ? 0.94 : 1);
      // the bony rib: from its head on the vertebra back and out to its angle, round the side and down to its end
      const yOf = (phi: number) => yh - (yh - yEnd) * Math.pow(phi / pe, 1.5);
      const loop: V3[] = [];
      for (let q = 0; q <= 14; q++) { const phi = 0.1 * Math.PI + (pe - 0.1 * Math.PI) * (q / 14); loop.push(F.wall(yOf(phi), phi, s, w)); }
      const path = smoothPath([[s * 0.016 * k, yh, sp[2]], [s * 0.03 * k, yh, sp[2] - 0.012 * k], ...loop], 40);
      const r = RIB_R[i - 1] * k, joint = i <= 9 ? "Chest" : "Spine2";
      out.push(piece(F, `rib-${i}-${side.toLowerCase()}`, "bone", joint, tube(path, (t) => [r * (t < 0.08 ? 0.7 : 1) * (1 - 0.2 * t * t), r * 0.45], 8), COLORS.bone, 0.55));
      // its cartilage: a true rib's to the sternum's edge, rising; ribs 8 to 10's up to the cartilage over them
      const end = path[path.length - 1];
      let to: V3 | undefined;
      if (i <= 7) to = [s * st.half(ys) * 0.95, ys, st.at(ys)[2]];
      else if (i <= 10) to = joinAt[s * 100 + i - 1];
      if (to) {
        const mid: V3 = v.lerp(end, to, 0.5);
        const cp = smoothPath([end, [mid[0] * 1.04, mid[1] - (i <= 7 ? 0.004 * k : 0.008 * k), mid[2] + 0.004 * k], to], 14);
        out.push(piece(F, `cartilage-${i}-${side.toLowerCase()}`, "cartilage", "Chest", tube(cp, () => [r * 0.85, r * 0.5], 8), COLORS.cartilage, 0.4));
        joinAt[s * 100 + i] = cp[Math.round(cp.length * 0.45)];
      }
    }
  }
  return out;
}

function girdle(F: Frame): Piece[] {
  const k = F.k, out: Piece[] = [], dT = F.vertebrae.find((q) => q.name === "T1")!.h;
  for (const [side, s] of sides) {
    const arm = F.joint(`${side}Arm`), j = F.y.jug, f = F.cage(j).front;
    const acromion: V3 = [arm[0] - s * 0.012 * k, arm[1] + 0.035 * k, arm[2] + 0.004 * k];
    const clav = smoothPath([[s * 0.022 * k, j + 0.006 * k, f - 0.004 * k], [s * 0.06 * k, j + 0.012 * k, f - 0.004 * k], [s * 0.11 * k, j + 0.02 * k, f - 0.018 * k], acromion], 24);
    out.push(piece(F, `clavicle-${side.toLowerCase()}`, "bone", `${side}Shoulder`, tube(clav, (t) => [0.0068 * k * (1 + 0.3 * t), 0.0055 * k], 8), COLORS.bone, 0.55));
    // the shoulder blade: a triangular blade on the back of the ribs from its upper angle to its lower, its spine up
    // to the acromion and its neck out to the socket under it
    const yTop = F.y.T1 - 1.5 * dT, yLow = F.y.T1 - 7 * dT;
    const onBack = (x: number, y: number): V3 => [x, y, F.cage(y).back - 0.006 * k + (Math.abs(x) - 0.06 * k) * 0.45];
    const blade: V3[] = Array.from({ length: 9 }, (_, q) => { const t = q / 8, y = yTop + (yLow - yTop) * t, hw = interp([0, 0.3, 1], [0.042, 0.048, 0.004], t) * k; return onBack(s * (0.068 * k + hw * 0.9), y); });
    const bw = (t: number) => interp([0, 0.3, 1], [0.042, 0.048, 0.006], t) * k;
    const geo = merge(
      tube(blade, (t) => [bw(t), 0.0035 * k], 10, [s, 0, 0.35]),
      tube(smoothPath([onBack(s * 0.07 * k, F.y.T1 - 2.4 * dT), onBack(s * 0.12 * k, F.y.T1 - 2.0 * dT), [acromion[0], acromion[1] - 0.006 * k, acromion[2] - 0.012 * k]], 12), () => [0.0075 * k, 0.004 * k], 8),
      tube([onBack(s * 0.13 * k, F.y.T1 - 2.8 * dT), [arm[0] - s * 0.024 * k, arm[1] - 0.004 * k, arm[2] - 0.008 * k]], (t) => [0.011 * k * (1 + 0.6 * t), 0.01 * k], 10),
    );
    out.push(piece(F, `scapula-${side.toLowerCase()}`, "bone", `${side}Shoulder`, geo, COLORS.bone, 0.55));
  }
  return out;
}

function pelvis(F: Frame): Piece[] {
  const k = F.k, out: Piece[] = [], L5 = F.y.L5, sp = F.spine(L5);
  for (const [side, s] of sides) {
    const H = F.joint(`${side}Leg`), front = (y: number) => F.skin(y).front;
    const crest: V3[] = [[s * 0.042 * k, L5 + 0.02 * k, sp[2] - 0.035 * k], [s * 0.09 * k, L5 + 0.045 * k, sp[2] - 0.022 * k], [s * 0.125 * k, L5 + 0.05 * k, sp[2] + 0.015 * k], [s * 0.118 * k, H[1] + 0.07 * k, front(H[1] + 0.07 * k) - 0.022 * k]];
    const wing = smoothPath(crest.map((p, q) => [p[0] * (q === 3 ? 0.98 : 0.97), p[1] - 0.032 * k, p[2] + (q === 0 ? 0.008 * k : 0)] as V3), 20);
    const sym: V3 = [s * 0.011 * k, H[1] - 0.03 * k, front(H[1] - 0.04 * k) - 0.035 * k];
    const tub: V3 = [s * 0.055 * k, H[1] - 0.075 * k, H[2] - 0.035 * k];
    const sock = Array.from({ length: 13 }, (_, q) => { const a = (2 * Math.PI * q) / 12; return [H[0] + s * 0.012 * k, H[1] + 0.027 * k * Math.cos(a), H[2] + 0.027 * k * Math.sin(a)] as V3; });
    const geo = merge(
      tube(wing, (t) => [interp([0, 0.5, 1], [0.03, 0.04, 0.022], t) * k, 0.0045 * k], 10, [0, 1, 0]),
      tube(smoothPath([wing[10], v.lerp(wing[10], H, 0.55), [H[0] + s * 0.004 * k, H[1] + 0.02 * k, H[2]]], 10), () => [0.016 * k, 0.008 * k], 10, [0, 0, 1]),
      tube(sock, () => [0.006 * k, 0.006 * k], 8, [s, 0, 0]),
      tube(smoothPath([[H[0] - s * 0.01 * k, H[1] - 0.012 * k, H[2] + 0.022 * k], v.lerp([H[0], H[1], H[2] + 0.03 * k], sym, 0.5), sym], 10), () => [0.008 * k, 0.006 * k], 8),
      tube(smoothPath([[H[0] - s * 0.004 * k, H[1] - 0.026 * k, H[2] - 0.012 * k], tub, [s * 0.03 * k, H[1] - 0.07 * k, sym[2] - 0.03 * k], [sym[0], sym[1] - 0.022 * k, sym[2] - 0.004 * k]], 16), (t) => [interp([0, 0.3, 1], [0.011, 0.013, 0.006], t) * k, 0.008 * k], 8),
      tube([[s * 0.04 * k, L5 - 0.01 * k, sp[2] - 0.03 * k], wing[2]], () => [0.012 * k, 0.008 * k], 8),
    );
    out.push(piece(F, `hip-bone-${side.toLowerCase()}`, "bone", "Hips", geo, COLORS.bone, 0.55));
  }
  return out;
}

function skull(F: Frame): Piece[] {
  const k = F.k, l = F.joint("LeftEye"), r = F.joint("RightEye"), e = v.lerp(l, r, 0.5);
  // (measured on gb-sentry's fitted head, 11-17: the crown 1.761 over the eyes' 1.649, the back of the head 18.8 cm behind
  // them: the cranium 8 mm under the crown and 1 cm inside the back)
  const cranium = blob(v.add(e, [0, 0.03 * k, -0.08 * k]), I3, [0.071 * k, 0.074 * k, 0.098 * k], (u) => 1 - 0.1 * Math.max(0, -u[1]) - 0.04 * Math.max(0, u[2]) * Math.max(0, -u[1]), 32, 22);
  // (a face 10 cm across under the cranium read as a snowman's second ball, 11-17: as wide as the cheekbones, set back
  // into the cranium and narrowing to the upper jaw, with the cheekbones' arches back to the ears)
  const face = blob(v.add(e, [0, -0.03 * k, -0.03 * k]), I3, [0.058 * k, 0.05 * k, 0.05 * k], (u) => (1 - 0.3 * Math.max(0, -u[1])) * (1 - 0.12 * Math.max(0, -u[2])), 26, 18);
  const arches = [1, -1].map((s) => tube(smoothPath([v.add(e, [s * 0.048 * k, -0.022 * k, -0.012 * k]), v.add(e, [s * 0.062 * k, -0.024 * k, -0.04 * k]), v.add(e, [s * 0.064 * k, -0.026 * k, -0.065 * k])], 10), () => [0.007 * k, 0.004 * k], 8));
  // the upper jaw and its teeth, between the face and the lower jaw
  const maxilla = blob(v.add(e, [0, -0.078 * k, -0.004 * k]), I3, [0.031 * k, 0.017 * k, 0.03 * k], (u) => 1 - 0.2 * Math.max(0, -u[2]), 20, 12);
  const pieces = [piece(F, "skull", "bone", "Head", merge(cranium, face, maxilla, ...arches), COLORS.bone, 0.5)];
  const jaw = smoothPath([[0.05 * k, -0.035 * k, -0.07 * k], [0.048 * k, -0.087 * k, -0.058 * k], [0.034 * k, -0.105 * k, -0.012 * k], [0, -0.114 * k, 0.012 * k], [-0.034 * k, -0.105 * k, -0.012 * k], [-0.048 * k, -0.087 * k, -0.058 * k], [-0.05 * k, -0.035 * k, -0.07 * k]].map((p) => v.add(e, p as V3)), 30);
  const hasJaw = (() => { try { F.joint("Jaw"); return true; } catch { return false; } })();
  pieces.push(piece(F, "mandible", "bone", hasJaw ? "Jaw" : "Head", tube(jaw, (t) => [interp([0, 0.15, 0.5, 0.85, 1], [0.016, 0.013, 0.011, 0.013, 0.016], t) * k, 0.0045 * k], 8, [0, 1, 0]), COLORS.bone, 0.5));
  return pieces;
}

function limbs(F: Frame): Piece[] {
  const k = F.k, out: Piece[] = [];
  const shaft = (a: V3, b: V3, r: number, ends: [number, number], n = 12): Geo => tube(smoothPath([a, v.lerp(a, b, 0.5), b], n), (t) => { const q = r * (1 + ends[0] * Math.max(0, 1 - t / 0.12) ** 2 + ends[1] * Math.max(0, (t - 0.88) / 0.12) ** 2); return [q, q]; }, 10);
  const off = (a: V3, b: V3, d: number): V3 => { const ax = v.norm(v.sub(b, a)); return v.mul(v.norm(v.cross(ax, Math.abs(ax[1]) > 0.9 ? [1, 0, 0] : [0, 1, 0])), d); };
  for (const [side, s] of sides) {
    const L = side.toLowerCase(), arm = F.joint(`${side}Arm`), fore = F.joint(`${side}ForeArm`), hand = F.joint(`${side}Hand`);
    out.push(piece(F, `humerus-${L}`, "bone", `${side}Arm`, merge(ball(v.add(arm, [-s * 0.004 * k, 0.004 * k, -0.003 * k]), [0.023 * k, 0.023 * k, 0.023 * k]), shaft(arm, fore, 0.0105 * k, [0.6, 0.9])), COLORS.bone, 0.55));
    const o = off(fore, hand, 0.009 * k);
    out.push(piece(F, `radius-${L}`, "bone", `${side}ForeArm`, shaft(v.add(fore, o), v.add(hand, v.mul(o, 1.3)), 0.0068 * k, [0.4, 1.1]), COLORS.bone, 0.55));
    out.push(piece(F, `ulna-${L}`, "bone", `${side}ForeArm`, shaft(v.sub(fore, o), v.sub(hand, v.mul(o, 0.7)), 0.0062 * k, [1.0, 0.3]), COLORS.bone, 0.55));
    const hip = F.joint(`${side}Leg`), knee = F.joint(`${side}Shin`), ankle = F.joint(`${side}Foot`);
    const troch: V3 = [hip[0] + s * 0.045 * k, hip[1] - 0.02 * k, hip[2] - 0.004 * k];
    out.push(piece(F, `femur-${L}`, "bone", `${side}Leg`, merge(ball(hip, [0.024 * k, 0.024 * k, 0.024 * k]), tube([hip, troch], () => [0.014 * k, 0.012 * k], 10), shaft(troch, v.add(knee, [0, 0.012 * k, 0]), 0.0135 * k, [0.4, 1.0])), COLORS.bone, 0.55));
    out.push(piece(F, `patella-${L}`, "bone", `${side}Shin`, blob(v.add(knee, [0, 0.012 * k, 0.04 * k]), I3, [0.021 * k, 0.023 * k, 0.009 * k], () => 1, 16, 10), COLORS.bone, 0.55));
    out.push(piece(F, `tibia-${L}`, "bone", `${side}Shin`, merge(blob(v.add(knee, [0, -0.012 * k, 0]), I3, [0.036 * k, 0.012 * k, 0.028 * k], () => 1, 18, 10), shaft(v.add(knee, [0, -0.012 * k, 0]), v.add(ankle, [-s * 0.004 * k, 0.012 * k, 0]), 0.012 * k, [0.5, 0.6])), COLORS.bone, 0.55));
    out.push(piece(F, `fibula-${L}`, "bone", `${side}Shin`, shaft(v.add(knee, [s * 0.03 * k, -0.022 * k, -0.012 * k]), v.add(ankle, [s * 0.028 * k, -0.008 * k, -0.008 * k]), 0.006 * k, [0.6, 0.6]), COLORS.bone, 0.55));
  }
  return out;
}

export function bonesOf(skel: Parameters<typeof frameOf>[0], F: Frame = frameOf(skel)): Piece[] {
  return [...spine(F), ...ribs(F), ...girdle(F), ...pelvis(F), ...skull(F), ...limbs(F)];
}
