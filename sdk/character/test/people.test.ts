// raea/people (src/people.ts, docs/character.feature CH50-CH53): people drawn from roles, their days, where they are
// at a minute, the levels of detail and how they meet an event.
import { expect, test } from "bun:test";
import { census, cityDay, dayOf, Grid, levelAt, passersBy, populate, react, rng, snapshot, streetMix, whereAt, type Walk, type Folk, type Place, type Role, type V3 } from "../src/people";

const folk: Folk = {
  names: { given: { f: ["Anna", "Martha", "Frieda"], m: ["Karl", "Otto", "Paul"] }, family: ["Schulz", "Müller", "Krüger"] },
  irises: { blue: 2, brown: 1 }, build: [-0.8, 0.6],
  facialHair: { young: { none: 3, moustache: 1 }, middle: { none: 1, moustache: 2 }, old: { moustache: 2, "beard-full": 1 } },
};
const places: Place[] = [
  ...Array.from({ length: 20 }, (_, i) => ({ id: `h${i}`, kind: "home", at: [i * 30, 0, 0] as [number, number, number], holds: 4 })),
  { id: "bakery", kind: "bakery", at: [100, 0, 40], opens: [6 * 60, 19 * 60] },
  { id: "bakery2", kind: "bakery", at: [500, 0, 40], opens: [6 * 60, 19 * 60] },
  { id: "works", kind: "munitions", at: [2500, 0, 1800], holds: 200 },
  { id: "post1", kind: "post", at: [300, 0, 10] },
  { id: "post2", kind: "post", at: [400, 0, -60] },
];
const roles: Role[] = [
  {
    id: "housewife", title: "Housewife", women: 1, ages: [22, 70], dress: { "winter-coat": 2, shawl: 1 }, carry: { basket: 0.8 },
    traits: { conscientiousness: 0.6 },
    day: [{ what: "queue for bread", at: "bakery", from: 6 * 60, minutes: 120, jitter: 30, acts: ["wait", "look at the shop door"] },
      { what: "market", at: "bakery", from: 10 * 60 + 30, minutes: 45, chance: 0.5 }],
    habits: [{ id: "church", act: "church", from: 9 * 60, to: 11 * 60, at: "home", minutes: 60, chance: 0.9, days: [0] }],
  },
  {
    id: "munitions-worker", title: "Munitions worker", women: 0.8, ages: [16, 45], dress: { overall: 1 }, work: { at: "munitions" },
    modes: { walk: 1, tram: 3 }, day: [{ what: "shift", at: "work", from: 6 * 60, to: 18 * 60, jitter: 5 }],
  },
  {
    id: "sentry", title: "Landsturm sentry", women: 0, ages: [38, 48], rank: "landsturmmann", dress: { "greatcoat-m15": 1 }, gait: "march",
    traits: { conscientiousness: 0.75 },
    day: [{ what: "patrol", at: "post", from: 8 * 60, to: 16 * 60, round: { stops: 6, dwell: 50 }, jitter: 2 }],
  },
];
const mix = { housewife: 50, "munitions-worker": 20, sentry: 2 };

test("the same seed draws the same people, a different one others", () => {
  const a = populate({ roles, places, folk, mix, seed: "leipziger" }), b = populate({ roles, places, folk, mix, seed: "leipziger" });
  const c = populate({ roles, places, folk, mix, seed: "jerusalemer" });
  expect(a.length).toBe(72);
  expect(JSON.stringify(a)).toBe(JSON.stringify(b));
  expect(a.map((p) => p.name).join()).not.toBe(c.map((p) => p.name).join());
});

test("a role's sex, ages, rank and gait hold for everyone drawn from it", () => {
  const ps = populate({ roles, places, folk, mix, seed: 3 });
  for (const p of ps.filter((x) => x.role === "housewife")) { expect(p.sex).toBe("f"); expect(p.age).toBeGreaterThanOrEqual(22); expect(p.age).toBeLessThanOrEqual(70); }
  for (const p of ps.filter((x) => x.role === "sentry")) { expect(p.rank).toBe("landsturmmann"); expect(p.gait).toBe("march"); expect(p.sex).toBe("m"); }
  // workers have a workplace; faces are crowd faces with a seed of their own
  expect(ps.filter((x) => x.role === "munitions-worker").every((p) => p.work === "works")).toBe(true);
  expect(new Set(ps.map((p) => (p.face.kind === "crowd" ? p.face.seed : 0))).size).toBe(ps.length);
});

