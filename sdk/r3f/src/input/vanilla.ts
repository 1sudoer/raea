// "three" as a composition's own code imports it in raea (docs/r3f.feature R3F19): three itself, its WebGLRenderer a
// stand-in that draws nothing and records what it's asked to render into the <Three program> running it (program.ts).
// raea's bundler resolves the composition's `import "three"` here (engine/bundle edit.rs); three's own modules, its
// addons and the SDK's import three as it is.
import type * as THREE from "three";
import { standIn } from "./program";

export * from "three";

class StandIn {
  constructor(params?: { canvas?: unknown }) {
    // the stand-in in its place: `new WebGLRenderer()` is what the constructor returns
    return standIn(params, false) as unknown as StandIn;
  }
}

export const WebGLRenderer = StandIn as unknown as typeof THREE.WebGLRenderer;
export type WebGLRenderer = THREE.WebGLRenderer;
