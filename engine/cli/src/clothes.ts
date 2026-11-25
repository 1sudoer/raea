// `raea clothes`: clothes for SOMA's body (raea docs/character.feature CH45c). make sews a garment: a pattern from
// words (the garment model's first guess) or from the plain base, any setting changed by path, cut to SOMA, draped
// and baked; cover makes one by procedure on the skin; wear puts garments on a body. The work is the host's (op
// "clothes": engine/cli/src/wardrobe.ts and sewn.ts on the native raea); this file reads the words and says what
// was done.

type Ask = (op: string, args: Record<string, unknown>) => any;

/** the body's parts a garment made by procedure covers (tools/clothes/garment.py's PARTS) */
export const PARTS = ["chest", "foot", "foreArm", "hand", "head", "hips", "neck", "shin", "thigh", "toes", "torso", "upperArm", "waist"];

export const CLOTHES_USAGE = `raea clothes   (the project's garments: assets/clothes/NAME.json, their files in assets/clothes/NAME/)
raea clothes make NAME ["WHAT IT IS"] [--plain] [--set PATH=VALUE,...] [--colour #RRGGBB]   (a sewn garment: its pattern from the words, or from the plain base with --plain or no words; each PATH=VALUE changes one setting)
raea clothes settings   (a sewn garment's settings: each path, its value in the plain base, what it may be)
raea clothes cover NAME --parts PART[:SHARE],... --colour #RRGGBB [--stand MM] [--over UNDER] [--hard [--sole MM] [--sole-colour #RRGGBB] [--panels #RRGGBB,...]]   (by procedure on the skin; --hard: footwear, on a last round the foot with a sole under it: ${PARTS.join(", ")}; a limb's share is how far along it from the end that joins the rest: upperArm,foreArm:0.5 from the elbow, foot,shin:0.4 from the ankle)
raea clothes wear OUT_DIR GARMENT... [--body DIR] [--head HEAD_DIR]   (GARMENT: a garment's name or its .json, innermost first; DIR: a SOMA body folder, SOMA's own unless said; HEAD_DIR: the head that joins a headless body, whose skin is graded to it at the collar)
`;