test("headwear is drawn by the role's weights, and giving a role headwear draws everything else as before", () => {
  const capped = roles.map((r) => (r.id === "sentry" ? { ...r, head: { "landsturm-cap": 2, feldmuetze: 1 } } : r.id === "housewife" ? { ...r, head: { headscarf: 1, none: 1 } } : r));
  const a = populate({ roles, places, folk, mix, seed: 3 }), b = populate({ roles: capped, places, folk, mix, seed: 3 });
  expect(b.map(({ head: _, ...p }) => p)).toEqual(a);
  expect(a.every((p) => p.head === undefined)).toBe(true);
  const sentries = b.filter((p) => p.role === "sentry"), wives = b.filter((p) => p.role === "housewife");
  expect(sentries.every((p) => p.head === "landsturm-cap" || p.head === "feldmuetze")).toBe(true);
  // "none" leaves some bareheaded, about as often as its weight says
  const bare = wives.filter((p) => p.head === undefined).length / wives.length;
  expect(bare).toBeGreaterThan(0.3); expect(bare).toBeLessThan(0.7);
  expect(b.filter((p) => p.role === "munitions-worker").every((p) => p.head === undefined)).toBe(true);
});

test("a day covers 00:00 to 24:00 with no gaps, and a trip joins every change of place", () => {
  const ps = populate({ roles, places, folk, mix, seed: 5 });
  const d = cityDay(ps, roles, places, { key: "1917-12-03", weekday: 1 });
  for (const p of ps) {
    const day = d.days.get(p.id)!;
    expect(day[0].from).toBe(0);
    expect(day[day.length - 1].to).toBe(1440);
    for (let i = 1; i < day.length; i++) expect(day[i].from).toBe(day[i - 1].to);
    let at = p.home;
    for (const a of day) {
      if (a.kind === "trip") { expect(a.leave).toBe(at); at = a.arrive; } else expect(a.at).toBe(at);
    }
  }
});

test("a far commute in the cold goes by tram; a short errand is walked", () => {
  const ps = populate({ roles, places, folk, mix, seed: 9 });
  const d = cityDay(ps, roles, places, { key: "1917-12-04", weekday: 2 }, undefined, 0.9);
  const trips = [...d.days.values()].flat().filter((a) => a.kind === "trip");
  const far = trips.filter((t) => t.kind === "trip" && t.metres > 2500), near = trips.filter((t) => t.kind === "trip" && t.metres < 900);
  expect(far.filter((t) => t.kind === "trip" && t.mode === "tram").length / far.length).toBeGreaterThan(0.6);
  expect(near.every((t) => t.kind === "trip" && t.mode === "walk")).toBe(true);
});

test("punctual people start their shift closer to its time than careless ones", () => {
  const ps = populate({ roles, places, folk, mix: { "munitions-worker": 200 }, seed: 11 });
  const d = cityDay(ps, roles, places, { key: "1917-12-05", weekday: 3 });
  const late = (p: (typeof ps)[number]) => Math.abs(d.days.get(p.id)!.find((a) => a.kind === "stay" && a.what === "shift")!.from - 360);
  const exact = ps.filter((p) => p.traits.conscientiousness > 0.65), loose = ps.filter((p) => p.traits.conscientiousness < 0.35);
  const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
  expect(mean(exact.map(late))).toBeLessThan(mean(loose.map(late)));
});

test("a patrol is a round of posts; Sunday brings church", () => {
  const ps = populate({ roles, places, folk, mix, seed: 13 });
  const sun = cityDay(ps, roles, places, { key: "1917-12-02", weekday: 0 });
  const sentry = ps.find((p) => p.role === "sentry")!;
  const posts = sun.days.get(sentry.id)!.filter((a) => a.kind === "stay" && a.what === "patrol");
  expect(posts.length).toBe(6);
  const church = ps.filter((p) => p.habits.includes("church")).filter((p) => sun.days.get(p.id)!.some((a) => a.kind === "stay" && a.what === "church"));
  expect(church.length).toBeGreaterThan(0);
  const mon = cityDay(ps, roles, places, { key: "1917-12-03", weekday: 1 });
  expect([...mon.days.values()].flat().some((a) => a.kind === "stay" && a.what === "church")).toBe(false);
});

