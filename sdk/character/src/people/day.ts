// A person's day: the role's blocks that apply on that date, at real places, at times moved by how punctual the person
// is, with the trips between them (a mode drawn by the person's own leanings and the distance), and habits slotted in
// where they fit. The same person and date give the same day.
import { gridOf } from "./near";
import { leaned } from "./persona";
import { rng } from "./rng";
import type { Activity, Block, Habit, Minute, Mode, Person, Place, Role, Router, V3 } from "./types";

/** Speeds by mode in metres a second, door to door (a tram's counts its stops; a walk is an ordinary adult's). */
export const SPEED: Record<Mode, number> = { walk: 1.25, tram: 4.2, bus: 3.6, cab: 3.4, cart: 1.6, bicycle: 3.8, car: 6, train: 9 };
/** Below this a trip is walked whatever the person's leaning (metres). */
export const WALK_UNDER = 900;

/** A straight line, a fifth longer for the corners a street makes. A project's router follows its streets. */
export const straight: Router = (a, b) => {
  const d = Math.hypot(b.at[0] - a.at[0], b.at[2] - a.at[2]) * 1.2;
  return { path: [a.at, b.at], metres: d };
};

/** How fast this person walks: age, the gait and the cold (people hurry in the cold, the old slow down). */
export function walkSpeed(p: Person, cold = 0.5): number {
  const age = p.age < 25 ? 1.05 : p.age < 55 ? 1 : p.age < 70 ? 0.86 : 0.74;
  const gait = p.gait === "old" ? 0.85 : p.gait === "hurried" ? 1.15 : p.gait === "tired" ? 0.92 : p.gait === "march" ? 1.1 : 1;
  return SPEED.walk * age * gait * (1 + 0.08 * (cold - 0.5));
}

export interface DayInput {
  person: Person;
  role: Role;
  /** the date: its weekday (0 Sunday) decides which blocks apply, its key varies the day ("1917-12-03") */
  date: { key: string; weekday: number };
  /** the places by id, and all places of a kind (for blocks "at" a kind: the nearest open one, else any) */
  places: Map<string, Place>;
  byKind: Map<string, Place[]>;
  router?: Router;
  /** how cold it is (0 mild to 1 bitter): pace and how many take the tram */
  cold?: number;
}

function placeFor(at: string, p: Person, inp: DayInput, near: Place, r: ReturnType<typeof rng>): Place | undefined {
  if (at === "home") return inp.places.get(p.home);
  if (at === "work") return p.work ? inp.places.get(p.work) : undefined;
  if (at === "here") return near;
  const all = inp.byKind.get(at);
  if (!all || all.length === 0) return undefined;
  // the nearest few, one of them (people do not all use the one nearest shop)
  const ranked = gridOf(all).nearest(near.at, 4);
  return ranked[Math.min(ranked.length - 1, Math.floor(Math.pow(r.next(), 2) * ranked.length))];
}

function modeFor(p: Person, metres: number, cold: number, r: ReturnType<typeof rng>): Mode {
  if (metres < WALK_UNDER) return "walk";
  const w = { ...p.modes };
  // the further and the colder, the more the tram (or whatever the person rides) is worth the fare
  const far = Math.min(1, (metres - WALK_UNDER) / 2500);
  for (const k of Object.keys(w) as Mode[]) if (k !== "walk") w[k] = (w[k] ?? 0) * (1 + 2 * far + cold);
  return r.weighted(w);
}

function applies(b: Block | Habit, weekday: number) {
  return !b.days || b.days.includes(weekday);
}

