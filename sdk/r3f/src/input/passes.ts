// What an app draws into a render target itself, drawn on raea's GPU (docs/r3f.feature R3F23): a pass
// (gl.setRenderTarget(t); gl.render(scene, camera)) that targets.ts doesn't do on the CPU becomes its scene's parts as
// they were when it was drawn (its uniforms then, not at the end of the frame), on a render layer of its own, and a
// camera part drawing that layer into the target's image ("render:NAME:WxH", ":float" for a float target), ordered
// before the screen's camera in the order the app drew. A material whose texture is the target's reads that image
// (texture.ts srcOf), with v turned over as a flipY picture's is: three's target counts rows from the bottom.
import * as THREE from "three";
import type { Part } from "./material";
import { cameraOf } from "./lights";
import { sceneParts } from "./scene";
import { drawPassesWith, imageOf, type Pass } from "./targets";

/** the first render layer a pass takes: past a frame look's cells (layers 1 to 30) and their composite (31) */
const FIRST_LAYER = 64;
/** the order of the first pass's camera; the screen's is 0 */
const FIRST_ORDER = -10000;

/** each pass's place by its key (the target's texture, what was drawn into it and its place among those passes a
 *  frame), as it was first drawn: its layer and camera order stay the same frame to frame, so its parts and camera are
 *  the same rows */
const seen = new Map<string, number>();

function placeOf(key: string): number {
  let i = seen.get(key);
  if (i === undefined) {
    i = seen.size;
    seen.set(key, i);
  }
  return i;
}

/** a colour as the camera's clear: sRGB hex with alpha (three's clear colour is linear, as Bevy's decoded one is) */
function clearOf(c: THREE.Color, a: number): string {
  return `#${c.getHexString()}${Math.round(Math.min(1, Math.max(0, a)) * 255).toString(16).padStart(2, "0")}`;
}

/** `parts` on render layer `layer`: each part but a group's (a mesh, a light, text); a mesh's own children (its
 *  geometry and surface) go with it */
function onLayer(parts: Part[], layer: number): Part[] {
  return parts.map((p) => (p.type === "Group" ? { ...p, children: onLayer(p.children ?? [], layer) } : { ...p, props: { ...p.props, layers: [layer] }, children: p.children?.map(intoTarget) }));
}

/** a three shader drawn into a target writes its colour as it is: three tone-maps and encodes only what it draws to
 *  the screen (raea-sys-three-shader's output "target") */
function intoTarget(p: Part): Part {
  return p.type === "ThreeShaderMaterial" ? { ...p, props: { ...p.props, output: "target" } } : { ...p, children: p.children?.map(intoTarget) };
}

/** a camera for a pass three draws with a bare THREE.Camera (GPUComputationRenderer's, a full-screen quad's): its
 *  shader places the quad itself, so any camera that sees the quad at z 0 will do */
const QUAD: Part = { type: "OrthographicCamera", props: { position: [0, 0, 1], lookAt: [0, 0, 0], up: [0, 1, 0], height: 2, near: 0.01, far: 2 } };

/** a perspective pass camera's lens as three draws with it, from its projection matrix (drei's reflector copies the
 *  screen camera's projection into a camera of its own, whose fov and aspect stay as it was made): its fov, and its
 *  aspect where that isn't its target's shape (three draws the projection into any target; the engine keeps it) */
function lensOf(c: THREE.Camera, t: THREE.RenderTarget): Partial<Record<"fov" | "aspect", number>> {
  if (!(c as THREE.PerspectiveCamera).isPerspectiveCamera) return {};
  const e = c.projectionMatrix.elements;
  const fov = (2 * Math.atan(1 / e[5]) * 180) / Math.PI, aspect = e[5] / e[0];
  return Math.abs(aspect - t.width / t.height) > 1e-3 ? { fov, aspect } : { fov };
}

/** a pass as parts: its scene's on its layer and its camera into the target's image */
function passParts(p: Omit<Pass, "parts">, gl: THREE.WebGLRenderer): Part[] {
  const key = `${p.target.texture.uuid}#${p.what}#${p.k}`;
  const i = placeOf(key);
  const layer = FIRST_LAYER + i;
  const root = `pass/${key}`;
  const { world, drawn } = sceneParts(p.scene as THREE.Scene, p.camera, gl, p.target.height, root);
  const cam = cameraOf(p.camera) ?? QUAD;
  const bg = (p.scene as THREE.Scene).background;
  const clear = bg instanceof THREE.Color ? { bg: clearOf(bg, 1) } : p.autoClear ? { bg: clearOf(new THREE.Color(p.clear[0], p.clear[1], p.clear[2]), p.clear[3]) } : { clear: "none" };
  const camera: Part = { type: cam.type, props: { ...cam.props, ...lensOf(p.camera, p.target), name: root, target: imageOf(p.target), layers: [layer], order: FIRST_ORDER + i, ...clear } };
  return [camera, ...onLayer([...world, ...drawn], layer)];
}

drawPassesWith(passParts);