test("whereAt walks a trip's path; snapshot and census count everyone", () => {
  const ps = populate({ roles, places, folk, mix, seed: 17 });
  const d = cityDay(ps, roles, places, { key: "1917-12-03", weekday: 1 });
  const day = d.days.get(ps.find((p) => p.role === "munitions-worker")!.id)!;
  const trip = day.find((a) => a.kind === "trip")!;
  const mid = whereAt(day, d.places, (trip.from + trip.to) / 2);
  expect(mid.activity).toBe(trip);
  const s = snapshot(d, 7 * 60, [0, 0, 0], { near: 150, middle: 1500 });
  expect(s.length).toBe(ps.length);
  expect(s.some((x) => x.level === "far")).toBe(true);
  const c = census(d);
  for (let h = 0; h < 24; h++) expect([...c.values()].reduce((a, v) => a + v[h], 0)).toBe(ps.length);
});

test("levels change with hysteresis at their edges", () => {
  const range = { near: 150, middle: 1500 };
  expect(levelAt(140, range)).toBe("near");
  expect(levelAt(160, range, "near")).toBe("near");
  expect(levelAt(170, range, "near")).toBe("middle");
  expect(levelAt(140, range, "middle")).toBe("middle");
  expect(levelAt(130, range, "middle")).toBe("near");
});

test("a shock underfoot turns eyes to the ground; the nervous flinch, the far feel nothing", () => {
  const ps = populate({ roles, places, folk, mix, seed: 19 });
  const e = { kind: "tremor", at: [0, 0, 0] as [number, number, number], strength: 1 };
  const nervous = ps.find((p) => p.traits.neuroticism > 0.65)!;
  const near = react(nervous, e, [5, 0, 0]);
  expect(near.acts[0]).toBe("look down at the ground");
  expect(near.acts.some((a) => a === "flinch and duck" || a === "cover the ears")).toBe(true);
  expect(react(nervous, e, [5000, 0, 0]).acts.length).toBe(0);
});

test("dayOf is the same for the same person and date", () => {
  const ps = populate({ roles, places, folk, mix, seed: 23 });
  const pm = new Map(places.map((p) => [p.id, p])), byKind = new Map<string, Place[]>();
  for (const p of places) byKind.set(p.kind, [...(byKind.get(p.kind) ?? []), p]);
  const inp = { person: ps[0], role: roles[0], date: { key: "1918-01-15", weekday: 2 }, places: pm, byKind };
  expect(JSON.stringify(dayOf(inp))).toBe(JSON.stringify(dayOf(inp)));
});

test("a name the place rejects (a real resident's) is never given", () => {
  const avoid = (_g: string, f: string) => f === "Müller" || f === "Schulz";
  const ps = populate({ roles, places, folk: { ...folk, avoid }, mix, seed: 29 });
  expect(ps.every((p) => !p.name.endsWith(" Müller") && !p.name.endsWith(" Schulz"))).toBe(true);
  // every family name rejected: named by role and number instead
  const none = populate({ roles, places, folk: { ...folk, avoid: () => true }, mix: { sentry: 1 }, seed: 31 });
  expect(none[0].name).toBe("Landsturm sentry p0");
});

