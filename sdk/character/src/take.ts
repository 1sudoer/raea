// @raea/character/take: a character takes a thing with its hands, with no capture of the action (docs/hands.feature):
// it reaches, grabs with one hand or two, lifts, carries, and puts the thing down or throws it. The engine works each
// frame out (raea-sys-grab, the native call character.take); this asks it and hands back the pose to give
// <Character> and where to draw the thing.
//
//   const box = { box: [0.16, 0.1, 0.12], at: [0, 1.05, 0.42] } as const;          // half sizes, metres, the body's frame
//   const now = take(kid, () => rest(kid), { thing: box, start: 0.5, throw: { at: 2.5, velocity: [0, 2, 3] } }, frame / fps);
//   <Character body="kid" pose={now.pose} />
//   <Mesh3d transform={Transform.fromXyz(...now.thing.at).withRotation(now.thing.turn)}>...</Mesh3d>
//
// `base` is the body's own motion, its pose at any time (a clip, a walk, a stand): the take only moves what the hands
// need and says how much (`moved`, degrees) and how far a wrist ended from its hold (`miss`, metres). A thing out of
// reach is missed and said so: `stand` is where to walk the body to first. A low thing (on the floor) is taken by
// bending and squatting, the feet staying where they stand. Frame-pure: any frame can be asked for.
import { native } from "@raea/app/native";
import type { Pose, Quat, Skeleton } from "./character/core";
import { qmul, slerp } from "./character/vectors";

type Vec3 = [number, number, number];

/** A thing to take: one shape (a ball's radius; a box's half sizes; a capsule's or a cylinder's half length along its
 * own y, then radius), where its centre is and how it is turned, in the frame the body is posed in. */
export interface TakeThing {
  ball?: number;
  box?: Vec3;
  capsule?: [number, number];
  cylinder?: [number, number];
  at: Vec3;
  turn?: Quat;
}

export interface TakeSpec {
  thing: TakeThing;
  /** when the hands leave the body's own motion, seconds */
  start: number;
  /** when they close on the thing (0.6 s after start) and when it has reached the carry (0.5 s after that) */
  grab?: number;
  carried?: number;
  /** where the thing rides, from the chest: x to the body's left, y up, z the way it faces */
  carry?: Vec3;
  /** the hand that takes a thing small enough for one (the right) */
  hand?: "L" | "R";
  /** how far below the thing's middle the hands go, metres */
  lower?: number;
  /** a thing held one way brings its holds, in its own frame: where a palm goes (`at`, on its surface), the way the
   * surface faces there (`normal`), optionally the way the fingers point (`along`) and the hand's shape as
   * handShape's first numbers (`shape`: about -1.2 is a loose fist round a handle). None: holds are found on its shape */
  holds?: { side: "L" | "R"; at: Vec3; normal: Vec3; along?: Vec3; shape?: number[] }[];
  /** what is done with the thing once carried: where it is at times (`carry`, from the chest as above) and how it
   * is tipped from how it is carried (`tilt`, in the body's axes); the hands follow. A drink is a key at the mouth */
  path?: { t: number; carry: Vec3; tilt?: Quat }[];
  /** how heavy it is, kilograms: held lower and closer, lifted slower, the trunk leaning back against it */
  weight?: number;
  /** when a thing held in one hand is passed to the other, seconds */
  pass?: number;
  /** a thing that isn't carried but moved where it is (a drawer, a door, a lever): where it is at times after the
   * grab, in the frame the body is posed in; the hands stay on it */
  slide?: { t: number; at: Vec3; turn?: Quat }[];
  /** when the hands leave the thing where it is (after a slide), seconds */
  leave?: number;
  /** the scene's solids the hands keep out of (a table's top, a shelf) */
  avoid?: TakeThing[];
  /** set down: moved to `place` from `at`, standing there from `down` */
  put?: { at: number; down: number; place: Vec3 };
  /** let go at `at` with a velocity, metres a second; it falls to `floor` (0) */
  throw?: { at: number; velocity: Vec3; floor?: number };
}