const flagOf = (args: string[], name: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
/** the flags that take no value */
const BARE = new Set(["--plain", "--hard"]);
/** the words that aren't flags or a flag's value */
const plain = (args: string[]) => args.filter((a, i) => !a.startsWith("--") && (!args[i - 1]?.startsWith("--") || BARE.has(args[i - 1])));
const NAME = /^[a-z0-9][a-z0-9_-]*$/i;

export function clothesVerb(ask: Ask, args: string[]): string {
  const q = (op: string, a: Record<string, unknown> = {}) => ask("clothes", { op, ...a });
  const [sub, ...rest] = args;
  switch (sub) {
    case undefined:
    case "list": {
      const gs: { name: string; layer: number; vertices?: number; over?: string; sewn?: string }[] = q("list").garments;
      if (!gs.length) return 'No garments yet. raea clothes make NAME "what it is" sews one.\n';
      return gs.map((g) => (g.sewn !== undefined ? `${g.name}  sewn: ${g.sewn}` : `${g.name}  layer ${g.layer}, ${g.vertices} of SOMA's vertices${g.over ? `, over ${g.over}` : ""}`)).join("\n") + "\n";
    }
    case "make": {
      const plainBase = rest.includes("--plain"), [name, words, ...more] = plain(rest.filter((x) => x !== "--plain"));
      if (!name || more.length) throw new Error(`raea clothes make NAME ["WHAT IT IS"] [--plain] [--set PATH=VALUE,...] [--colour #RRGGBB]\n${CLOTHES_USAGE}`);
      if (!NAME.test(name)) throw new Error(`${name}: a garment's name is letters, digits, - and _ (it names its file)`);
      const set = flagOf(rest, "--set")?.split(",").map((x) => x.trim()).filter(Boolean) ?? [], colour = flagOf(rest, "--colour");
      for (const x of set) if (!/^[a-z][a-z0-9_.-]*=[^=]+$/i.test(x)) throw new Error(`--set ${x}: a setting is PATH=VALUE, sleeve.length=0.3 (raea clothes settings lists them)`);
      if (colour !== undefined && !/^#?[0-9a-f]{6}$/i.test(colour)) throw new Error(`--colour ${colour}: a colour is #RRGGBB`);
      if (!words && !plainBase && !set.length) throw new Error(`say what ${name} is, or make it from the plain base: --plain, --set PATH=VALUE,...\n${CLOTHES_USAGE}`);
      return q("make", { name, words: plainBase ? undefined : words, set, plain: plainBase || !words, colour: colour === undefined ? undefined : colour.startsWith("#") ? colour : `#${colour}` }).said;
    }
    case "settings":
      if (rest.length) throw new Error(`raea clothes settings takes nothing after it\n${CLOTHES_USAGE}`);
      return q("settings").said;
    case "cover": {
      const [name, ...more] = plain(rest);
      const parts = flagOf(rest, "--parts"), colour = flagOf(rest, "--colour"), over = flagOf(rest, "--over"), mm = Number(flagOf(rest, "--stand") ?? 6);
      if (!name || more.length || !parts || !colour) throw new Error(`raea clothes cover NAME --parts PART[:SHARE],... --colour #RRGGBB [--stand MM] [--over UNDER]\n${CLOTHES_USAGE}`);
      if (!NAME.test(name)) throw new Error(`${name}: a garment's name is letters, digits, - and _ (it names its file)`);
      for (const p of parts.split(",")) {
        const [part, share] = p.trim().split(":");
        if (!PARTS.includes(part)) throw new Error(`--parts: no part ${part}; the parts are ${PARTS.join(", ")}`);
        if (share !== undefined && !(Number(share) > 0 && Number(share) <= 1)) throw new Error(`--parts ${p}: a share is how far along the limb, above 0 and at most 1`);
      }
      if (!/^#?[0-9a-f]{6}$/i.test(colour)) throw new Error(`--colour ${colour}: a colour is #RRGGBB`);
      if (!(mm > 0 && mm <= 80)) throw new Error("--stand takes how far the cloth stands off the skin, in millimetres (above 0, at most 80)");
      if (over !== undefined && !NAME.test(over)) throw new Error(`--over ${over}: name a garment already made (raea clothes lists them)`);
      // a hard piece (footwear): its own shape on a last round the foot, a sole under it, panels in their colours
      const hard = rest.includes("--hard"), sole = flagOf(rest, "--sole"), soleColour = flagOf(rest, "--sole-colour"), panels = flagOf(rest, "--panels");
      if (!hard && (sole ?? soleColour ?? panels) !== undefined) throw new Error("--sole, --sole-colour and --panels are a hard piece's: add --hard");
      if (sole !== undefined && !(Number(sole) >= 0 && Number(sole) <= 60)) throw new Error("--sole takes the sole's thickness in millimetres (0 to 60)");
      for (const c of [soleColour, ...(panels?.split(",") ?? [])]) if (c !== undefined && !/^#?[0-9a-f]{6}$/i.test(c.trim())) throw new Error(`${c}: a colour is #RRGGBB`);
      const hex = (c: string) => (c.trim().startsWith("#") ? c.trim() : `#${c.trim()}`);
      return q("cover", { name, parts: parts.replace(/\s+/g, ""), colour: colour.startsWith("#") ? colour : `#${colour}`, stand: mm / 1000, over,
        ...(hard ? { hard: { sole: sole === undefined ? undefined : Number(sole), soleColour: soleColour && hex(soleColour), panels: panels?.split(",").map(hex) } } : {}) }).said;
    }
    case "wear": {
      const [out, ...garments] = plain(rest);
      if (!out || !garments.length) throw new Error(`raea clothes wear OUT_DIR GARMENT... [--body DIR] [--head HEAD_DIR]\n${CLOTHES_USAGE}`);
      return q("wear", { out, garments, body: flagOf(rest, "--body"), head: flagOf(rest, "--head") }).said;
    }
    default:
      throw new Error(CLOTHES_USAGE);
  }
}
