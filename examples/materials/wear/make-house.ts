// Writes the corner house (house.ts) to media/house.glb, where wear.tsx's staticFile reads it.
//   bun examples/materials/wear/make-house.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { houseGlb } from "./house";

const dir = join(import.meta.dir, "media");
mkdirSync(dir, { recursive: true });
const glb = houseGlb();
writeFileSync(join(dir, "house.glb"), glb);
console.log(`make-house: ${join(dir, "house.glb")} (${glb.length} bytes)`);
