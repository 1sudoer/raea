// The people passing through (docs/character.feature CH52): a street carries more than the people who live on it.
// Each sidewalk holds walkers by its length, how busy it is at its busiest hour and the hour's share of that. A
// walker comes out of a door, walks a stretch of the sidewalk at their own pace one way or the other, keeping to their
// right, and goes in at the door nearest where the stretch ends; then someone else comes out. A sidewalk round a
// block is closed, so walkers turn its corners; one with no doors on a stretch starts and stops there. Where a walker
// is at any minute is computed, not stepped, so any frame of a shot can be drawn alone.
import { walkSpeed } from "./day";
import { makePerson, type Folk } from "./persona";
import { hash, rng } from "./rng";
import type { Minute, Person, Place, Role, V3 } from "./types";

/** A sidewalk's centreline and how busy it is: walkers a metre at its busiest hour. */
export interface Walk {
  id: string;
  path: V3[];
  /** walkers a metre at the busiest hour (each leg's own when `legs` is given) */
  perMetre: number;
  /** its width (metres, 3): walkers keep to their right within it */
  width?: number;
  /** a ring (round a block): the last point joins the first, and walkers go on round */
  closed?: boolean;
  /** stretches of their own busyness, in order: each ends at `to` metres along the walk */
  legs?: { to: number; perMetre: number }[];
  /** the doors along it, in metres along the walk: walkers come out of and go in at these */
  doors?: number[];
}

/** A city street's walkers by hour, against its busiest (5 pm): the morning and evening rush, the noon errands, the
 *  empty night. Index h is h:00; a minute between is read between. */
export const STREET_HOURS = [0.03, 0.02, 0.02, 0.02, 0.04, 0.1, 0.3, 0.6, 0.7, 0.55, 0.6, 0.75, 0.9, 0.85, 0.75, 0.8, 0.9, 1, 0.85, 0.5, 0.3, 0.2, 0.1, 0.05];
/** The share of a walker's round spent walking (the rest is indoors, between a door and the next) */
export const OUT_SHARE = 0.7;
/** How far round a corner past its own leg a walker may go (metres) */
export const SPILL = 25;

export interface Passer {
  id: string;
  walk: string;
  at: V3;
  facing: number;
  person: Person;
  /** metres along the walk */
  s: number;
}

export interface FlowInput {
  roles: Role[];
  folk: Folk;
  /** the roles' shares among people on the street */
  mix: Record<string, number>;
  /** the street's mix by the minute of the day, when it changes over the day (the workers at the Works by day, the
   *  children in school till one): each round of a walker (out of a door, along the street, in at a door) is then its
   *  own person, drawn by the mix at the minute the round began, so the street's people change as the day goes and no
   *  one changes on it (a passer's id is its round's). Without it every walker keeps one person by `mix` all day; an
   *  empty mix at a minute takes `mix`. streetMix(day) gives it from a city's own days. */
  mixAt?: (m: Minute) => Record<string, number>;
  seed: number | string;
  /** the hour profile (STREET_HOURS) */
  hours?: number[];
  /** how cold (0 mild to 1 bitter): pace */
  cold?: number;
  /** more or fewer than the walks' own numbers (1): a shot's busier street */
  busy?: number;
  /** only walks with a point within `within` metres of `near` */
  near?: V3;
  within?: number;
}

const people = new Map<string, Person>();

/** The share of the busiest hour at a minute of the day. */
export function busyAt(m: Minute, hours = STREET_HOURS): number {
  const h = (((m / 60) % 24) + 24) % 24, i = Math.floor(h), k = h - i;
  return hours[i] * (1 - k) + hours[(i + 1) % 24] * k;
}

/** A walk's points with the closing point for a ring, and the metres along it to each. */
function measure(w: Walk) {
  const P = w.closed ? [...w.path, w.path[0]] : w.path;
  const cum = [0];
  for (let i = 1; i < P.length; i++) cum.push(cum[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][2] - P[i - 1][2]));
  return { P, cum, L: cum[cum.length - 1] };
}
const walks = new WeakMap<Walk, ReturnType<typeof measure>>();

/** The point `s` metres along a walk and its direction there, turned smoothly through the TURN metres either side of
 *  a corner (a walker's keep-right offset hangs off the direction: a sharp turn would jump them across the corner). */
