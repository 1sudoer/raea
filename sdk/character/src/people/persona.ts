// A person drawn from a role: sex, age, traits, habits, face, body, hair, dress, what they carry, how they travel and
// how they walk, all from one seed, so person 12 of a place is the same person on every run and every machine.
import { rng, type Rng } from "./rng";
import type { Body, Face, Mode, Person, Place, Role, Sex, Traits } from "./types";

export const TRAITS: (keyof Traits)[] = ["openness", "conscientiousness", "extraversion", "agreeableness", "neuroticism"];

/** Names a project gives for its people, by sex: given names and family names, each list as common first. */
export interface Names {
  given: Record<Sex, string[]>;
  family: string[];
}

/** What a project says about its people beyond the role: names, the hair styles and colours of the place and time,
 *  the face model's tone range, and the share of each iris colour. Everything has a default. */
export interface Folk {
  names: Names;
  /** hair styles by sex, each with a weight; colours (words a hair tool reads) by weight */
  hairStyles?: Record<Sex, Record<string, number>>;
  hairColours?: Record<string, number>;
  /** facial hair for men by age band: style -> weight (style "none" for clean-shaven) */
  facialHair?: { young: Record<string, number>; middle: Record<string, number>; old: Record<string, number> };
  irises?: Record<string, number>;
  /** the face model's build for this population: mean and sd (a hungry city leans thin: below 0) */
  build?: [number, number];
  /** heights by sex in metres: mean and sd */
  heights?: Record<Sex, [number, number]>;
  /** names no one may be given (a real resident's, for a place drawn from its records): a drawn name this rejects is
   *  drawn again, up to 50 times, then the person is named by role and number */
  avoid?: (given: string, family: string) => boolean;
}

const DEFAULT_HEIGHT: Record<Sex, [number, number]> = { f: [1.6, 0.06], m: [1.71, 0.065] };

function traitsFor(role: Role, r: Rng): Traits {
  const sd = role.spread ?? 0.15;
  const t = {} as Traits;
  for (const k of TRAITS) t[k] = Math.min(1, Math.max(0, r.normal(role.traits?.[k] ?? 0.5, sd)));
  return t;
}

/** Leans of a chance by traits: each weight times (trait - 0.5), added to the chance's log-odds. */
export function leaned(chance: number, traits: Traits, leans?: Partial<Record<keyof Traits, number>>): number {
  if (!leans) return chance;
  const c = Math.min(0.999, Math.max(0.001, chance));
  let logit = Math.log(c / (1 - c));
  for (const k of TRAITS) logit += (leans[k] ?? 0) * (traits[k] - 0.5) * 4;
  return 1 / (1 + Math.exp(-logit));
}

/** The gait a person keeps when nothing else sets it: a role's own (marching), else from age and nerves. Names are
 *  the motion library's situations (100STYLE's "Old", "Tired", "Proud", ...). */
export function gaitOf(role: Role, age: number, traits: Traits, r: Rng): string {
  if (role.gait) return role.gait;
  if (age >= 68) return "old";
  if (age >= 58 && r.chance(0.5)) return "old";
  if (traits.neuroticism > 0.72) return "hurried";
  if (traits.extraversion > 0.75 && traits.neuroticism < 0.35) return "proud";
  return r.chance(0.35) ? "tired" : "neutral";
}

function nameFor(folk: Folk, sex: Sex, r: Rng, fallback: string): string {
  for (let k = 0; k < 50; k++) {
    const given = r.pick(folk.names.given[sex]), family = r.pick(folk.names.family);
    if (!folk.avoid?.(given, family)) return `${given} ${family}`;
  }
  return fallback;
}

export function makePerson(id: string, role: Role, folk: Folk, home: Place, seed: number, work?: Place): Person {
  const r = rng(seed);
  const sex: Sex = r.chance(role.women) ? "f" : "m";
  const age = Math.round(role.ages[0] + (role.ages[1] - role.ages[0]) * Math.pow(r.next(), 1.15));
  const traits = traitsFor(role, r.fork("traits"));
  const name = nameFor(folk, sex, r.fork("name"), `${role.title} ${id}`);
  const habits = (role.habits ?? []).filter((h) => r.fork(h.id).next() < leaned(0.5, traits, h.leans)).map((h) => h.id);
  const [bm, bsd] = folk.build ?? [0, 0.8];
  const build = Math.max(-2.5, Math.min(2.5, r.normal(bm + (age > 45 ? 0.3 : 0), bsd)));
  const greyAt = r.normal(48, 9);
  const grey = Math.max(0, Math.min(1, (age - greyAt) / 22));
  const style = r.weighted(folk.hairStyles?.[sex] ?? { short: 1 });
  const colour = r.weighted(folk.hairColours ?? { brown: 1 });
  const band = age < 30 ? "young" : age < 50 ? "middle" : "old";
  const fh = sex === "m" && folk.facialHair ? r.weighted(folk.facialHair[band]) : "none";
  const face: Face = {
    kind: "crowd", seed: r.int(1, 2 ** 30), build, age, iris: r.weighted(folk.irises ?? { brown: 1 }),
    stubble: sex === "m" ? Math.round(r.next() * 60) / 100 : 0,
    ...(fh === "none" ? {} : fh.startsWith("beard") ? { beard: fh } : { moustache: fh }),
  };
  const [hm, hsd] = (folk.heights ?? DEFAULT_HEIGHT)[sex];
  const body: Body = { height: Math.round(r.normal(hm, hsd) * 100) / 100, build: Math.round(build * 10) / 10 };
  const dress = r.weighted(role.dress);
  // (on its own fork, so a role given headwear draws everything else as it did)
  const head = role.head ? r.fork("head").weighted(role.head) : "none";
  const carry = Object.entries(role.carry ?? {}).filter(([k, p]) => r.fork(`carry-${k}`).chance(p)).map(([k]) => k);
  const modes: Partial<Record<Mode, number>> = role.modes ?? { walk: 1 };
  return {
    id, name, sex, age, role: role.id, ...(role.rank ? { rank: role.rank } : {}), home: home.id, ...(work ? { work: work.id } : {}),
    traits, habits, face, body, hair: { style, colour, grey: Math.round(grey * 100) / 100 }, dress, ...(head === "none" ? {} : { head }), carry, modes,
    gait: gaitOf(role, age, traits, r.fork("gait")), seed,
  };
}