export interface Taken {
  pose: Pose;
  thing: { at: Vec3; turn: Quat };
  phase: "Before" | "Reach" | "Lift" | "Carry" | "Release" | "After";
  hands: 1 | 2;
  /** the worst a wrist ended from where its hold asks, metres */
  miss: number;
  /** the most any joint was turned from the body's own motion, degrees */
  moved: number;
  /** a wrist was held at the most it bends (70 degrees), so its palm isn't square on its hold */
  limited: boolean;
  /** where the body would stand to take the thing, and the turn about the vertical that faces it: walk it there first
   * when the thing is out of reach */
  stand: { at: Vec3; yaw: number };
  /** whether the body can take the thing from where it stands when it starts to reach, and if not how far it has to
   * move (to `stand`), metres */
  reach: { ok: boolean; move: number };
}

/** The body and the thing at `t` seconds of a take. */
export function take(skel: Skeleton, base: (t: number) => Pose, spec: TakeSpec, t: number): Taken {
  const grab = spec.grab ?? spec.start + 0.6;
  // the moments the take reads the body's own motion at: now, the start, the grab, the release
  const times = [t, spec.start, grab, spec.put?.at, spec.put?.down, spec.throw?.at].filter((x): x is number => x !== undefined);
  const poses = [...new Set(times)].map((at) => {
    const p = base(at);
    return { t: at, root: p.root, turns: skel.joints.map((_, i) => p.turns[i] ?? ([0, 0, 0, 1] as Quat)) };
  });
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  return native("character.take", { ...spec, joints, t, base: poses });
}

/** A point of the body seen somewhere: on a joint (by name), `offset` from its head in the rest pose's axes (none: the
 * joint itself), seen at `at`. */
export interface Mark {
  joint: string;
  offset?: Vec3;
  at: Vec3;
  weight?: number;
}

/** A pose fitted to where points of the body were seen, so motion that arrives as positions (a capture, a tracker,
 * another rig's joints) plays on this body; `miss` is the marks' root-mean-square distance from where they were seen,
 * metres. It starts from `from` (the rest pose), and a turn the marks say nothing about stays as `from` has it. */
export function fitPose(skel: Skeleton, marks: Mark[], from?: Pose, passes?: number): { pose: Pose; miss: number } {
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  const start = from && { root: from.root, turns: skel.joints.map((_, i) => from.turns[i] ?? ([0, 0, 0, 1] as Quat)) };
  return native("character.fit", { joints, marks, from: start, passes });
}

/** A hand shaped by a model of real hand poses (SOMA-X's hand prior): `z` are its numbers, each in its own standard
 * deviations (none: the average relaxed hand; the first opens the hand when positive and closes it when negative).
 * The rest of `from` (the rest pose) is kept. `odd` says how far the hand is from the hands people make (0: the
 * average hand; the rig's own rest hand reads about 2). With no `z`, `from`'s own hand is read. */
export function handShape(skel: Skeleton, side: "L" | "R", z?: number[], from?: Pose): { pose: Pose; z: number[]; odd: number } {
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  const pose = from && { root: from.root, turns: skel.joints.map((_, i) => from.turns[i] ?? ([0, 0, 0, 1] as Quat)) };
  return native("character.hand", { joints, side, z, pose });
}

/** A pose turned about the vertical by `yaw` (about the origin, radians) and moved by `by`: the same body stood
 * somewhere else, facing another way. take() works wherever the body is. */
export function placedPose(p: Pose, yaw: number, by: Vec3 = [0, 0, 0]): Pose {
  const c = Math.cos(yaw), s = Math.sin(yaw), [x, y, z] = p.root;
  const turn: Quat = [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)];
  return { root: [c * x + s * z + by[0], y + by[1], c * z - s * x + by[2]], turns: [qmul(turn, p.turns[0] ?? [0, 0, 0, 1]), ...p.turns.slice(1)] };
}