const TURN = 1.5;
function along(m: ReturnType<typeof measure>, s: number, closed: boolean): { at: V3; tx: number; tz: number } {
  const { P, cum } = m, n = cum.length - 1;
  let lo = 0, hi = n;
  while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (cum[mid] <= s) lo = mid; else hi = mid; }
  const dir = (i: number): [number, number] => {
    // segment i's direction (wrapping round a ring; held at an open walk's ends)
    const k = closed ? ((i % n) + n) % n : Math.min(n - 1, Math.max(0, i));
    const d = cum[k + 1] - cum[k] || 1;
    return [(P[k + 1][0] - P[k][0]) / d, (P[k + 1][2] - P[k][2]) / d];
  };
  const A = P[lo], B = P[hi], d = cum[hi] - cum[lo] || 1, u = Math.min(1, Math.max(0, (s - cum[lo]) / d));
  let [tx, tz] = dir(lo);
  const intoStart = s - cum[lo], toEnd = cum[hi] - s;
  // within TURN of either end of the segment, half-way to the neighbour's direction at the corner itself
  const blend = (o: [number, number], x: number) => { const w = 0.5 * Math.max(0, 1 - x / TURN); tx = tx * (1 - w) + o[0] * w; tz = tz * (1 - w) + o[1] * w; };
  if (intoStart < TURN && (closed || lo > 0)) blend(dir(lo - 1), intoStart);
  if (toEnd < TURN && (closed || hi < n)) blend(dir(lo + 1), toEnd);
  const l = Math.hypot(tx, tz) || 1;
  return { at: [A[0] + (B[0] - A[0]) * u, A[1] + (B[1] - A[1]) * u, A[2] + (B[2] - A[2]) * u], tx: tx / l, tz: tz / l };
}

const longest = (x: number, len: number) => Math.min(40 + 160 * x, len + SPILL);
/** One round's stretch on the leg [a, b]: out of a door (or anywhere on a leg with none), most walking the way with
 *  more of the leg ahead of them (so a busy leg's walkers stay on it), as far as the round allows. */
function stretchOf(rc: ReturnType<typeof rng>, a: number, b: number, doors: number[], most: number) {
  const s0 = doors.length ? doors[Math.floor(rc.next() * doors.length)] : a + rc.next() * (b - a);
  const ahead = b - s0, behind = s0 - a, dir = rc.next() * (ahead + behind) < ahead ? 1 : -1;
  return { s0, dir, D: Math.max(8, Math.min(most, (dir > 0 ? ahead : behind) + SPILL * rc.next())) };
}
/** The mean share of its longest that a round's stretch spends on its own leg (from 256 draws of its own: the part
 *  round a corner counts on the next leg, not this one) */
function stretchShare(a: number, b: number, doors: number[]): number {
  const r = rng(`stretch-${a}-${b}-${doors.length}`);
  let sum = 0;
  for (let k = 0; k < 256; k++) {
    const most = longest(r.next(), b - a), { s0, dir, D } = stretchOf(r, a, b, doors, most);
    const end = s0 + dir * D, on = Math.min(Math.max(s0, end), b) - Math.max(Math.min(s0, end), a);
    sum += Math.max(0, on) / most;
  }
  return sum / 256;
}
const stretch = new Map<string, number>();
/** A round when walkers come and go by the hour's mix: as long as the slowest walker's (an old man in the cold, 0.68 m/s)
 *  takes over its longest stretch, so anyone's walk fits in it; walkers are out less of it than of their own round, by
 *  the street's mean pace over it, so a walk has that many more slots */
const ROUND_PACE = 0.65, MEAN_PACE = 1.2;
const rounds = new Map<string, { n: number; p: Person }>();

/** The person a slot (or one round of it, `n`) is: drawn by the mix, the same on every call */
function passer(f: FlowInput, roles: Map<string, Role>, id: string, mix: Record<string, number>, home: Place, n?: number): Person {
  if (n === undefined) {
    let p = people.get(`${f.seed}-${id}`);
    if (!p) {
      // who they are from a draw of their own, so a walker made earlier walks the same way
      p = makePerson(id, roles.get(rng(`${f.seed}-${id}-who`).weighted(mix))!, f.folk, home, hash(`${f.seed}-${id}`));
      people.set(`${f.seed}-${id}`, p);
    }
    return p;
  }
  // (a slot keeps only its latest round's person: rounds go by, and a minute asked again draws the same one again)
  const key = `${f.seed}-${id}`, had = rounds.get(key);
  if (had?.n === n) return had.p;
  const rid = `${id}~${n}`, p = makePerson(rid, roles.get(rng(`${f.seed}-${rid}-who`).weighted(mix))!, f.folk, home, hash(`${f.seed}-${rid}`));
  rounds.set(key, { n, p });
  return p;
}

