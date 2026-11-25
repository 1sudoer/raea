// A body's anatomy made from its skeleton (src/anatomy): its pieces, inside its skin, and a shot's path through them.
// The body here is made up: a SOMA man's joints (gb-sentry's, 1.75 m) and an oval trunk of skin samples round them.
import { expect, test } from "bun:test";
import { anatomyOf, hits } from "../src/anatomy";
import { rest, type Skeleton } from "../src/character/all";

type V3 = [number, number, number];
const JOINTS: [string, number, V3][] = [
  ["Hips", -1, [0, 0.998, -0.049]], ["Spine1", 0, [0, 1.049, -0.05]], ["Spine2", 1, [0, 1.121, -0.051]], ["Chest", 2, [0, 1.194, -0.058]],
  ["Neck1", 3, [0, 1.457, -0.059]], ["Neck2", 4, [0, 1.532, -0.035]], ["Head", 5, [0, 1.596, -0.016]], ["Jaw", 6, [0, 1.6, 0.0]],
  ["LeftEye", 6, [0.032, 1.66, 0.07]], ["RightEye", 6, [-0.032, 1.66, 0.07]],
  ["LeftShoulder", 3, [0.016, 1.428, -0.008]], ["LeftArm", 10, [0.161, 1.401, -0.063]], ["LeftForeArm", 11, [0.34, 1.18, -0.07]], ["LeftHand", 12, [0.47, 0.98, -0.05]],
  ["RightShoulder", 3, [-0.016, 1.428, -0.008]], ["RightArm", 14, [-0.161, 1.401, -0.063]], ["RightForeArm", 15, [-0.34, 1.18, -0.07]], ["RightHand", 16, [-0.47, 0.98, -0.05]],
  ["LeftLeg", 0, [0.1, 0.915, -0.027]], ["LeftShin", 18, [0.11, 0.5, -0.03]], ["LeftFoot", 19, [0.12, 0.08, -0.05]],
  ["RightLeg", 0, [-0.1, 0.915, -0.027]], ["RightShin", 21, [-0.11, 0.5, -0.03]], ["RightFoot", 22, [-0.12, 0.08, -0.05]],
];
// the trunk's skin: an oval 30 cm across and 21 cm deep (its middle 4 cm behind the hips' joint), from the hips to the neck
const FRONT = 0.065, BACK = -0.145, HALF = 0.15;
const surface: Skeleton["surface"] = [];
for (let y = 0.86; y <= 1.52; y += 0.012)
  for (let a = 0; a < 48; a++) {
    const t = (2 * Math.PI * a) / 48, x = HALF * Math.sin(t), z = Math.cos(t) >= 0 ? (FRONT + BACK) / 2 + ((FRONT - BACK) / 2) * Math.cos(t) : (FRONT + BACK) / 2 + ((FRONT - BACK) / 2) * Math.cos(t);
    surface.push({ part: y > 1.2 ? "chest" : y > 1.0 ? "torso" : "hips", side: "", j: [0], w: [1], o: [x, y - 0.998, z + 0.049], n: [Math.sin(t), 0, Math.cos(t)] });
  }
const skel = { name: "anatomy-test", height: 1.75, landmarks: { eyes: [0, 1.66, 0.07] }, surface, joints: JOINTS.map(([name, parent, head]) => ({ name, parent, head, tail: [head[0], head[1] + 0.02, head[2]] })) } as unknown as Skeleton;
const A = anatomyOf(skel);

test("it has the spine's 24 vertebrae, 24 ribs, the sternum, both lungs, the heart and the belly's organs", () => {
  const names = new Set(A.pieces.map((p) => p.name));
  expect(A.pieces.filter((p) => p.name.startsWith("vertebra-")).length).toBe(24);
  expect(A.pieces.filter((p) => /^rib-\d+-(left|right)$/.test(p.name)).length).toBe(24);
  for (const n of ["sternum", "lung-left", "lung-right", "heart", "aorta", "liver", "stomach", "kidney-left", "kidney-right", "skull", "femur-left"]) expect(names.has(n)).toBe(true);
});

test("the chest's pieces stay inside the trunk's skin", () => {
  const head = new Map(skel.joints.map((j) => [j.name, j.head as V3]));
  for (const p of A.pieces.filter((q) => /^(rib|lung|heart|liver|stomach|sternum|vertebra-T)/.test(q.name))) {
    const o = head.get(p.joint)!, P = p.geo.positions;
    for (let i = 0; i < P.length; i += 3) {
      const x = P[i] + o[0], z = P[i + 2] + o[2];
      const r = (x / HALF) ** 2 + ((z - (FRONT + BACK) / 2) / ((FRONT - BACK) / 2)) ** 2;
      if (r >= 1) throw new Error(`${p.name} reaches outside the skin at ${x.toFixed(3)}, ${(P[i + 1] + o[1]).toFixed(3)}, ${z.toFixed(3)}`);
    }
  }
});

test("a shot straight through the left chest goes in through the skin, then the heart and lung, in order", () => {
  const H = hits(skel, rest(skel), A, [0.06, 1.3, 5], [0, 0, -1]);
  expect(H[0].part).toBe("skin");
  expect(H[0].entry[2]).toBeCloseTo(FRONT, 1);
  const parts = H.map((h) => h.part);
  expect(parts.some((p) => p === "heart" || p === "lung-left")).toBe(true);
  for (let i = 1; i < H.length; i++) expect(H[i].t0).toBeGreaterThanOrEqual(H[i - 1].t0);
  for (const h of H) expect(h.t1).toBeGreaterThan(h.t0);
});

test("the shot finds the same pieces when the body is moved and turned, the shot with it", () => {
  const base = hits(skel, rest(skel), A, [0.06, 1.3, 5], [0, 0, -1]).map((h) => h.part);
  // the body turned a quarter left and moved: its front faces +x, so the same shot comes from +x
  const at = { position: [3, 0, -2] as V3, yaw: Math.PI / 2 };
  const turned = hits(skel, rest(skel), A, [3 + 5, 1.3, -2 - 0.06], [-1, 0, 0], at).map((h) => h.part);
  expect(turned).toEqual(base);
});
