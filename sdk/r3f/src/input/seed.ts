// Math.random as a seeded generator of its own: a scene started again draws the same numbers (session.ts, program.ts).

/** the next of `s`'s random numbers (mulberry32), its state kept in s.rand */
export function nextRandom(s: { rand: number }): number {
  let t = (s.rand = (s.rand + 0x6d2b79f5) | 0);
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

export const seedOf = (key: string) => [...key].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261) | 0;
