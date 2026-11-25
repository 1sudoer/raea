// The joints' names: SOMA's (NVlabs/SOMA-X, Apache-2.0), raea's one character skeleton (docs/character.feature CH1,
// CH12). Every part of the SDK names joints through these, never with string literals.
//
// Bodies made before SOMA name their joints as MakeHuman's game_engine rig does (Unreal's mannequin names: pelvis,
// thigh_l, index_02_r). They still play: `joint` finds a SOMA name on such a body by its old name, and the helpers
// below read a name in either form. `oldName` and `somaName` turn one into the other (raea-sys-body rig.rs `rename`
// is the same table). SOMA has a second neck joint (Neck2) and a bone in the palm for each finger (Index1..Pinky1),
// which an old body has none for: they ride their parents.

export type Side = "L" | "R";

export const SIDES: readonly Side[] = ["L", "R"];

/** the prefix a side's joints carry */
const px = (s: Side) => (s === "L" ? "Left" : "Right");

export const FINGERS = ["thumb", "index", "middle", "ring", "pinky"] as const;

export type Finger = (typeof FINGERS)[number];

const cap = (f: string) => f[0].toUpperCase() + f.slice(1);

/** Joint names, by role. A body's joints are the trunk, then each arm and leg; the fingers come after all of them. */
export const B = {
  pelvis: "Hips",
  /** the spine from the hips up: Spine1 (the waist), Spine2, Chest (which the shoulders and neck hang from) */
  spine: ["Spine1", "Spine2", "Chest"] as const,
  chest: "Chest",
  /** the neck's first joint (SOMA's second, Neck2, rides it unless something turns it) */
  neck: "Neck1",
  head: "Head",
  clavicle: (s: Side) => `${px(s)}Shoulder`,
  upperarm: (s: Side) => `${px(s)}Arm`,
  lowerarm: (s: Side) => `${px(s)}ForeArm`,
  hand: (s: Side) => `${px(s)}Hand`,
  thigh: (s: Side) => `${px(s)}Leg`,
  calf: (s: Side) => `${px(s)}Shin`,
  foot: (s: Side) => `${px(s)}Foot`,
  /** the ball of the foot, where the toes bend */
  ball: (s: Side) => `${px(s)}ToeBase`,
  /** a finger's phalanx k (1 at the knuckle, 3 at the tip; the thumb's 1 is its metacarpal): SOMA's 2 to 4 for a
   * finger (its 1 is the bone in the palm), 1 to 3 for the thumb */
  finger: (f: Finger, k: 1 | 2 | 3, s: Side) => `${px(s)}Hand${cap(f)}${f === "thumb" ? k : k + 1}`,
  arm: (s: Side) => [B.upperarm(s), B.lowerarm(s), B.hand(s)],
  leg: (s: Side) => [B.thigh(s), B.calf(s), B.foot(s)],
  /** both feet's ankles then both balls: L foot, R foot, L ball, R ball */
  feet: () => [B.foot("L"), B.foot("R"), B.ball("L"), B.ball("R")],
};

/** The body's joints in skeleton order (parents first): what a clip turns. SOMA's 23; an old body has 22 (no Neck2). */
export const BODY_JOINTS: readonly string[] = [
  B.pelvis, ...B.spine, B.neck, "Neck2", B.head,
  ...SIDES.flatMap((s) => [B.clavicle(s), B.upperarm(s), B.lowerarm(s), B.hand(s)]),
  ...SIDES.flatMap((s) => [B.thigh(s), B.calf(s), B.foot(s), B.ball(s)]),
];

/** The fingers' joints a hand pose turns: each side's thumb, index, middle, ring, pinky, 1 to 3. */
export const FINGER_JOINTS: readonly string[] = SIDES.flatMap((s) => FINGERS.flatMap((f) => ([1, 2, 3] as const).map((k) => B.finger(f, k, s))));

// ---------------------------------------------------------------- the old names

const TRUNK: [string, string][] = [["pelvis", "Hips"], ["spine_01", "Spine1"], ["spine_02", "Spine2"], ["spine_03", "Chest"], ["neck_01", "Neck1"], ["head", "Head"]];
const LIMB: [string, string][] = [["clavicle", "Shoulder"], ["upperarm", "Arm"], ["lowerarm", "ForeArm"], ["hand", "Hand"], ["thigh", "Leg"], ["calf", "Shin"], ["foot", "Foot"], ["ball", "ToeBase"]];

/** a game_engine name ("thigh_l", "index_02_r", "spine_03") as SOMA names that joint; any other name as it is */
export function somaName(name: string): string {
  const t = TRUNK.find((p) => p[0] === name);
  if (t) return t[1];
  const m = /^(.+)_([lr])$/.exec(name);
  if (!m) return name;
  const side = m[2] === "l" ? "Left" : "Right", l = LIMB.find((p) => p[0] === m[1]);
  if (l) return side + l[1];
  const f = /^(thumb|index|middle|ring|pinky)_0([123])$/.exec(m[1]);
  return f ? `${side}Hand${cap(f[1])}${Number(f[2]) + (f[1] === "thumb" ? 0 : 1)}` : name;
}

/** a SOMA name as the game_engine rig names that joint, where it has one; any other name as it is */
export function oldName(name: string): string {
  const t = TRUNK.find((p) => p[1] === name);
  if (t) return t[0];
  const m = /^(Left|Right)(.+)$/.exec(name);
  if (!m) return name;
  const sx = m[1] === "Left" ? "l" : "r", l = LIMB.find((p) => p[1] === m[2]);
  if (l) return `${l[0]}_${sx}`;
  const f = /^Hand(Thumb|Index|Middle|Ring|Pinky)([1-4])$/.exec(m[2]);
  if (!f) return name;
  const k = Number(f[2]) - (f[1] === "Thumb" ? 0 : 1);
  return k >= 1 && k <= 3 ? `${f[1].toLowerCase()}_0${k}_${sx}` : name;
}

/** whether two names are one joint, either written either way */
export const sameJoint = (a: string, b: string): boolean => a === b || somaName(a) === somaName(b);

/** a joint's side from its name ("" on the midline) */
export function sideOf(name: string): Side | "" {
  return name.startsWith("Left") || name.endsWith("_l") ? "L" : name.startsWith("Right") || name.endsWith("_r") ? "R" : "";
}

/** a joint's name on the other side (itself on the midline) */
export function mirrorName(name: string): string {
  if (name.startsWith("Left")) return "Right" + name.slice(4);
  if (name.startsWith("Right")) return "Left" + name.slice(5);
  return name.endsWith("_l") ? name.slice(0, -1) + "r" : name.endsWith("_r") ? name.slice(0, -1) + "l" : name;
}

/** a joint's role, its name without the side: "Leg", "HandIndex3", "Spine1" (an old name's: "thigh", "index_02") */
export function roleOf(name: string): string {
  return name.replace(/^(Left|Right)/, "").replace(/_[lr]$/, "");
}

/** whether a joint is a finger's (either naming; the bone in the palm and the tip's end joint count) */
export const isFinger = (name: string): boolean => /^(Left|Right)Hand(Thumb|Index|Middle|Ring|Pinky)/.test(name) || FINGERS.some((f) => name.startsWith(f + "_"));
