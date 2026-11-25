// @react-three/postprocessing's effects as raea's (src/input/post.ts, docs/r3f.feature R3F12): pmndrs postprocessing's
// own classes, read the way they're found under an <EffectComposer>.
import { describe, expect, test } from "bun:test";
import { BlendFunction, BloomEffect, ChromaticAberrationEffect, DepthEffect, DepthOfFieldEffect, EffectComposer, EffectPass, GlitchEffect, HueSaturationEffect, LUT3DEffect, NoiseEffect, RenderPass, ScanlineEffect, SMAAEffect, ToneMappingEffect, ToneMappingMode, VignetteEffect } from "postprocessing";
import * as THREE from "three";
import { postParts } from "../src/input/post";
import { standInRenderer } from "../src/input/session";
import { EFFECTS } from "../src/input/kinds";

describe("<EffectComposer> → raea's frame effects", () => {
  test("bloom, colour, tone and anti-aliasing become raea's", () => {
    const { parts, tone } = postParts([
      new BloomEffect({ intensity: 1.5, luminanceThreshold: 0.6 }),
      new VignetteEffect({ offset: 0.3, darkness: 0.7 }),
      new HueSaturationEffect({ saturation: -0.3 }),
      new ChromaticAberrationEffect({ offset: new THREE.Vector2(0.003, 0.004), radialModulation: false, modulationOffset: 0 }),
      new ToneMappingEffect({ mode: ToneMappingMode.AGX }),
      new SMAAEffect(),
    ]);
    expect(parts.find((p) => p.type === "Bloom")!.props).toEqual({ intensity: 1.5, threshold: 0.6 });
    const grade = parts.find((p) => p.type === "Grade")!.props as Record<string, any>;
    expect(grade.vignette.intensity).toBeCloseTo(0.7);
    expect(grade.saturation).toBeCloseTo(0.7);
    expect(grade.aberration).toBeCloseTo(0.005);
    expect(parts.find((p) => p.type === "AntiAliasing")!.props).toEqual({ mode: "smaa" });
    expect(tone!.props).toEqual({ mode: "agx" });
  });

  test("ACES carries three's 1/0.6 as the Grade's exposure, every pixel before the curve, not the camera's", () => {
    const { tone, parts } = postParts([new ToneMappingEffect({ mode: ToneMappingMode.ACES_FILMIC })]);
    expect(tone!.props).toEqual({ mode: "aces" });
    const grade = parts.find((p) => p.type === "Grade")!;
    expect(grade.props.exposure as number).toBeCloseTo(Math.log2(1 / 0.6));
  });

  test("one raea has nothing for is named and left out", () => {
    const warn = console.warn;
    const said: string[] = [];
    console.warn = (s: string) => said.push(s);
    try {
      expect(postParts([new DepthEffect()]).parts).toEqual([]);
      expect(said.join("\n")).toContain("DepthEffect");
    } finally {
      console.warn = warn;
    }
  });

  test("noise and scanlines are frame passes: their shader, their uniforms, their blend (R3F25)", () => {
    const scan = new ScanlineEffect({ density: 1.5 });
    scan.setSize(640, 360);
    const { parts } = postParts([new NoiseEffect({ premultiply: true }), scan, new NoiseEffect({ blendFunction: BlendFunction.SKIP })]);
    expect(parts.map((p) => p.type)).toEqual(["FramePass", "FramePass"]);
    const [noise, lines] = parts.map((p) => p.props as { wgsl: string; uniforms: Record<string, number>; at: string });
    expect(noise.wgsl).toContain("fn shade(");
    expect(noise.uniforms.premultiply).toBe(1);
    expect(noise.uniforms.blend).toBe(BlendFunction.SCREEN);
    expect(lines.uniforms.count).toBe(540);
    expect(lines.uniforms.blend).toBe(BlendFunction.OVERLAY);
    expect(lines.at).toBe("before-tonemap");
  });

  test("a frame pass before an ACES <ToneMapping> reads the frame as three has it, the exposure put on after it", () => {
    const { parts } = postParts([new NoiseEffect(), new ToneMappingEffect({ mode: ToneMappingMode.ACES_FILMIC }), new NoiseEffect()]);
    const [before, after] = parts.filter((p) => p.type === "FramePass").map((p) => p.props as { uniforms: Record<string, number>; at: string; wgsl: string });
    expect(before.at).toBe("before-tonemap");
    expect(before.wgsl).not.toContain("u_gain");
    expect(after.at).toBe("after-tonemap");
  });

  test("a glitch reads the frame at its moved uv and its perturbation map; a 3D LUT is a strip of slices (R3F25)", () => {
    const [glitch] = postParts([new GlitchEffect()]).parts;
    const g = glitch.props as { uniforms: Record<string, unknown>; textures: Record<string, { src: string }>; wgsl: string };
    // a mainUv effect isn't blended: its output is the frame where its uv lands
    expect(g.uniforms.blend).toBe(30);
    expect(g.textures.perturb.src).toStartWith("data:image/png;base64,");
    expect(g.wgsl).toContain("o = pp_frame(q);");
    const n = 2;
    const data = new Float32Array(n * n * n * 4).map((_, i) => (i % 4 === 3 ? 1 : (i % 4) / 3));
    const lut = new THREE.Data3DTexture(data, n, n, n);
    const [pass] = postParts([new LUT3DEffect(lut)]).parts;
    const l = pass.props as { uniforms: Record<string, unknown>; textures: Record<string, { src: string }>; wgsl: string };
    expect(l.uniforms.lut).toEqual([n, 1 / (n * n), 1 / n, 1]);
    expect(l.textures.lut.src).toStartWith("data:image/png;base64,");
    // read in sRGB, as the effect asks, and taken back
    expect(l.wgsl).toContain("pp_srgb(f.rgb)");
    expect(l.wgsl).toContain("pp_linear(b.rgb)");
  });

  test("<TiltShift2>'s effect, also named TiltShiftEffect, is told apart by its taper", () => {
    const uniforms = new Map<string, { value: unknown }>([["blur", { value: 0.15 }], ["taper", { value: 0.5 }], ["start", { value: [0.5, 0] }], ["end", { value: [0.5, 1] }], ["direction", { value: [1, 1] }], ["samples", { value: 10 }]]);
    const [pass] = postParts([{ name: "TiltShiftEffect", blendMode: { blendFunction: BlendFunction.NORMAL, opacity: { value: 1 } }, uniforms, defines: new Map() }]).parts;
    const p = pass.props as { uniforms: Record<string, unknown>; wgsl: string };
    expect(p.uniforms.start).toEqual([0.5, 0]);
    expect(p.wgsl).toContain("u_taper()");
  });

  test("every effect postprocessing exports is decided in kinds.ts", async () => {
    const pp = (await import("postprocessing")) as Record<string, unknown>;
    const effects = Object.keys(pp).filter((k) => /Effect$/.test(k) && typeof pp[k] === "function" && k !== "Effect");
    expect(effects.filter((k) => !EFFECTS[k])).toEqual([]);
  });
});

describe("<EffectComposer> on raea's stand-in renderer", () => {
  test("a depth of field renders frame after frame: its depth blit is a call that does nothing", () => {
    const gl = standInRenderer();
    const camera = new THREE.PerspectiveCamera();
    const composer = new EffectComposer(gl);
    composer.addPass(new RenderPass(new THREE.Scene(), camera));
    composer.addPass(new EffectPass(camera, new DepthOfFieldEffect(camera, { focusDistance: 0, focalLength: 0.02, bokehScale: 2 })));
    for (let i = 0; i < 3; i++) expect(() => composer.render(1 / 30)).not.toThrow();
    const ctx = gl.getContext() as unknown as Record<string, unknown>;
    expect(ctx.READ_FRAMEBUFFER).toBe(0);
    expect(ctx.then).toBeUndefined();
  });
});