test("people inside are counted, not drawn; a queue stands in a line along the front, first come nearest", () => {
  const inside = places.map((p) => (p.kind === "home" || p.kind === "munitions" ? { ...p, inside: true } : p.kind === "bakery" ? { ...p, inside: true, facing: 0 } : p));
  const queueRoles = roles.map((r) => (r.id === "housewife" ? { ...r, day: r.day.map((b) => (b.what === "queue for bread" ? { ...b, outside: true } : b)) } : r));
  const ps = populate({ roles: queueRoles, places: inside, folk, mix, seed: 37 });
  const d = cityDay(ps, queueRoles, inside, { key: "1917-12-03", weekday: 1 });
  const s = snapshot(d, 2 * 60, [0, 0, 0], { near: 150, middle: 1500 });
  expect(s.every((x) => x.level === "inside")).toBe(true);
  const q = snapshot(d, 7 * 60 + 15, [0, 0, 0], { near: 150, middle: 1500 }).filter((x) => x.doing === "queue for bread");
  expect(q.length).toBeGreaterThan(5);
  expect(q.every((x) => x.level !== "inside")).toBe(true);
  // no two in the queue on the same spot; each 0.65 m from the next along the front
  for (const door of new Set(q.map((x) => (x.activity.kind === "stay" ? x.activity.at : "")))) {
    const at = q.filter((x) => x.activity.kind === "stay" && x.activity.at === door).map((x) => x.at[0]).sort((a, b) => a - b);
    for (let k = 1; k < at.length; k++) expect(at[k] - at[k - 1]).toBeCloseTo(0.65, 5);
  }
  // each faces the door: from the bakery's door (facing 0, so the front runs along x) back along the line
  for (const x of q) {
    const door = inside.find((p) => x.activity.kind === "stay" && p.id === x.activity.at)!;
    const toDoor = Math.atan2(door.at[0] - x.at[0], door.at[2] + 1.1 - x.at[2]);
    expect(Math.abs(Math.atan2(Math.sin(x.facing - toDoor), Math.cos(x.facing - toDoor)))).toBeLessThan(0.01);
  }
  // a world's queue line is followed: its first place, then 0.65 m apart along it
  const lined = inside.map((p) => (p.kind === "bakery" ? { ...p, line: { at: [p.at[0], 0, p.at[2] + 2] as V3, along: [0, 0, 1] as V3 } } : p));
  const dl = cityDay(ps, queueRoles, lined, { key: "1917-12-03", weekday: 1 });
  const ql = snapshot(dl, 7 * 60 + 15, [0, 0, 0], { near: 150, middle: 1500 }).filter((x) => x.doing === "queue for bread");
  for (const door of lined.filter((p) => p.line)) {
    const zs = ql.filter((x) => x.activity.kind === "stay" && x.activity.at === door.id).map((x) => x.at[2] - door.at[2]).sort((a, b) => a - b);
    zs.forEach((z, k) => expect(z).toBeCloseTo(2 + 0.65 * k, 5));
  }
  expect(ql.every((x) => Math.abs(x.facing - Math.PI) < 1e-9 || Math.abs(x.facing + Math.PI) < 1e-9)).toBe(true);
});

test("the nearest places of a kind come from a grid, the same as sorting them all", () => {
  const r = rng(91);
  const many: Place[] = Array.from({ length: 3000 }, (_, i) => ({ id: `s${i}`, kind: "shop", at: [r.next() * 4000 - 2000, 0, r.next() * 4000 - 2000] as V3 }));
  const g = new Grid(many, 250);
  for (let t = 0; t < 200; t++) {
    const at: V3 = [r.next() * 5000 - 2500, 0, r.next() * 5000 - 2500];
    const d = (p: Place) => Math.hypot(p.at[0] - at[0], p.at[2] - at[2]);
    const want = [...many].sort((a, b) => d(a) - d(b)).slice(0, 4).map(d);
    expect(g.nearest(at, 4).map(d)).toEqual(want);
    const reach = 50 + r.next() * 600;
    expect(new Set(g.within(at, reach).map((p) => p.id))).toEqual(new Set(many.filter((p) => d(p) <= reach).map((p) => p.id)));
  }
  expect(g.nearest([0, 0, 0], 5000).length).toBe(3000);
});

test("a block with nowhere to happen is not lived: no workplace, no shift; no post, no patrol at the front door", () => {
  const bare = places.filter((p) => p.kind !== "post" && p.kind !== "munitions");
  const ps = populate({ roles, places: bare, folk, mix, seed: 5 });
  const d = cityDay(ps, roles, bare, { key: "1917-12-03", weekday: 1 });
  const workers = ps.filter((x) => x.role === "munitions-worker"), sentries = ps.filter((x) => x.role === "sentry");
  expect(workers.length).toBeGreaterThan(0);
  expect(sentries.length).toBeGreaterThan(0);
  for (const p of workers) expect(p.work).toBeUndefined();
  for (const p of [...workers, ...sentries]) {
    const day = d.days.get(p.id)!;
    expect(day.every((a) => a.kind === "stay" ? a.at === p.home || !["shift", "patrol"].includes(a.what) : true)).toBe(true);
    expect(day.some((a) => a.kind === "stay" && a.what === "patrol")).toBe(false);
  }
});

