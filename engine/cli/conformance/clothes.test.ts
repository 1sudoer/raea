// `raea clothes`: what the command sends the host for a garment sewn from words or settings, one made by procedure,
// or garments put on a body (docs/character.feature CH45c), and what it refuses before anything is made. The host
// (src/wardrobe.ts and sewn.ts, which run the sewing tools and Blender) is faked: this is the command's side.
import { describe, expect, test } from "bun:test";
import { clothesVerb } from "../src/clothes.ts";

/** clothesVerb with a host that keeps each op it was asked and answers as the host does */
function run(args: string[], garments: { name: string; layer: number; vertices?: number; over?: string; sewn?: string }[] = []) {
  const ops: Record<string, unknown>[] = [];
  const ask = (op: string, b: Record<string, unknown>) => {
    expect(op).toBe("clothes");
    ops.push(b);
    return b.op === "list" ? { garments } : { said: `did ${b.op}\n` };
  };
  return { out: clothesVerb(ask, args), ops };
}

describe("raea clothes", () => {
  test("alone, it lists the project's garments, and says how to make one when there are none", () => {
    expect(run([]).out).toContain('raea clothes make NAME "what it is"');
    const { out } = run(["list"], [{ name: "shorts", layer: 0, vertices: 769 }, { name: "bomber", layer: 1, vertices: 6120, over: "shorts" }, { name: "trousers", layer: 0, sewn: "trousers, a waistband" }]);
    expect(out).toBe("shorts  layer 0, 769 of SOMA's vertices\nbomber  layer 1, 6120 of SOMA's vertices, over shorts\ntrousers  sewn: trousers, a waistband\n");
  });

  test("make sends the name, the words or the plain base, the settings changed and the colour", () => {
    const { ops } = run(["make", "hoodie", "a hooded sweatshirt", "--set", "sleeve.length=0.9, shirt.length=1.6", "--colour", "3f5a3c"]);
    expect(ops).toEqual([{ op: "make", name: "hoodie", words: "a hooded sweatshirt", set: ["sleeve.length=0.9", "shirt.length=1.6"], plain: false, colour: "#3f5a3c" }]);
    expect(run(["make", "tee", "--plain"]).ops[0]).toEqual({ op: "make", name: "tee", words: undefined, set: [], plain: true, colour: undefined });
    expect(run(["make", "trousers", "--set", "meta.upper=none,meta.bottom=Pants"]).ops[0]).toMatchObject({ words: undefined, plain: true, set: ["meta.upper=none", "meta.bottom=Pants"] });
    expect(run(["make", "tee", "--plain", "a white tee"]).ops[0]).toMatchObject({ words: undefined, plain: true });
    expect(run(["settings"]).ops).toEqual([{ op: "settings" }]);
  });

  test("cover sends the parts, the colour and the stand-off in metres, for a garment with no picture", () => {
    const { ops } = run(["cover", "shorts", "--parts", "hips, thigh:0.5", "--colour", "1f2a44", "--stand", "8", "--over", "pants"]);
    expect(ops).toEqual([{ op: "cover", name: "shorts", parts: "hips,thigh:0.5", colour: "#1f2a44", stand: 0.008, over: "pants" }]);
    expect(run(["cover", "tee", "--parts", "chest,torso,waist,upperArm:0.4", "--colour", "#ffffff"]).ops[0]).toMatchObject({ stand: 0.006, over: undefined });
  });

  test("wear sends the folder, the garments innermost first and the body", () => {
    const { ops } = run(["wear", "assets/characters/dressed", "tee-jeans", "bomber", "--body", "assets/characters/soma-headless"]);
    expect(ops).toEqual([{ op: "wear", out: "assets/characters/dressed", garments: ["tee-jeans", "bomber"], body: "assets/characters/soma-headless" }]);
    expect(run(["wear", "out", "tee", "--body", "headless", "--head", "assets/characters/heads/real"]).ops[0]).toMatchObject({ body: "headless", head: "assets/characters/heads/real" });
  });

  test("what doesn't fit is said before anything is drawn", () => {
    const refused = (args: string[], says: string) => expect(() => run(args)).toThrow(says);
    refused(["make", "tee"], "say what tee is, or make it from the plain base");
    refused(["make", "tee", "a white tee", "and more"], 'raea clothes make NAME ["WHAT IT IS"]');
    refused(["make", "../tee", "a white tee"], "a garment's name is letters, digits, - and _");
    refused(["make", "tee", "a white tee", "--set", "sleeve length 3"], "--set sleeve length 3: a setting is PATH=VALUE");
    refused(["make", "tee", "a white tee", "--colour", "white"], "--colour white: a colour is #RRGGBB");
    refused(["settings", "tee"], "raea clothes settings takes nothing after it");
    refused(["cover", "tee", "--parts", "chest"], "raea clothes cover NAME --parts");
    refused(["cover", "tee", "--parts", "belly", "--colour", "#ffffff"], "--parts: no part belly");
    refused(["cover", "tee", "--parts", "shin:1.5", "--colour", "#ffffff"], "a share is how far along the limb");
    refused(["cover", "tee", "--parts", "chest", "--colour", "white"], "--colour white: a colour is #RRGGBB");
    refused(["cover", "tee", "--parts", "chest", "--colour", "#ffffff", "--stand", "200"], "--stand takes how far the cloth stands off");
    refused(["wear", "out"], "raea clothes wear OUT_DIR GARMENT...");
    refused(["dress"], "raea clothes make NAME");
  });
});
