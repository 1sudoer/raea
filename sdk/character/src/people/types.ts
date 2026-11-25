// The people of a place, as data: who they are (a role, a rank, a trade, a face, a body, what they wear and carry), what
// they are like (traits, habits) and what they do each hour (a day of activities and the trips between them). A
// project gives the roles and places; the SDK makes the people and their days, the same from the same seed.

/** Minutes after midnight (0 to 1439); a day is one calendar day. */
export type Minute = number;
export type V3 = [number, number, number];
export type Sex = "f" | "m";

/** How a person gets about. */
export type Mode = "walk" | "tram" | "bus" | "cab" | "cart" | "bicycle" | "car" | "train";

/** What a person is like, each 0 to 1 (0.5 is ordinary): the five broad traits, which the SDK reads for timing
 *  (conscientious people keep to their times), company (extraverts stop to talk), nerves (neurotic people flinch and
 *  hurry) and manners (agreeable people give way). */
export interface Traits {
  openness: number;
  conscientiousness: number;
  extraversion: number;
  agreeableness: number;
  neuroticism: number;
}

/** How a person is, each 0 to 1, changing through the day: what moves gait, pace and reactions. */
export interface Condition {
  hunger: number;
  cold: number;
  fatigue: number;
  fear: number;
}

/** A thing a person does by habit: in a window of the day, at a kind of place (or wherever they are), with a chance a
 *  day, for some minutes, as an act a body can perform ("smokes a pipe on the doorstep", "reads the paper"). */
export interface Habit {
  id: string;
  /** what the body does: a stage act's do line, or a clip's situation */
  act: string;
  /** when it may happen */
  from: Minute;
  to: Minute;
  /** where: a place kind, "home", "work" or "here" (wherever the person is then) */
  at: string;
  minutes: number;
  /** the chance it happens on a given day, before traits */
  chance: number;
  /** traits that make it likelier (+) or rarer (-), each a weight on (trait - 0.5) */
  leans?: Partial<Record<keyof Traits, number>>;
  /** only on these days of the week (0 Sunday) */
  days?: number[];
  /** done out of doors (see Block.outside) */
  outside?: boolean;
}

/** One block of a role's day: what, where and roughly when. A day is the blocks that apply, in order, with the
 *  trips between them added. */
export interface Block {
  what: string;
  /** a place kind, "home" or "work" */
  at: string;
  /** when it starts, and when it ends (or how long it lasts) */
  from: Minute;
  to?: Minute;
  minutes?: number;
  /** minutes either way the start may move, before traits (punctual people move less) */
  jitter?: number;
  /** the chance this block happens on a given day */
  chance?: number;
  /** only on these days of the week (0 Sunday) */
  days?: number[];
  /** acts performed there, in the stage's words (go, wait, look, face, sit, take, do) */
  acts?: string[];
  /** done out of doors at the place (a queue at a shop's door, a beat corner, a guard post), so the person is seen
   *  even when the place is `inside`; people out at the same door stand in a queue along the front */
  outside?: boolean;
  /** a block that moves: a patrol or a round goes from place to place of its kind */
  round?: { stops: number; dwell: number };
}

/** A role in a place: a profession, a rank in a service, a household part. The project lists its roles with how
 *  many of each the place holds; the SDK draws people from them. */
export interface Role {
  id: string;
  title: string;
  /** who fills it: the share of women (0 to 1) and the range of ages */
  women: number;
  ages: [number, number];
  /** the traits people in the role lean to (each 0 to 1; unset, 0.5) and how far one person may stray (sd, 0.15) */
  traits?: Partial<Traits>;
  spread?: number;
  /** a rank the role carries (a project's rank table names its insignia) */
  rank?: string;
  /** the dress, by the project's kit ids: one is drawn per person, as likely as its weight */
  dress: Record<string, number>;
  /** what they wear on the head, by the project's kit ids, drawn like the dress; a weight under "none" leaves some
   *  bareheaded, and a role without it draws none */
  head?: Record<string, number>;
  /** what they carry, by the project's thing ids, each with its chance */
  carry?: Record<string, number>;
  /** how they get about, by weight (walk unless said) */
  modes?: Partial<Record<Mode, number>>;
  /** a vehicle that is their work (a tram's driver, a cabman): its kind and their place on it */
  vehicle?: { kind: string; station: string };
  /** the kind of place they work at, and how far from home it may be (metres) */
  work?: { at: string; within?: number };
  /** their day */
  day: Block[];
  /** habits open to the role */
  habits?: Habit[];
  /** a gait the role keeps (a soldier marching, an officer upright), else one from age and condition */
  gait?: string;
}

/** A place people go: a home, a shop, a workplace, a stop, a post. */
export interface Place {
  id: string;
  kind: string;
  at: V3;
  /** the door's facing (radians about +Y), where people queue or stand */
  facing?: number;
  /** open hours, if it has them */
  opens?: [Minute, Minute];
  /** how many it holds (homes: households; workplaces: hands) */
  holds?: number;
  /** being there hides a person (a home, a works, a school): no character is drawn for them unless their block is
   *  `outside`. Unset: a place out of doors (a corner, a post, a courtyard, a square) */
  inside?: boolean;
  /** where a queue at this door forms, when the world says (a tile's queue line): its first place and the way it runs.
   *  Unset: a step out from the door, along the front */
  line?: { at: V3; along: V3 };
}

/** A face: a hero's own head (made from its pictures) or a crowd head drawn from a face model by these numbers. */
export type Face =
  | { kind: "hero"; id: string }
  | { kind: "crowd"; seed: number; build: number; tone?: V3; iris: string; stubble: number; age: number; moustache?: string; beard?: string };

/** A body: the standard skeleton's shape, by numbers the body build reads. */
export interface Body {
  height: number;
  build: number;
  /** the person's posture: a stoop, a limp, a missing limb */
  marks?: string[];
}

/** A person in the place. */
export interface Person {
  id: string;
  name: string;
  sex: Sex;
  age: number;
  role: string;
  rank?: string;
  trade?: string;
  home: string;
  work?: string;
  traits: Traits;
  habits: string[];
  face: Face;
  body: Body;
  hair: { style: string; colour: string; grey: number };
  dress: string;
  /** the kit id worn on the head, when the role draws one */
  head?: string;
  carry: string[];
  modes: Partial<Record<Mode, number>>;
  gait: string;
  seed: number;
}

/** One stretch of a person's day: at a place doing something, or on the way from one place to another. */
export type Activity =
  | { kind: "stay"; what: string; at: string; from: Minute; to: Minute; acts?: string[]; outside?: boolean }
  | { kind: "trip"; what: string; from: Minute; to: Minute; leave: string; arrive: string; mode: Mode; path: V3[]; metres: number };

/** A way from one place to another for a mode: the project's street graph, tram lines, or a straight line. */
export type Router = (from: Place, to: Place, mode: Mode) => { path: V3[]; metres: number };

/** The level a person is simulated at, by distance from the camera (docs/character.feature CH52); "inside" for a
 *  person hidden in a building, who is counted and never drawn. */
export type Level = "near" | "middle" | "far";
