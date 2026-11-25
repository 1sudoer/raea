// The eyes' look target (docs/character.feature CH20c): each eye joint turned to a point, within what an eye does,
// on top of whatever the body's motion has done with the neck and head. A head part's eyes are skinned to SOMA's
// LeftEye and RightEye joints (a creature's eye_l and eye_r), which no clip turns: this is what turns them.
import { I, norm, qaxis, qinv, qmul, qrot, sub, pose, type Pose, type Quat, type Skeleton, type Vec3 } from "./all";

/** how far an eye turns from straight ahead in its head, radians: to a side, up and down. (Up is the least: a head's
 * upper lid has no morph that lifts it, and at 25 degrees up the iris went under it.) */
export const EYE_RANGE = { side: 0.61, up: 0.3, down: 0.44 };
const EYES = [["LeftEye", "RightEye"], ["eye_l", "eye_r"]];

/** A pose with its eyes turned to `target` (a point in the body's frame), each no further than `range` from straight
 * ahead in the head: past that the eye stops at its limit, and the rest is the head's to turn. A body with no eye
 * joints comes back as it was. */
export function eyesTo(skel: Skeleton, p: Pose, target: Vec3, range = EYE_RANGE): Pose {
  const idx = (n: string) => skel.joints.findIndex((j) => j.name === n);
  const eyes = (EYES.find((pair) => pair.every((n) => idx(n) >= 0)) ?? []).map(idx);
  if (!eyes.length) return p;
  const turns: Quat[] = skel.joints.map((_, j) => p.turns[j] ?? I);
  const w = pose(skel, { root: p.root, turns });
  const clamp = (v: number, lim: number) => Math.max(-lim, Math.min(lim, v));
  for (const i of eyes) {
    // the way to the target in the head's own frame (the eye's parent): at rest an eye looks along +z
    const d = qrot(qinv(w.rot[skel.joints[i].parent]), norm(sub(target, w.pos[i])));
    const side = clamp(Math.atan2(d[0], d[2]), range.side), up = Math.max(-range.down, Math.min(range.up, Math.atan2(d[1], Math.hypot(d[0], d[2]))));
    turns[i] = qmul(qaxis([0, 1, 0], side), qaxis([1, 0, 0], -up));
  }
  return { root: p.root, turns };
}

/** How far the upper lids come down with the eyes in a pose (0 to 1 of a blink): a lid rides its eye, so eyes
 * turned all the way down lower it to `most` of shut, and eyes level or raised leave it open. For the head's
 * blink_left and blink_right morphs, added to whatever blink it has. */
export function lidsFor(skel: Skeleton, p: Pose, most = 0.45, range = EYE_RANGE): number {
  const i = skel.joints.findIndex((j) => j.name === "LeftEye" || j.name === "eye_l"), q = i >= 0 ? p.turns[i] : undefined;
  if (!q) return 0;
  // how far below level the eye's own +z points, in its head's frame
  const down = Math.asin(Math.max(-1, Math.min(1, 2 * (q[3] * q[0] - q[1] * q[2]))));
  return Math.max(0, Math.min(1, down / range.down)) * most;
}
