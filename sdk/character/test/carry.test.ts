// Something carried on a joint (src/character/drawing.tsx onJoint): placed against the joint at rest, carried by the
// joint's turn from rest, and put in the scene by the body's placement, as a cap rides Head.
import { expect, test } from "bun:test";
import { onJoint, pose, qaxis, qmul, qrot, type Pose, type Quat, type Skeleton } from "../src/character/all";

const J = (name: string, parent: number, head: [number, number, number]) => ({ name, parent, head, tail: [head[0], head[1] + 0.02, head[2]] as [number, number, number] });
const skel = { name: "t", height: 1.7, joints: [J("Hips", -1, [0, 1, 0]), J("Chest", 0, [0, 1.3, 0]), J("RightShoulder", 1, [-0.05, 1.45, 0])] } as unknown as Skeleton;
const still: Pose = { root: [0, 1, 0], turns: skel.joints.map(() => [0, 0, 0, 1] as Quat) };
const near = (a: number[], b: number[], eps = 1e-6) => a.forEach((v, i) => expect(Math.abs(v - b[i])).toBeLessThan(eps));
const at: [number, number, number] = [-0.15, -0.3, -0.15];

test("at rest the thing is the joint's place plus its offset, unturned", () => {
  const c = onJoint(skel, still, "RightShoulder", at);
  near(c.position, [-0.2, 1.15, -0.15]);
  near(c.quat, [0, 0, 0, 1]);
});

test("the joint's turn carries the thing round the joint and turns it with it", () => {
  const chest = qaxis([0, 1, 0], Math.PI / 2), own = qaxis([1, 0, 0], 0.3);
  const turned: Pose = { ...still, turns: [[0, 0, 0, 1], chest, [0, 0, 0, 1]] };
  const c = onJoint(skel, turned, "RightShoulder", at, own);
  const w = pose(skel, turned), sh = w.pos[2];
  // the offset turned as the joint turned, from the joint's new place
  near(c.position, sh.map((v, i) => v + qrot(chest, at)[i]));
  near(c.quat, qmul(chest, own));
  expect(Math.hypot(c.position[0] - sh[0], c.position[1] - sh[1], c.position[2] - sh[2])).toBeCloseTo(Math.hypot(...at), 6);
});

test("the body's placement moves and turns the carried thing with the body", () => {
  const c = onJoint(skel, still, "RightShoulder", at, [0, 0, 0, 1], { position: [2, 0, 1], yaw: Math.PI / 2 });
  // the body turned a quarter about up: its -x (its right) now points along +z
  near(c.position, [2 + -0.15, 1.15, 1 + 0.2]);
  near(c.quat, qaxis([0, 1, 0], Math.PI / 2));
});

test("a joint the body hasn't is an error that names it", () => {
  expect(() => onJoint(skel, still, "LeftHand", at)).toThrow(/no joint LeftHand/);
});
