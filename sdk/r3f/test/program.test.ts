// A plain three.js program run as <Three program> runs it (docs/r3f.feature R3F19, src/input/program.ts): its renderer
// raea's stand-in (vanilla.ts, as raea's bundler resolves a composition's "three"), its loop stepped 1/fps a frame.
import { expect, test } from "bun:test";
import * as THREE from "three";
import { WebGLRenderer } from "../src/input/vanilla";
import { WebGPURenderer } from "../src/input/vanilla-webgpu";
import { drawnOf, endRun, programCanvas, startRun, stepRun } from "../src/input/program";

const FPS = 24;
const canvas = () => programCanvas(640, 360);

test("setAnimationLoop steps once a frame at that frame's time, and the scene it renders is drawn", () => {
  const seen: { scene?: THREE.Scene; cube?: THREE.Mesh; ticks: number; now: number[] } = { ticks: 0, now: [] };
  const run = startRun("loop", (c) => {
    const renderer = new WebGLRenderer({ canvas: c });
    renderer.shadowMap.enabled = true;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    const scene = (seen.scene = new THREE.Scene());
    const camera = new THREE.PerspectiveCamera(50, 16 / 9, 0.1, 100);
    const cube = (seen.cube = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial()));
    scene.add(cube);
    renderer.setAnimationLoop((t) => {
      seen.ticks++;
      seen.now.push(performance.now());
      cube.rotation.y = t / 1000;
      cube.rotation.x += 0.1;
      renderer.render(scene, camera);
    });
  }, canvas());
  stepRun(run, 24, FPS);
  expect(seen.ticks).toBe(25);
  expect(seen.cube!.rotation.y).toBeCloseTo(1);
  expect(seen.cube!.rotation.x).toBeCloseTo(2.5);
  // performance.now() reads the step's time, three's Clock with it
  expect(seen.now[12]).toBeCloseTo(500);
  const d = drawnOf(run)!;
  expect(d.scene).toBe(seen.scene!);
  expect(d.gl.shadowMap.enabled).toBe(true);
  expect(d.gl.toneMapping).toBe(THREE.ACESFilmicToneMapping);
  // drawn again at the same frame: no step
  stepRun(run, 24, FPS);
  expect(seen.ticks).toBe(25);
  endRun(run);
});

test("a requestAnimationFrame loop steps as the page's would, and the page's own comes back after", () => {
  const was = globalThis.requestAnimationFrame;
  const times: number[] = [];
  const run = startRun("raf", () => {
    const renderer = new WebGLRenderer();
    const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera();
    const animate = (t: number) => {
      requestAnimationFrame(animate);
      times.push(t);
      renderer.render(scene, camera);
    };
    requestAnimationFrame(animate);
  }, canvas());
  expect(globalThis.requestAnimationFrame).toBe(was);
  stepRun(run, 10, FPS);
  expect(times.length).toBe(11);
  expect(times[10]).toBeCloseTo((10 * 1000) / FPS);
  expect(drawnOf(run)).not.toBeNull();
});

test("Math.random is the run's own: a program started again draws the same numbers", () => {
  const draw = () => {
    const xs: number[] = [];
    const run = startRun("random", () => {
      const renderer = new WebGLRenderer();
      xs.push(Math.random());
      renderer.setAnimationLoop(() => void xs.push(Math.random()));
    }, canvas());
    stepRun(run, 3, FPS);
    endRun(run);
    return xs;
  };
  const a = draw();
  expect(draw()).toEqual(a);
  expect(new Set(a).size).toBe(a.length);
});

test("ending a run undoes it with what the program returned, and drops its loop", () => {
  let undone = 0, ticks = 0;
  const run = startRun("stop", () => {
    new WebGLRenderer().setAnimationLoop(() => void ticks++);
    return () => void undone++;
  }, canvas());
  stepRun(run, 2, FPS);
  endRun(run);
  expect(undone).toBe(1);
  stepRun(run, 5, FPS);
  expect(ticks).toBe(3);
});

test("a full-screen pass to the screen (three's EffectComposer) is drawn as the scene it rendered", () => {
  const scene = new THREE.Scene();
  const run = startRun("composer", () => {
    const renderer = new WebGLRenderer();
    const target = new THREE.WebGLRenderTarget(64, 64);
    const camera = new THREE.PerspectiveCamera();
    const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), new THREE.MeshBasicMaterial({ map: target.texture }));
    const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    renderer.setAnimationLoop(() => {
      renderer.setRenderTarget(target);
      renderer.render(scene, camera);
      renderer.setRenderTarget(null);
      renderer.render(quad, ortho);
    });
  }, canvas());
  stepRun(run, 0, FPS);
  expect(drawnOf(run)!.scene).toBe(scene);
});

test("three/webgpu's renderer: `await renderer.init()`, then its loop, stepped from frame 0 once it's ready", async () => {
  let ticks = 0;
  const run = startRun("webgpu", async (c) => {
    const renderer = new WebGPURenderer({ canvas: c });
    await renderer.init();
    const scene = new THREE.Scene(), camera = new THREE.PerspectiveCamera();
    renderer.setAnimationLoop(() => {
      ticks++;
      void renderer.renderAsync(scene, camera);
    });
  }, canvas());
  expect(run.waiting).toBeDefined();
  await run.waiting;
  expect(run.waiting).toBeUndefined();
  stepRun(run, 4, FPS);
  expect(ticks).toBe(5);
  expect(drawnOf(run)).not.toBeNull();
});

test("a renderer made outside <Three program> draws nothing and throws nothing", () => {
  const r = new WebGLRenderer();
  r.setAnimationLoop(() => {});
  r.render(new THREE.Scene(), new THREE.PerspectiveCamera());
  r.setSize(100, 100);
  expect(r.domElement).toBeDefined();
});
