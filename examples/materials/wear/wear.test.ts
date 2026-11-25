// The corner house (house.ts) and raea/wear's demo (wear.tsx): every part docs/wear.feature WR7 lists, openings cut
// through the walls (where cracks start), a valid .glb with one material per class, a still for every cause and for
// each of WR9's eight conditions by day and at dusk, and worn paint (docs/examples.feature EX9).
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DEBUG_CAUSES, WEAR_CLASSES, WEAR_CONDITIONS } from "raea/wear";
import { BAYS, CLASSES, DOOR, houseGlb, solids, SOUTH, STOREYS } from "./house";

type V3 = [number, number, number];
const box = (s: V3[]) => [0, 1, 2].map((a) => [Math.min(...s.map((p) => p[a])), Math.max(...s.map((p) => p[a]))]);
const parts = solids();
/** Whether any solid of `cls` holds the point. */
const solidAt = (p: V3, cls = [...parts.keys()]) =>
  cls.some((c) => parts.get(c)!.some((s) => box(s).every(([lo, hi], a) => p[a] > lo + 1e-6 && p[a] < hi - 1e-6)));

describe("the corner house", () => {
  test("its classes are raea/wear's, every one used", () => {
    const names = WEAR_CLASSES.map((c) => c.name);
    for (const c of CLASSES) expect(names).toContain(c);
    for (const c of CLASSES) expect(parts.get(c)!.length).toBeGreaterThan(0);
  });

  test("has every part WR7 names", () => {
    const n = (c: string) => parts.get(c as (typeof CLASSES)[number])!.length;
    // plinth, string course, sills, cornice and consoles, the door's surround; steps; balcony and hatch; gutter and
    // downpipe; the two street plates; the roof
    expect(n("sandstone")).toBeGreaterThan(40);
    expect(n("granite")).toBeGreaterThanOrEqual(3);
    expect(n("iron")).toBeGreaterThan(10);
    expect(n("zinc")).toBeGreaterThanOrEqual(4);
    expect(n("enamel")).toBe(2);
    expect(n("slate")).toBe(2);
    expect(n("copper")).toBe(1);
    // the ground floor rusticated: bands set back 2 cm every 0.4 m up to the string course
    const grooves = parts.get("plaster")!.filter((s) => { const b = box(s); return b[1][0] < 4.2 && Math.abs(b[2][1] + 0.02) < 1e-6; });
    expect(grooves.length).toBeGreaterThan(20);
  });

  test("its windows and door go through the walls, their glass 15 cm in", () => {
    for (const st of STOREYS) {
      for (const x of BAYS.south) {
        if (st === STOREYS[0] && x === DOOR.at) continue;
        const mid: V3 = [x, (st.sill + st.head) / 2 + 0.3, -0.05];
        expect(solidAt(mid, ["plaster"])).toBe(false);
        expect(solidAt([x - 0.3, (st.sill + st.head) / 2 + 0.3, -0.16], ["glass"])).toBe(true);
      }
      for (const z of BAYS.east) expect(solidAt([SOUTH.x1 - 0.05, (st.sill + st.head) / 2 + 0.3, -z], ["plaster"])).toBe(false);
    }
    expect(solidAt([DOOR.at, 1.5, -0.05], ["plaster"])).toBe(false);
    // and the wall between them is solid
    expect(solidAt([(BAYS.south[0] + BAYS.south[1]) / 2, 2.5, -0.25], ["plaster"])).toBe(true);
  });

  test("is a .glb with one material per class, named for it", () => {
    const glb = houseGlb();
    const dv = new DataView(glb.buffer, glb.byteOffset);
    expect(dv.getUint32(0, true)).toBe(0x46546c67);
    expect(dv.getUint32(8, true)).toBe(glb.length);
    const len = dv.getUint32(12, true);
    const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + len)));
    expect(json.materials.map((m: { name: string }) => m.name)).toEqual([...CLASSES]);
    expect(json.meshes[0].primitives).toHaveLength(CLASSES.length);
    expect(houseGlb()).toEqual(glb);
  });
});

describe("the demo", () => {
  const src = readFileSync(join(import.meta.dir, "wear.tsx"), "utf8");
  test("has a still of every cause in false colour", () => {
    const listed = src.match(/const CAUSES: WearCause\[\] = \[([^\]]+)\]/)![1].match(/"(\w+)"/g)!.map((s) => s.slice(1, -1));
    expect(listed.sort()).toEqual(Object.keys(DEBUG_CAUSES).sort());
  });
  test("draws worn paint with no bake, as <WearExtension> (EX9), from light to heavy", () => {
    expect(src).toContain("paint: () => <Paint />");
    const amounts = [...src.matchAll(/shape: "\w+", wear: \{ amount: ([\d.]+)/g)].map((m) => Number(m[1]));
    expect(amounts).toEqual([0.5, 0.8, 1]);
    expect(src).toContain("extension={<WearExtension {...wear} />}");
  });
  test("draws each of WR9's eight conditions by day and at dusk", () => {
    expect(Object.keys(WEAR_CONDITIONS)).toHaveLength(8);
    expect(src).toContain("CONDITIONS.flatMap((c) => [");
    expect(src).toContain("[`${c}-dusk`, () => <House look={{ ...old, weather: WEAR_CONDITIONS[c] }} sky={SKY[c]} dusk />]");
  });
});