/** The day's activities, in order, covering 00:00 to 24:00. */
export function dayOf(inp: DayInput): Activity[] {
  const { person: p, role, date } = inp;
  const r = rng(`${p.seed}-${date.key}`);
  const cold = inp.cold ?? 0.5;
  const route = inp.router ?? straight;
  const home = inp.places.get(p.home)!;
  // punctual people keep to their times: jitter scaled from 1.6x (careless) to 0.4x (exact)
  const keep = 1.6 - 1.2 * p.traits.conscientiousness;
  type Stay = { what: string; at: Place; from: Minute; to: Minute; acts?: string[]; outside?: boolean };
  const stays: Stay[] = [];
  let here = home;
  for (const b of role.day) {
    if (!applies(b, date.weekday)) continue;
    if (b.chance !== undefined && !r.chance(b.chance)) continue;
    const stops = b.round ? b.round.stops : 1;
    let from = b.from + Math.round(r.normal(0, (b.jitter ?? 10) * keep));
    const end = b.to ?? from + (b.minutes ?? 60);
    for (let s = 0; s < stops; s++) {
      // a block with nowhere to happen (no workplace was found for the person, no place of its kind in the city) is
      // not lived that day: a sentry with no post stays home rather than standing guard at their own door
      const at = placeFor(b.at, p, inp, here, r);
      if (!at) break;
      const to = b.round ? Math.min(end, from + b.round.dwell) : end + Math.round(r.normal(0, (b.jitter ?? 10) * keep * 0.5));
      stays.push({ what: b.what, at, from, to: Math.max(from + 1, to), acts: b.acts, outside: b.outside });
      here = at;
      from = to + 3;
    }
  }
  // habits, each on its own chance for the day, leaned by traits, slotted at their place within their window
  for (const h of role.habits ?? []) {
    if (!p.habits.includes(h.id) || !applies(h, date.weekday)) continue;
    if (!r.fork(h.id).chance(leaned(h.chance, p.traits, h.leans))) continue;
    const start = h.from + Math.floor(r.next() * Math.max(1, h.to - h.from - h.minutes));
    const host = [...stays].reverse().find((s) => s.from <= start) ?? stays[0];
    const at = h.at === "here" && host ? host.at : placeFor(h.at, p, inp, host?.at ?? home, r);
    if (!at) continue;
    stays.push({ what: h.act, at, from: start, to: start + h.minutes, outside: h.outside });
  }
  stays.sort((a, b) => a.from - b.from);
  // home before the first block and after the last; overlaps clipped (a later block wins its start)
  const out: Activity[] = [];
  let at = home, t = 0;
  // a trip planned before it is taken: the mode first (by distance, cold and the person's leanings), then how long
  // that mode takes, so the person leaves in time for the block by the way they actually travel
  const plan = (dest: Place) => {
    const way = route(at, dest, "walk");
    const mode = modeFor(p, way.metres, cold, r);
    const path = mode === "walk" ? way : route(at, dest, mode);
    const speed = mode === "walk" ? walkSpeed(p, cold) : SPEED[mode];
    const minutes = Math.max(1, Math.round(path.metres / speed / 60 + (mode === "tram" || mode === "bus" ? 6 : 0)));
    return { mode, path, minutes };
  };
  const take = (dest: Place, leaveAt: Minute, what: string, way: ReturnType<typeof plan>) => {
    out.push({ kind: "trip", what, from: leaveAt, to: leaveAt + way.minutes, leave: at.id, arrive: dest.id, mode: way.mode, path: way.path.path, metres: Math.round(way.path.metres) });
    at = dest;
    return leaveAt + way.minutes;
  };
  const legTo = (dest: Place, leaveAt: Minute, what: string) => (dest.id === at.id ? leaveAt : take(dest, leaveAt, what, plan(dest)));
  for (const s of stays) {
    if (s.to <= t) continue;
    if (s.at.id !== at.id) {
      // leave in time to arrive at the block's start; one who arrives early waits for it, one who can't make it is late
      const way = plan(s.at);
      const leave = Math.max(t, s.from - way.minutes);
      if (leave > t) out.push({ kind: "stay", what: at.id === p.home ? "home" : "wait", at: at.id, from: t, to: leave });
      t = take(s.at, leave, `to ${s.what}`, way);
      if (s.from > t) {
        out.push({ kind: "stay", what: "wait", at: at.id, from: t, to: s.from });
        t = s.from;
      }
    } else if (s.from > t) {
      out.push({ kind: "stay", what: at.id === p.home ? "home" : "wait", at: at.id, from: t, to: s.from });
      t = s.from;
    }
    const to = Math.max(t + 1, s.to);
    out.push({ kind: "stay", what: s.what, at: s.at.id, from: t, to, ...(s.acts ? { acts: s.acts } : {}), ...(s.outside ? { outside: true } : {}) });
    t = to;
  }
  if (at.id !== home.id && t < 1440) t = legTo(home, t, "home");
  if (t < 1440) out.push({ kind: "stay", what: "home", at: home.id, from: t, to: 1440 });
  return out;
}

/** Where a person is at a minute of their day, and what they are doing: a place (standing at its door) or a point
 *  along a trip, with the direction they face. */
export function whereAt(day: Activity[], places: Map<string, Place>, m: Minute): { at: V3; facing: number; doing: string; mode?: Mode; activity: Activity } {
  const a = day.find((x) => x.from <= m && m < x.to) ?? day[day.length - 1];
  if (a.kind === "stay") {
    const pl = places.get(a.at)!;
    return { at: pl.at, facing: pl.facing ?? 0, doing: a.what, activity: a };
  }
  const u = Math.min(1, Math.max(0, (m - a.from) / Math.max(1, a.to - a.from)));
  const P = a.path;
  const seg = P.slice(1).map((q, i) => Math.hypot(q[0] - P[i][0], q[2] - P[i][2]));
  const total = seg.reduce((x, y) => x + y, 0) || 1;
  let d = u * total, i = 0;
  while (i < seg.length - 1 && d > seg[i]) d -= seg[i++];
  const A = P[i], B = P[Math.min(i + 1, P.length - 1)], k = seg[i] ? d / seg[i] : 0;
  const at: V3 = [A[0] + (B[0] - A[0]) * k, A[1] + (B[1] - A[1]) * k, A[2] + (B[2] - A[2]) * k];
  return { at, facing: Math.atan2(B[0] - A[0], B[2] - A[2]), doing: a.what, mode: a.mode, activity: a };
}
