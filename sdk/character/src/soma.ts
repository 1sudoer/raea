// @raea/character/soma: motion on SOMA's skeleton played on a raea body. SOMA (NVlabs/SOMA-X, Apache-2.0) is the
// 78-joint skeleton NVIDIA's motion models and datasets speak (Kimodo, GEM-X, BONES-SEED). A SOMA animation saved
// as text USD (a SkelAnimation: joints, and each frame's turns and offsets) is read into where every joint is at
// every frame; somaPose then points the body's bones the way SOMA's point at a frame (the native call
// character.soma), whatever the two bodies' proportions.
//
//   import walk from "./assets/clips/soma-walk.usda?raw";
//   const clip = readSomaUsda(walk);
//   <Character body="kid" pose={somaPose(kid, clip, frame * (clip.fps / fps))} />
import { native } from "@raea/app/native";
import type { Pose, Quat, Skeleton } from "./character/core";
import { qmul, qrot } from "./character/vectors";

type Vec3 = [number, number, number];

export interface SomaClip {
  fps: number;
  /** SOMA's joints by name (the last part of each joint's path) */
  names: string[];
  /** each frame: where every joint is, metres, y up */
  frames: Vec3[][];
}

const numbers = (text: string): number[] => (text.match(/-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/gi) ?? []).map(Number);

/** one attribute's time samples: each frame's flat list of numbers, in frame order */
function samples(usda: string, attribute: string): number[][] {
  const at = usda.indexOf(`${attribute}.timeSamples`);
  if (at < 0) throw new Error(`SOMA animation: no ${attribute}`);
  const open = usda.indexOf("{", at), close = usda.indexOf("}", open), out: [number, number[]][] = [];
  for (const line of usda.slice(open + 1, close).split("\n")) {
    const colon = line.indexOf(":");
    if (colon > 0) out.push([Number(line.slice(0, colon)), numbers(line.slice(colon + 1))]);
  }
  return out.sort((a, b) => a[0] - b[0]).map((s) => s[1]);
}

/** A SOMA animation as text USD read: forward kinematics over its joints, into where each is at each frame. */
export function readSomaUsda(usda: string): SomaClip {
  const paths = [...(/uniform token\[\] joints = \[([^\]]*)\]/.exec(usda)?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  if (!paths.length) throw new Error("SOMA animation: no joints");
  const parent = paths.map((p) => paths.indexOf(p.slice(0, p.lastIndexOf("/"))));
  const unit = Number(/metersPerUnit = ([\d.e-]+)/.exec(usda)?.[1] ?? 0.01), fps = Number(/framesPerSecond = ([\d.]+)/.exec(usda)?.[1] ?? 24);
  const turns = samples(usda, "rotations"), offsets = samples(usda, "translations");
  const frames = turns.map((r, f) => {
    const t = offsets[Math.min(f, offsets.length - 1)], pos: Vec3[] = [], rot: Quat[] = [];
    paths.forEach((_, j) => {
      // (USD writes a quaternion w first)
      const local: Quat = [r[j * 4 + 1], r[j * 4 + 2], r[j * 4 + 3], r[j * 4]], off: Vec3 = [t[j * 3] * unit, t[j * 3 + 1] * unit, t[j * 3 + 2] * unit], p = parent[j];
      if (p < 0) { pos.push(off); rot.push(local); return; }
      const moved = qrot(rot[p], off);
      pos.push([pos[p][0] + moved[0], pos[p][1] + moved[1], pos[p][2] + moved[2]]);
      rot.push(qmul(rot[p], local));
    });
    return pos;
  });
  return { fps, names: paths.map((p) => p.slice(p.lastIndexOf("/") + 1)), frames };
}

/** The body posed as a SOMA animation has its body at a frame (a whole frame; the nearest is taken). */
export function somaPose(skel: Skeleton, clip: SomaClip, frame: number): Pose {
  const at = clip.frames[Math.max(0, Math.min(clip.frames.length - 1, Math.round(frame)))];
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  return native("character.soma", { joints, names: clip.names, at, eyes: skel.landmarks?.eyes }).pose;
}

/** A motion on SOMA's skeleton given as turns, as Kimodo writes one: per frame where the hips are (metres) and each
 * named joint's world turn from SOMA's T-pose. */
export interface TurnsClip {
  fps: number;
  names: string[];
  root: Vec3[];
  turns: Quat[][];
}

/** A SOMA body (a skeleton whose rig is "soma", with each joint's turn from bind to the T-pose in `tpose`) posed by
 * a motion's turns at a frame, its twist joints set (the native call character.turns): nothing is fitted, since the
 * body and the motion share a skeleton. */
export function turnsPose(skel: Skeleton & { tpose?: Record<string, Quat> }, clip: TurnsClip, frame: number): Pose {
  const f = Math.max(0, Math.min(clip.turns.length - 1, Math.round(frame)));
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  return native("character.turns", { joints, tpose: skel.tpose ?? {}, names: clip.names, turns: clip.turns[f], root: clip.root[f] }).pose;
}

/** A SOMA body's pose with its twist joints set from its limbs' turns (the native call character.twists): for a
 * pose from a clip or a solver, which turn the named joints only. A body without twist joints comes back as it is. */
export function withTwists(skel: Skeleton, pose: Pose): Pose {
  const joints = skel.joints.map((j) => ({ name: j.name, parent: j.parent, head: j.head, tail: j.tail }));
  return native("character.twists", { joints, pose }).pose;
}