/** Everyone walking the given sidewalks at minute `m` (fractional minutes are seconds): who, where, which way. */
export function passersBy(list: Walk[], m: Minute, f: FlowInput): Passer[] {
  const busy = busyAt(m, f.hours), out: Passer[] = [], sec = m * 60;
  const roles = new Map(f.roles.map((r) => [r.id, r]));
  for (const w of list) {
    if (f.near && f.within !== undefined && !w.path.some((p) => Math.hypot(p[0] - f.near![0], p[2] - f.near![2]) <= f.within!)) continue;
    let g = walks.get(w);
    if (!g) walks.set(w, (g = measure(w)));
    const L = g.L;
    if (L < 2) continue;
    const legs = w.legs ?? [{ to: L, perMetre: w.perMetre }];
    let from = 0;
    legs.forEach((leg, j) => {
      const a = from, b = Math.min(L, leg.to), len = b - a;
      from = b;
      if (len <= 0) return;
      const doors = (w.doors ?? []).filter((d) => d >= a && d < b);
      // the leg's walkers at its busiest; a slot walks when its own draw is under the hour's share, so the walkers
      // thin out and come back as the day goes without anyone jumping. (Indoors part of each round: more slots.)
      // (a round's stretch is often shorter than its longest, the rest of the round indoors: more slots for that, by
      // the leg's own mean share, which its length and doors decide)
      const key = `${w.id}:${j}`;
      let share = stretch.get(key);
      if (share === undefined) stretch.set(key, (share = stretchShare(a, b, doors)));
      const slots = Math.round(((len * leg.perMetre * (f.busy ?? 1)) / (OUT_SHARE * share)) * (f.mixAt ? MEAN_PACE / ROUND_PACE : 1));
      for (let k = 0; k < slots; k++) {
        const slot = `${w.id}:${j}:${k}`, r = rng(`${f.seed}-${slot}`);
        if (r.next() >= busy) continue;
        const home: Place = { id: `passing-${w.id}`, kind: "street", at: w.path[0] };
        // a round: out of a door, along the leg (and round its corner by up to SPILL metres), in at a door, a while
        // inside. The longest stretch sets the round; a shorter one leaves them inside longer that round.
        let p: Person, v: number, most: number, round: number, tau: number, id = slot;
        if (f.mixAt) {
          // (the round at the slowest pace; its person by the mix when it began, at the round's start in minutes)
          const pace = 0.9 + 0.2 * r.next();
          most = longest(r.next(), len);
          round = most / ROUND_PACE / OUT_SHARE;
          const phase = r.next() * round, k0 = Math.floor((sec + phase) / round);
          tau = sec + phase;
          const mx = f.mixAt((k0 * round - phase) / 60);
          p = passer(f, roles, slot, Object.keys(mx).length ? mx : f.mix, home, k0);
          id = p.id;
          v = walkSpeed(p, f.cold ?? 0.5) * pace;
        } else {
          p = passer(f, roles, slot, f.mix, home);
          v = walkSpeed(p, f.cold ?? 0.5) * (0.9 + 0.2 * r.next());
          most = longest(r.next(), len);
          round = most / v / OUT_SHARE;
          tau = sec + r.next() * round;
        }
        const n = Math.floor(tau / round), u = tau - n * round;
        const { s0, dir, D } = stretchOf(rng(`${f.seed}-${id}-${n}`), a, b, doors, most);
        if (u > D / v) continue;
        let s = s0 + dir * v * u;
        if (w.closed) s = ((s % L) + L) % L;
        else if (s < 0 || s > L) continue; // walked off an open walk's end: turned into a street it doesn't cover
        const { at, tx, tz } = along(g, s, !!w.closed);
        // keep right: anywhere from the centreline to near the right-hand edge (a busy pavement is two loose streams,
        // not two files)
        const off = dir * (w.width ?? 3) * (0.04 + 0.38 * r.next());
        out.push({ id, walk: w.id, at: [at[0] - tz * off, at[1], at[2] + tx * off], facing: Math.atan2(dir * tx, dir * tz), person: p, s });
      }
    });
  }
  return out;
}
