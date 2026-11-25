// The eyes' look target (src/character/eyes.ts, docs/character.feature CH20c): the eye joints turned to a point within
// an eye's range, on top of the head's turn, and the lids riding the eyes down.
import { expect, test } from "bun:test";
import { EYE_RANGE, eyesTo, lidsFor, pose, qaxis, qrot, type Pose, type Skeleton } from "../src/character/all";

const J = (name: string, parent: number, head: [number, number, number]) => ({ name, parent, head, tail: [head[0], head[1] + 0.02, head[2]] as [number, number, number] });
const skel = { name: "t", height: 1.7, joints: [J("Hips", -1, [0, 1, 0]), J("Head", 0, [0, 1.6, 0]), J("LeftEye", 1, [0.03, 1.65, 0.06]), J("RightEye", 1, [-0.03, 1.65, 0.06])] } as unknown as Skeleton;
const rest: Pose = { root: [0, 1, 0], turns: skel.joints.map(() => [0, 0, 0, 1]) };
const ahead = (p: Pose, i: number) => qrot(pose(skel, p).rot[i], [0, 0, 1]);
const angle = (a: number[], b: number[]) => Math.acos(Math.min(1, a[0] * b[0] + a[1] * b[1] + a[2] * b[2]));

test("each eye points at a target within its range", () => {
  const t: [number, number, number] = [0.4, 1.75, 1.5], p = eyesTo(skel, rest, t);
  for (const i of [2, 3]) {
    const e = skel.joints[i].head, d = [t[0] - e[0], t[1] - e[1], t[2] - e[2]], l = Math.hypot(...d);
    expect(angle(ahead(p, i), d.map((v) => v / l))).toBeLessThan(1e-3);
  }
});

test("past its range an eye stops: 35 degrees to a side, 17 up, 25 down", () => {
  const side = ahead(eyesTo(skel, rest, [9, 1.65, 0.06]), 2), up = ahead(eyesTo(skel, rest, [0.03, 9, 0.5]), 2), down = ahead(eyesTo(skel, rest, [0.03, -9, 0.5]), 2);
  expect(Math.atan2(side[0], side[2])).toBeCloseTo(EYE_RANGE.side, 4);
  expect(Math.asin(up[1])).toBeCloseTo(EYE_RANGE.up, 4);
  expect(Math.asin(-down[1])).toBeCloseTo(EYE_RANGE.down, 4);
});

test("the eyes cover what the head has not turned", () => {
  // the head turned 20 degrees to its left, the target straight ahead of the body: the eyes turn back toward it
  const turned: Pose = { root: rest.root, turns: [rest.turns[0], qaxis([0, 1, 0], 0.35), rest.turns[2], rest.turns[3]] };
  const w = pose(skel, turned), t: [number, number, number] = [0, 1.65, 3], p = eyesTo(skel, turned, t);
  const d = [t[0] - w.pos[2][0], t[1] - w.pos[2][1], t[2] - w.pos[2][2]], l = Math.hypot(...d);
  expect(angle(ahead(p, 2), d.map((v) => v / l))).toBeLessThan(1e-3);
});

test("the lids come down with the eyes, and stay up otherwise", () => {
  expect(lidsFor(skel, rest)).toBe(0);
  expect(lidsFor(skel, eyesTo(skel, rest, [0.03, 9, 0.5]))).toBe(0);
  expect(lidsFor(skel, eyesTo(skel, rest, [0.03, -9, 0.5]))).toBeCloseTo(0.45, 3);
  expect(eyesTo({ ...skel, joints: skel.joints.slice(0, 2) } as Skeleton, rest, [0, 0, 1])).toBe(rest);
});