test("passers-by: as many as the line and the hour say, on it, keeping right, the same each time, moving at a walk", () => {
  const walk: Walk = { id: "w", path: [[0, 0, 0], [200, 0, 0], [200, 0, 100]], perMetre: 0.3, width: 3 };
  const f = { roles, folk, mix, seed: "flow" };
  const at17 = passersBy([walk], 17 * 60, f), at3 = passersBy([walk], 3 * 60, f);
  // 300 m at 0.3 a metre at the busiest hour (5 pm) less those walked off the open ends; a fiftieth of that at 3 am
  expect(at17.length).toBeGreaterThan(65);
  expect(at17.length).toBeLessThan(105);
  expect(at3.length).toBeLessThan(8);
  expect(passersBy([walk], 17 * 60, f)).toEqual(at17);
  for (const p of at17) {
    const onFirst = Math.abs(p.at[2]) <= 1.5 && p.at[0] >= -1e-6 && p.at[0] <= 200 + 1.5;
    const onSecond = Math.abs(p.at[0] - 200) <= 1.5 && p.at[2] >= -1.5 && p.at[2] <= 100;
    expect(onFirst || onSecond).toBe(true);
  }
  // on the first leg (along +x), those walking +x keep to one side and those walking -x to the other
  const leg = at17.filter((p) => p.at[0] < 190 && Math.abs(p.at[2]) < 2);
  const east = leg.filter((p) => Math.sin(p.facing) > 0.5), west = leg.filter((p) => Math.sin(p.facing) < -0.5);
  expect(east.length + west.length).toBe(leg.length);
  expect(east.every((p) => p.at[2] > 0) && west.every((p) => p.at[2] < 0)).toBe(true);
  // a second later each still out has moved about a walking pace along the line
  const later = new Map(passersBy([walk], 17 * 60 + 1 / 60, f).map((p) => [p.id, p]));
  for (const p of leg) {
    const q = later.get(p.id);
    if (!q) continue;
    const d = Math.hypot(q.at[0] - p.at[0], q.at[2] - p.at[2]);
    expect(d).toBeGreaterThan(0.7);
    expect(d).toBeLessThan(1.8);
  }
});

test("passers-by round a block: no one jumps; they come out of doors and go in at doors; a busy leg stays busy", () => {
  // a 100 x 60 block: the avenue side (y = 0, 100 m) busy, the other three quiet; doors every 12 m
  const ring: Walk = {
    id: "b", closed: true, width: 4, perMetre: 0.08,
    path: [[0, 0, 0], [100, 0, 0], [100, 0, 60], [0, 0, 60]],
    legs: [{ to: 100, perMetre: 0.5 }, { to: 160, perMetre: 0.08 }, { to: 260, perMetre: 0.08 }, { to: 320, perMetre: 0.08 }],
    doors: Array.from({ length: 26 }, (_, i) => 6 + i * 12),
  };
  const f = { roles, folk, mix, seed: "ring" };
  let seen = 0, came = 0, went = 0;
  const near = (p: { s: number }) => ring.doors!.some((d) => Math.abs(d - p.s) < 0.15 || Math.abs(Math.abs(d - p.s) - 320) < 0.15);
  for (let t = 0; t < 48; t++) {
    const m = 17 * 60 + t / (24 * 60), now = passersBy([ring], m, f), next = passersBy([ring], m + 1 / (24 * 60), f);
    const a = new Map(now.map((p) => [p.id, p])), b = new Map(next.map((p) => [p.id, p]));
    for (const [id, p] of a) {
      const q = b.get(id);
      if (q) { seen++; expect(Math.hypot(q.at[0] - p.at[0], q.at[2] - p.at[2])).toBeLessThan(0.12); }
      else { went++; }
    }
    for (const [id, q] of b) if (!a.has(id)) { came++; expect(near(q)).toBe(true); }
  }
  expect(seen).toBeGreaterThan(1000);
  expect(came + went).toBeGreaterThan(0);
  // the avenue side holds more a metre than the quiet sides
  const at = passersBy([ring], 17 * 60, f);
  const avenue = at.filter((p) => p.s < 100).length / 100, quiet = at.filter((p) => p.s >= 100).length / 220;
  expect(avenue).toBeGreaterThan(2.5 * quiet);
});

