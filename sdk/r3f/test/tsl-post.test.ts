// three/webgpu's PostProcessing as raea's frame effects (src/input/tsl-post.ts, docs/r3f.feature R3F25): a plain
// program's pipeline drawn by the stand-in renderer, its graph read for the scene pass and the nodes after it.
import { expect, test } from "bun:test";
import * as THREE from "three";
import * as TSLTyped from "three/tsl";
import * as BloomM from "three/examples/jsm/tsl/display/BloomNode.js";
import * as FilmM from "three/examples/jsm/tsl/display/FilmNode.js";
import * as FXAAM from "three/examples/jsm/tsl/display/FXAANode.js";
import * as SepiaM from "three/examples/jsm/tsl/display/Sepia.js";
import * as OutlineM from "three/examples/jsm/tsl/display/OutlineNode.js";
import { composerEffects, postParts } from "../src/input/post";
import { drawnOf, programCanvas, startRun, stepRun } from "../src/input/program";
import { PostProcessing, WebGPURenderer } from "../src/input/vanilla-webgpu";
import type { Part } from "../src/parts";

const FPS = 24;
// untyped: @types/three's chained TSL types make tsc take minutes and gigabytes
type Fn = (...a: unknown[]) => any;
const { pass, uniform, renderOutput } = TSLTyped as unknown as Record<string, Fn>;
const [{ bloom }, { film }, { fxaa }, { sepia }, { outline }] = [BloomM, FilmM, FXAAM, SepiaM, OutlineM] as unknown as Record<string, Fn>[];

/** a program drawing `scene` through the PostProcessing `chain` makes, stepped to frame 2 */
async function piped(chain: (scenePass: any, pp: PostProcessing) => void, tone: THREE.ToneMapping = THREE.ACESFilmicToneMapping) {
  const scene = new THREE.Scene();
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicMaterial()));
  const camera = new THREE.PerspectiveCamera(45, 16 / 9, 0.1, 100);
  const run = startRun(`pp-${Math.random()}`, async (c) => {
    const renderer = new WebGPURenderer({ canvas: c });
    await renderer.init();
    renderer.setSize(640, 360);
    renderer.toneMapping = tone;
    renderer.toneMappingExposure = 1.5;
    const pp = new PostProcessing(renderer);
    chain(pass(scene, camera), pp);
    renderer.setAnimationLoop(() => pp.render());
  }, programCanvas(640, 360));
  await run.waiting;
  stepRun(run, 2, FPS);
  return { run, scene, camera };
}

const of = (parts: Part[], type: string) => parts.filter((p) => p.type === type);

test("the scene its pass draws is the frame, and bloom, film and the output's tone are raea's", async () => {
  const strength = uniform(0.8);
  const { run, scene, camera } = await piped((sp, pp) => {
    const color = sp.getTextureNode("output");
    pp.outputNode = film(color.add(bloom(color, strength, 0.2, 0.6)), uniform(0.4));
  });
  const d = drawnOf(run)!;
  expect(d.scene).toBe(scene);
  expect(d.camera).toBe(camera);
  const found = composerEffects(scene);
  expect(found.found).toBe(true);
  const { parts, tone } = postParts(found.effects);
  expect(of(parts, "Bloom")[0].props).toEqual({ intensity: 3.2, threshold: 0.6, additive: true });
  const fp = of(parts, "FramePass");
  expect(fp.length).toBe(1);
  expect((fp[0].props as { uniforms: Record<string, number> }).uniforms.intensity).toBeCloseTo(0.4);
  // effects before the output's tone mapping run on the linear frame
  expect((fp[0].props as { at: string }).at).toBe("before-tonemap");
  expect(tone!.props).toEqual({ mode: "aces" });
  // three's exposure and its ACES's 1/0.6, every pixel before the curve
  expect(of(parts, "Grade")[0].props.exposure as number).toBeCloseTo(Math.log2(1.5 / 0.6));
});

test("an effect after renderOutput() runs on the tone-mapped frame; sepia() is known by its weights", async () => {
  const { scene } = await piped((sp, pp) => {
    pp.outputColorTransform = false;
    pp.outputNode = fxaa(sepia(renderOutput(sp)));
  }, THREE.AgXToneMapping);
  const { parts, tone } = postParts(composerEffects(scene).effects);
  expect(tone!.props).toEqual({ mode: "agx" });
  expect(of(parts, "AntiAliasing")[0].props).toEqual({ mode: "fxaa" });
  const fp = of(parts, "FramePass");
  expect(fp.length).toBe(1);
  expect((fp[0].props as { at: string; wgsl: string }).at).toBe("after-tonemap");
  expect((fp[0].props as { wgsl: string }).wgsl).toContain("0.393");
});

test("a node raea has nothing for is named once and left out, and the frame still draws", async () => {
  const warn = console.warn;
  const said: string[] = [];
  console.warn = (s: string) => said.push(s);
  try {
    const { run, scene } = await piped((sp, pp) => {
      const o = outline(sp.scene as THREE.Scene, sp.camera as THREE.Camera, { selectedObjects: [] });
      pp.outputNode = sp.add(o);
    });
    expect(drawnOf(run)!.scene).toBe(scene);
    postParts(composerEffects(scene).effects);
  } finally {
    console.warn = warn;
  }
  expect(said.some((s) => /Outline node/.test(s))).toBe(true);
});

test("every post node three ships is named, with what raea draws for it", async () => {
  const { TSL_POST } = await import("../src/input/kinds");
  const dir = new URL("../node_modules/three/examples/jsm/tsl/display/", import.meta.url).pathname;
  const nodes = [...new Bun.Glob("*Node.js").scanSync(dir)].map((f) => f.replace(/\.js$/, ""));
  expect(nodes.length).toBeGreaterThan(20);
  expect(nodes.filter((k) => !TSL_POST[k])).toEqual([]);
});
