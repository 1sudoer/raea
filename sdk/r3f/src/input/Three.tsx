// <Three program={fn}>: a plain three.js program drawn by raea (docs/r3f.feature R3F19). `fn` is a single-file demo's
// script as a function: it makes its renderer (`new THREE.WebGLRenderer({ canvas })`, three/webgpu's WebGPURenderer, or
// its own domElement appended to document.body), its scene and camera, and renders from setAnimationLoop or
// requestAnimationFrame. raea's bundler gives it a renderer that draws nothing and records what it renders
// (program.ts); each frame that scene becomes raea's parts as <R3F>'s does (scene.ts), drawn by raea's engine. Its loop
// steps 1/fps a frame from the composition's frame, never the wall clock, and a frame before the last runs the program
// again from the start, so a render equals its play. It needs three alone: no React Three Fiber.
import { useEffect, useId, useMemo } from "react";
import { AbsoluteFill, useCurrentFrame, useVideoConfig } from "raea";
import type * as THREE from "three";
import { useHold, useWake } from "./hold";
import { drawnOf, endRun, programCanvas, startRun, stepRun, type Program, type Run } from "./program";
import { convert } from "./scene";
import { standInImages } from "./texture";
import { standInPMREM } from "./pmrem";
import { elementsOf } from "../parts";

standInImages();
standInPMREM();

export interface ThreeProps {
  /** the program: given the canvas to draw on, it may return a function that undoes it (or a promise: an async program
   *  is waited for, the frame held, then stepped from frame 0) */
  program: Program;
  /** a name of its own when one composition has more than one program that come and go */
  name?: string;
}

const runs = new Map<string, Run>();

function end(key: string) {
  const r = runs.get(key);
  if (!r) return;
  runs.delete(key);
  endRun(r);
}

/** a plain three.js program, drawn by raea */
export function Three({ program, name }: ThreeProps) {
  const frame = useCurrentFrame();
  const { fps, width, height } = useVideoConfig();
  useWake();
  const id = useId();
  const key = name ?? id;
  const canvas = useMemo(() => programCanvas(width, height), [width, height]);
  let run = runs.get(key);
  // another program, another canvas, or a frame before the last (a seek back): from the start
  if (run && (run.source !== String(program) || run.canvas !== canvas || (!run.waiting && frame < run.frame))) {
    end(key);
    run = undefined;
  }
  if (!run) runs.set(key, (run = startRun(key, program, canvas)));
  useHold(run.waiting);
  useEffect(() => () => end(key), [key]);
  // what it threw is the composition's: the render is refused, naming it (as <R3F>'s is, #131)
  if (run.failed) {
    const { error } = run.failed;
    end(key);
    throw error;
  }
  if (run.waiting) return null;
  try {
    stepRun(run, frame, fps);
  } catch (e) {
    end(key);
    throw e;
  }
  const d = drawnOf(run);
  if (!d) return null;
  const out = convert(d.scene as THREE.Scene, d.camera, d.gl, height, run.now / 1000, 1 / fps);
  const drawn = elementsOf(out.parts, "three.");
  return out.background ? <AbsoluteFill style={{ background: out.background }}>{drawn}</AbsoluteFill> : <>{drawn}</>;
}
