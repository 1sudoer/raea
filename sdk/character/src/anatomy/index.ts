// A body's anatomy for an X-ray and a shot's path through it (docs/character.feature CH50): its skeleton as bones and
// its chest and belly's organs, made from the body's own skeleton.json (its joints and its skin's samples) so they fit
// any registered SOMA body, each piece riding its joint as the body is posed. All of it is made here in code: no
// anatomy data is downloaded (the open atlases are share-alike).
//
//   const A = anatomyOf(skel);                       // once per body
//   hits(skel, pose, A, muzzle, dir, at)             // what a shot passes through, in order: [{ part, entry, exit }]
//   <Anatomy skel={skel} pose={pose} anatomy={A} name="sentry" xray={1} />
import type { Skeleton } from "../character/all";
import { bonesOf } from "./bones";
import { frameOf, type Frame } from "./frame";
import { organsOf } from "./organs";
import type { Piece } from "./piece";

export interface AnatomyOf {
  body: string;
  frame: Frame;
  pieces: Piece[];
}

const made = new WeakMap<Skeleton, AnatomyOf>();

/** the body's anatomy (made once per skeleton): its bones and organs, each a mesh on the joint it rides */
export function anatomyOf(skel: Skeleton): AnatomyOf {
  let a = made.get(skel);
  if (!a) {
    const F = frameOf(skel);
    made.set(skel, (a = { body: skel.name, frame: F, pieces: [...bonesOf(skel, F), ...organsOf(skel, F)] }));
  }
  return a;
}

export { hits, type Hit } from "./hits";
export { Anatomy, Round, ShotPath, Wounds, xrayGhost, xrayParts, type AnatomyProps } from "./draw";
export type { Piece, Kind } from "./piece";
export type { Frame } from "./frame";
