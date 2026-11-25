// A scene drawn through a composer (post.ts composerEffects): three draws it into the composer's float buffer, so a
// ShaderMaterial's colour is scene light there (shader.ts composing), and the composer's own passes into its targets
// (the scene into its buffer, a bloom's mip chain) aren't drawn by raea, whose effects stand for them (targets.ts
// passesOf). A target the frame shows is still drawn.
import { expect, test } from "bun:test";
import * as PP from "@react-three/postprocessing";
import { createPortal, useFrame } from "@react-three/fiber";
import { useMemo, type ReactNode } from "react";
import * as THREE from "three";
import { convert } from "../src/input/scene";
import { end, stepTo } from "../src/input/session";
import type { Part } from "../src/parts";

const flat = (parts: Part[]): Part[] => parts.flatMap((p) => [p, ...flat(p.children ?? [])]);
let n = 0;
function partsOf(scene: () => ReactNode): Part[] {
  const key = `composer${n++}`;
  const s = stepTo(key, 2, 30, 640, 360, {}, scene);
  const st = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
  const out = flat(convert(st.scene, st.camera, st.gl, 360).parts);
  end(key);
  return out;
}

function Ball() {
  const m = useMemo(() => new THREE.ShaderMaterial({
    vertexShader: "void main() { gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }",
    fragmentShader: "void main() { gl_FragColor = linearToOutputTexel(vec4(0.8, 0.4, 0.2, 1.0)); }",
  }), []);
  return <mesh material={m}><sphereGeometry /></mesh>;
}

/** a knot drawn into a target every frame, shown on a plane (or the plane plain) */
function Fbo({ show = true }: { show?: boolean }) {
  const target = useMemo(() => new THREE.WebGLRenderTarget(128, 64), []);
  const scene = useMemo(() => new THREE.Scene(), []);
  const cam = useMemo(() => {
    const c = new THREE.PerspectiveCamera(50, 2, 0.1, 100);
    c.position.set(0, 0, 4);
    return c;
  }, []);
  useFrame(({ gl }) => {
    gl.setRenderTarget(target);
    gl.render(scene, cam);
    gl.setRenderTarget(null);
  });
  return (
    <>
      {createPortal(<mesh><torusKnotGeometry args={[0.6, 0.2, 32, 8]} /><meshBasicMaterial color="#2266ff" /></mesh>, scene)}
      <mesh><planeGeometry args={[2, 1]} /><meshBasicMaterial map={show ? target.texture : null} /></mesh>
    </>
  );
}

const bloom = <PP.EffectComposer><PP.Bloom intensity={0.6} luminanceThreshold={0.9} mipmapBlur /></PP.EffectComposer>;
const shader = (out: Part[]) => out.find((p) => p.type === "ThreeShaderMaterial")!;
const intoTargets = (out: Part[]) => out.filter((p) => typeof p.props.target === "string");

test("a ShaderMaterial is scene light under a composer, its output colour space the buffer's", () => {
  const plain = shader(partsOf(() => <Ball />));
  expect(plain.props.output).toBe("display");
  expect((plain.props.glsl as { fragment: string }).fragment).toContain("0.41666");
  const composed = shader(partsOf(() => <><Ball />{bloom}</>));
  expect(composed.props.output).toBe("linear");
  expect((composed.props.glsl as { fragment: string }).fragment).toContain("vec4 linearToOutputTexel(vec4 v) { return v; }");
});

test("a composer's own passes aren't drawn: one camera, the screen's, for the scene and a bloom", () => {
  const out = partsOf(() => <><Ball />{bloom}</>);
  expect(intoTargets(out)).toEqual([]);
  expect(out.filter((p) => p.type === "ThreeShaderMaterial").length).toBe(1);
  expect(out.some((p) => p.type === "Bloom")).toBe(true);
});

test("under a composer, a target the frame shows is drawn and one it doesn't isn't; without one, both are", () => {
  const shown = intoTargets(partsOf(() => <><Fbo />{bloom}</>));
  expect(shown.length).toBe(1);
  expect(shown[0].props.target).toMatch(/:128x64$/);
  expect(intoTargets(partsOf(() => <><Fbo show={false} />{bloom}</>))).toEqual([]);
  expect(intoTargets(partsOf(() => <Fbo show={false} />)).length).toBe(1);
});
