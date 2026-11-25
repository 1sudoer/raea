// Weather that changes during a shot (docs/wear.feature WR9): rain starting, snow settling, frost forming. What lies
// on the surfaces (wet, snow, driven snow, frost) follows what the sky does at its own rate, so a shot that starts
// dry darkens over the minutes rain takes to soak a front and dries over the hour it takes to dry. `weatherAt` gives
// the surfaces' weather at a moment, for wearShader each frame.
//
//   const spells = [{ at: 0 }, { at: 2, rain: 1 }, { at: 40 }];        // dry, raining from minute 2 to 40
//   const minutes = (frame / fps) * 60;                                 // a timelapse: a second a minute
//   wearShader({ baked, weather: weatherAt({ day: 120 }, spells, minutes) })
import type { WearWeather } from "./index";

/** What the sky does from minute `at` of the shot on: rain, snow falling, a storm driving it, frost (each 0..1;
 *  what a spell leaves out is 0). */
export interface WearSpell {
  at: number;
  rain?: number;
  snow?: number;
  storm?: number;
  ice?: number;
}

/** How fast the surfaces follow the sky, the share of the way left covered per minute (ASSUMED until measured):
 *  wetting in about 3 minutes of steady rain, drying over about an hour; snow settling over about 20 minutes and
 *  melting over about two hours; driven snow plastering in about 10 and falling off over 30; frost forming over half
 *  an hour and going in 20. */
export const WEATHER_RATES = { wet: 1 / 3, dry: 1 / 60, settle: 1 / 20, melt: 1 / 120, plaster: 1 / 10, shed: 1 / 30, freeze: 1 / 30, thaw: 1 / 20 } as const;

type Lying = { wet: number; snow: number; storm: number; ice: number };
const KEYS = [
  ["wet", "rain", "wet", "dry"],
  ["snow", "snow", "settle", "melt"],
  ["storm", "storm", "plaster", "shed"],
  ["ice", "ice", "freeze", "thaw"],
] as const;

/** The surfaces' weather `minutes` into the shot: `base` is how they lie when it starts (its day, and its wet, snow,
 *  storm and ice), `spells` what the sky does from when (in order). Each value eases toward the sky's at its rate,
 *  exactly (an exponential over each spell), so any frame can be drawn alone. */
export function weatherAt(base: WearWeather, spells: readonly WearSpell[], minutes: number): WearWeather {
  if (!Number.isFinite(minutes)) throw new RangeError(`weatherAt: minutes is a number, got ${minutes}`);
  for (let i = 1; i < spells.length; i++) if (!(spells[i].at >= spells[i - 1].at)) throw new RangeError("weatherAt: spells are in order of `at`");
  const v: Lying = { wet: base.wet ?? 0, snow: base.snow ?? 0, storm: base.storm ?? 0, ice: base.ice ?? 0 };
  let t = spells.length ? Math.min(spells[0].at, minutes) : minutes;
  for (let i = 0; i < spells.length && spells[i].at <= minutes; i++) {
    const s = spells[i];
    const end = Math.min(i + 1 < spells.length ? spells[i + 1].at : minutes, minutes);
    const dt = Math.max(0, end - Math.max(t, s.at));
    for (const [k, sky, up, down] of KEYS) {
      const target = s[sky] ?? 0;
      if (!(target >= 0 && target <= 1)) throw new RangeError(`weatherAt: a spell's ${sky} is 0..1, got ${target}`);
      const rate = WEATHER_RATES[target > v[k] ? up : down];
      v[k] = target + (v[k] - target) * Math.exp(-rate * dt);
    }
    t = end;
  }
  const r = (x: number) => Math.round(x * 1e4) / 1e4;
  return { ...base, wet: r(v.wet), snow: r(v.snow), storm: r(v.storm), ice: r(v.ice) };
}

/** WR9's eight conditions as weather on the surfaces: a day of the year in its season and what lies on them. The
 *  light that goes with each (overcast for rain, a flash for thunder) is the scene's. */
export const WEAR_CONDITIONS = {
  /** damp lingering on north faces and plinths, algae where the run-off and the shade meet */
  spring: { day: 105, damp: 0.85 },
  /** dry: dust on ledges and treads */
  summer: { day: 205 },
  /** leaves where blown things settle, wet and dark after a shower */
  autumn: { day: 300, wet: 0.35, leaves: 1 },
  /** frost on the faces the winter sun misses */
  winter: { day: 20, ice: 0.8 },
  /** snow lying on what is open above, deeper in the lee */
  snow: { day: 30, snow: 0.85, ice: 0.4 },
  /** snow, and driven snow plastered on the faces the storm reaches */
  blizzard: { day: 40, snow: 1, storm: 0.9, ice: 0.6 },
  /** wet where the rain reaches, rivulets down the run-off's paths, puddles in the walk's hollows */
  rain: { day: 130, wet: 0.8 },
  /** heavy rain: wet through */
  thunder: { day: 200, wet: 1 },
} as const satisfies Record<string, WearWeather>;
export type WearCondition = keyof typeof WEAR_CONDITIONS;