test("passers-by by the hour's mix: the street's people change over the day and no one changes on it", () => {
  const ring: Walk = {
    id: "h", closed: true, width: 4, perMetre: 0.3,
    path: [[0, 0, 0], [100, 0, 0], [100, 0, 60], [0, 0, 60]],
    doors: Array.from({ length: 26 }, (_, i) => 6 + i * 12),
  };
  // the workers on the street until eleven, then the housewives
  const mixAt = (m: number) => (m < 11 * 60 ? { "munitions-worker": 1 } : { housewife: 1 });
  const plain = { roles, folk, mix, seed: "hour" }, hourly = { ...plain, mixAt };
  // about as many out at an hour as without it
  const a = passersBy([ring], 17 * 60, plain).length, b = passersBy([ring], 17 * 60, hourly).length;
  expect(b).toBeGreaterThan(0.75 * a);
  expect(b).toBeLessThan(1.33 * a);
  // well before the change only workers, well after only housewives
  expect(passersBy([ring], 10 * 60 + 30, hourly).every((p) => p.person.role === "munitions-worker")).toBe(true);
  expect(passersBy([ring], 11 * 60 + 30, hourly).every((p) => p.person.role === "housewife")).toBe(true);
  // across the change, a twenty-fourth of a second at a time: anyone still out is the same person a step on; anyone new came out of a door
  const near = (p: { s: number }) => ring.doors!.some((d) => Math.abs(d - p.s) < 0.15 || Math.abs(Math.abs(d - p.s) - 320) < 0.15);
  let seen = 0;
  for (let t = -240; t < 240; t += 4) {
    const m = 11 * 60 + t / 60, now = passersBy([ring], m, hourly), next = new Map(passersBy([ring], m + 1 / (24 * 60), hourly).map((p) => [p.id, p]));
    const was = new Set(now.map((p) => p.id));
    for (const p of now) {
      const q = next.get(p.id);
      if (!q) continue;
      seen++;
      expect(q.person.role).toBe(p.person.role);
      expect(Math.hypot(q.at[0] - p.at[0], q.at[2] - p.at[2])).toBeLessThan(0.12);
    }
    for (const [id, q] of next) if (!was.has(id)) expect(near(q)).toBe(true);
  }
  expect(seen).toBeGreaterThan(500);
  // the same minute asked again gives the same street
  expect(passersBy([ring], 11 * 60 + 2, hourly)).toEqual(passersBy([ring], 11 * 60 + 2, hourly));
});

test("the street's mix by the hour comes from the city's days: who is walking then", () => {
  const people = populate({ roles, places, folk, mix, people: 300, seed: "mix" });
  const d = cityDay(people, roles, places, { key: "1917-12-03", weekday: 1 });
  const at = streetMix(d);
  // the housewives walk to the market at half past ten; at noon the workers are at the Works and the housewives home
  // again, and only the sentries' patrols are walking
  const share = (r: Record<string, number>, k: string) => (r[k] ?? 0) / Object.values(r).reduce((x, y) => x + y, 0);
  expect(share(at(10 * 60 + 30), "housewife")).toBeGreaterThan(0.6);
  expect(share(at(12 * 60), "munitions-worker")).toBeLessThan(0.05);
  expect(share(at(12 * 60), "sentry")).toBeGreaterThan(0.8);
  expect(Object.keys(at(3 * 60)).length).toBeGreaterThan(0);
  // walked through passersBy: the noon street carries the patrols, though the mix is mostly housewives
  const ring: Walk = { id: "m", closed: true, width: 4, perMetre: 0.3, path: [[0, 0, 0], [100, 0, 0], [100, 0, 60], [0, 0, 60]] };
  const noon = passersBy([ring], 12 * 60 + 20, { roles, folk, mix, seed: "mix", mixAt: at });
  expect(noon.length).toBeGreaterThan(20);
  expect(noon.filter((p) => p.person.role === "sentry").length).toBeGreaterThan(0.8 * noon.length);
});
