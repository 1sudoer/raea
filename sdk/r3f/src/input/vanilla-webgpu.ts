// "three/webgpu" as a composition's own code imports it in raea (docs/r3f.feature R3F19): three's WebGPU build, its
// WebGPURenderer a stand-in that draws nothing and records what it's asked to render into the <Three program> running
// it (program.ts). Its node materials are compiled by tsl.ts against three's own renderer, which the SDK imports as it is.
import type * as WEBGPU from "three/webgpu";
import { standIn } from "./program";

export * from "three/webgpu";

class StandIn {
  constructor(params?: { canvas?: unknown }) {
    return standIn(params, true) as unknown as StandIn;
  }
}

export const WebGPURenderer = StandIn as unknown as typeof WEBGPU.WebGPURenderer;
export type WebGPURenderer = WEBGPU.WebGPURenderer;
