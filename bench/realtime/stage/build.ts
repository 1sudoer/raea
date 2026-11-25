// The stage page (page.tsx) bundled for raea-page-host as ui-parity bundles a case's Bevy side
// (sdk/ui/parity/build.ts): @raea/ui from the tree, React DOM swapped for its adapter, parity's one React.
//   bun bench/realtime/stage/build.ts OUT.js
import { join } from "node:path";

const here = import.meta.dir;
const parity = join(here, "../../../sdk/ui/parity");
const mods = join(parity, "node_modules");
const ui = join(here, "../../../sdk/ui/src");
const out = process.argv[2];
if (!out) {
  console.error("usage: bun build.ts OUT.js");
  process.exit(2);
}
const b = await Bun.build({
  entrypoints: [join(here, "page.tsx")],
  target: "browser",
  format: "iife",
  minify: { syntax: true },
  define: { "process.env.NODE_ENV": '"production"' },
  plugins: [
    {
      name: "rt-stage",
      setup(b) {
        b.onResolve({ filter: /^react-dom(\/client)?$/ }, () => ({ path: join(parity, "probe/react-dom-bevy.ts") }));
        b.onResolve({ filter: /^@raea\/ui(\/.*)?$/ }, (a) => ({ path: join(ui, (a.path.slice("@raea/ui/".length) || "index") + ".ts") }));
        b.onResolve({ filter: /^(react|react-dom|react-reconciler|scheduler)(\/.*)?$/ }, (a) => ({ path: Bun.resolveSync(a.path, mods + "/") }));
      },
    },
  ],
});
if (!b.success) {
  console.error(b.logs.map((l) => String(l)).join("\n"));
  process.exit(1);
}
await Bun.write(out, await b.outputs[0].text());