/** The body's own motion for walking up to a place: a walk clip (poses carrying their own root motion, `fps` a
 * second) played from its start for `seconds`, set down so it ends at `to` (x and z) walking the way `yaw` faces,
 * then eased over `settle` seconds into `stand` there. Give take() `at` as its base, a take's `stand` as `to` and
 * `yaw`, and start the take once it `arrives`. The feet slide a little while it settles: a clip that stops would
 * not. */
export function walkUp(clip: Pose[], fps: number, seconds: number, stand: Pose, to: Vec3, yaw: number, settle = 0.5): { at: (t: number) => Pose; arrives: number } {
  const last = Math.min(clip.length - 1, Math.round(seconds * fps)), a = clip[Math.max(0, last - 4)].root, b = clip[last].root;
  // the way the clip is walking as it ends, and the turn that makes that `yaw`
  const by = yaw - Math.atan2(b[0] - a[0], b[2] - a[2]), end = placedPose(clip[last], by);
  const shift: Vec3 = [to[0] - end.root[0], 0, to[2] - end.root[2]];
  const there = placedPose({ root: [0, stand.root[1], 0], turns: stand.turns }, yaw, [to[0], 0, to[2]]);
  const at = (t: number): Pose => {
    const f = Math.max(0, Math.min(last, t * fps)), i = Math.floor(f), u = f - i, p = clip[i], q = clip[Math.min(last, i + 1)];
    const walking = placedPose({ root: [p.root[0] + (q.root[0] - p.root[0]) * u, p.root[1] + (q.root[1] - p.root[1]) * u, p.root[2] + (q.root[2] - p.root[2]) * u], turns: p.turns.map((turn, j) => slerp(turn, q.turns[j] ?? turn, u)) }, by, shift);
    const k = Math.max(0, Math.min(1, (t - last / fps) / settle)), e = k * k * (3 - 2 * k);
    if (e <= 0) return walking;
    return { root: [walking.root[0] + (there.root[0] - walking.root[0]) * e, walking.root[1] + (there.root[1] - walking.root[1]) * e, walking.root[2] + (there.root[2] - walking.root[2]) * e], turns: walking.turns.map((turn, j) => slerp(turn, there.turns[j] ?? turn, e)) };
  };
  return { at, arrives: last / fps + settle };
}

/** Fetching: the body takes a thing, walking up to it first when it is out of reach. `stand` is the body's standing
 * pose where it is; `walk` a walk clip (poses carrying their own root motion) and its rate. When the thing is in
 * reach the take runs as given; when it is not, the body walks to where the take says to stand (walkUp), and every
 * time of the take is put off until it arrives. The answer is the take's, with `arrives` (0 when it didn't walk). */
export function fetch(skel: Skeleton, stand: Pose, walk: { clip: Pose[]; fps: number; seconds?: number }, spec: TakeSpec, t: number): Taken & { arrives: number } {
  const first = take(skel, () => stand, spec, spec.start);
  if (first.reach.ok) return { ...take(skel, () => stand, spec, t), arrives: 0 };
  const up = walkUp(walk.clip, walk.fps, walk.seconds ?? Math.max(1, first.reach.move / 1.2), { root: [0, stand.root[1], 0], turns: stand.turns }, first.stand.at, first.stand.yaw);
  const later = (x: number | undefined) => (x === undefined ? undefined : x + up.arrives);
  const put = spec.put && { ...spec.put, at: spec.put.at + up.arrives, down: spec.put.down + up.arrives };
  const thrown = spec.throw && { ...spec.throw, at: spec.throw.at + up.arrives };
  const path = spec.path?.map((k) => ({ ...k, t: k.t + up.arrives }));
  const slide = spec.slide?.map((k) => ({ ...k, t: k.t + up.arrives }));
  const moved: TakeSpec = { ...spec, start: spec.start + up.arrives, grab: later(spec.grab), carried: later(spec.carried), pass: later(spec.pass), leave: later(spec.leave), put, throw: thrown, path, slide };
  return { ...take(skel, up.at, moved, t), arrives: up.arrives };
}
