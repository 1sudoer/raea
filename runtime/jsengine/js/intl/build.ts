// QuickJS's Intl, packed (docs/ports.feature PT0b): `bun install --frozen-lockfile && bun build.ts` here writes
// ../intl.pack, which raea-runtime-jsengine embeds (src/quickjs.rs) and loader.js reads. QuickJS has no Intl, so a
// composition that formatted a number threw "Intl is not defined" on Windows and Linux (#169) while JavaScriptCore,
// the Mac's, formatted it. The pack is entries of [name length u16][name][length u32][raw deflate], little-endian:
// "core" (core.js bundled: FormatJS's polyfills, English, the common time zones), "index" (the JSON list of the other
// locales) and one entry per locale, its data for each polyfill that has some. Uncompressed the 37 locales are 20 MB.
import { deflateRawSync } from "node:zlib";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const here = import.meta.dir;
/** the locales besides English: the languages most of raea's people write in, and the regions that format apart */
export const LOCALES = ["en-GB", "en-AU", "en-CA", "en-IN", "de", "fr", "fr-CA", "es", "es-419", "es-MX", "it", "pt", "pt-PT", "nl", "sv", "da", "nb", "fi", "pl", "cs", "sk", "hu", "ro", "el", "tr", "ru", "uk", "he", "ar", "hi", "bn", "th", "id", "ms", "vi", "ja", "ko", "zh", "zh-Hant", "fil"];
const DATA = ["intl-pluralrules", "intl-numberformat", "intl-datetimeformat", "intl-relativetimeformat", "intl-listformat"];

const built = await Bun.build({ entrypoints: [join(here, "core.js")], minify: true, format: "iife", target: "browser" });
if (!built.success) throw new Error(built.logs.join("\n"));
const licence = readFileSync(join(here, "LICENSE-FormatJS.md"), "utf8").trim().split("\n").map((l) => ` * ${l}`.trimEnd()).join("\n");
const core = `/*\n * FormatJS's Intl polyfills, packed for raea's QuickJS (runtime/jsengine/js/intl):\n${licence}\n */\n${await built.outputs[0].text()}`;

const entries: [string, string][] = [["core", core], ["index", JSON.stringify(LOCALES)]];
for (const l of LOCALES) {
  const parts = DATA.map((p) => join(here, "node_modules/@formatjs", p, "locale-data", `${l}.js`)).filter(existsSync);
  if (parts.length < 4) throw new Error(`${l}: data for ${parts.length} of the ${DATA.length} polyfills`);
  entries.push([l, parts.map((f) => readFileSync(f, "utf8")).join(";\n")]);
}
const chunks: Buffer[] = [];
for (const [name, src] of entries) {
  const n = Buffer.from(name), z = deflateRawSync(Buffer.from(src), { level: 9 });
  const head = Buffer.alloc(6 + n.length);
  head.writeUInt16LE(n.length, 0);
  n.copy(head, 2);
  head.writeUInt32LE(z.length, 2 + n.length);
  chunks.push(head, z);
}
const pack = Buffer.concat(chunks);
writeFileSync(join(here, "..", "intl.pack"), pack);
console.log(`intl.pack: ${entries.length} entries, ${(pack.length / 1024).toFixed(0)} KB (core ${(core.length / 1024).toFixed(0)} KB before deflate)`);
