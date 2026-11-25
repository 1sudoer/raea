// A seeded random stream for people: the same seed gives the same person, the same day and the same choices on every
// machine, so a render and live play agree (sdk/guides/craft/bevy/games.md: a game is a pure step over its input).

/** A random stream from a seed: `next()` is uniform in [0, 1). splitmix32: small, fast and well mixed. */
export interface Rng {
  next(): number;
  /** a whole number in [lo, hi] */
  int(lo: number, hi: number): number;
  /** a normal draw, mean `mu`, standard deviation `sd` */
  normal(mu?: number, sd?: number): number;
  /** one of `xs`, each as likely */
  pick<T>(xs: readonly T[]): T;
  /** one key of `w`, as likely as its weight */
  weighted<K extends string>(w: Readonly<Partial<Record<K, number>>>): K;
  /** true with chance `p` */
  chance(p: number): boolean;
  /** a stream of its own for `label`, unaffected by how much of this one has been drawn */
  fork(label: string | number): Rng;
}

/** A 32-bit hash of a string, for seeds made from names ("person-12", "day-1917-12-03"). */
export function hash(s: string | number): number {
  let h = 0x811c9dc5 ^ 0;
  const t = String(s);
  for (let i = 0; i < t.length; i++) h = Math.imul(h ^ t.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

export function rng(seed: number | string): Rng {
  const base = typeof seed === "number" ? seed >>> 0 : hash(seed);
  let s = base;
  const next = () => {
    s = (s + 0x9e3779b9) >>> 0;
    let z = s;
    z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
    return ((z ^ (z >>> 16)) >>> 0) / 4294967296;
  };
  const self: Rng = {
    next,
    int: (lo, hi) => lo + Math.floor(next() * (hi - lo + 1)),
    normal: (mu = 0, sd = 1) => mu + sd * Math.sqrt(-2 * Math.log(1 - next())) * Math.cos(2 * Math.PI * next()),
    pick: (xs) => xs[Math.floor(next() * xs.length)],
    weighted: (w) => {
      const ks = Object.keys(w) as (keyof typeof w)[];
      const total = ks.reduce((a, k) => a + (w[k] ?? 0), 0);
      let r = next() * total;
      for (const k of ks) if ((r -= w[k] ?? 0) < 0) return k as never;
      return ks[ks.length - 1] as never;
    },
    chance: (p) => next() < p,
    fork: (label) => rng((base ^ hash(label)) >>> 0),
  };
  return self;
}
