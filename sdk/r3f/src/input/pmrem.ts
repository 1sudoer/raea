// three's PMREMGenerator filters an environment on the GPU: it renders a scene (RoomEnvironment, three's own recipe for
// soft studio light) or a panorama into a cube map with the renderer, which in raea is a stand-in that never draws
// (session.ts). raea filters an environment itself (lights.ts skyOf, raea's <Environment>), so the generator hands
// back what raea can read: fromEquirectangular of a panorama raea loads is that panorama, lighting the scene as
// scene.environment; fromScene of three's RoomEnvironment is raea's <Environment room>, the same room made by raea's
// engine (#165); fromScene of any other scene and fromCubemap are left out, the scene drawn without them and a warning
// saying so (skyOf). fromScene threw inside the stand-in renderer and took the whole <R3F> scene with it (#131).
import * as THREE from "three";
import { srcOf } from "./texture";

/** what a stand-in environment was made by, when raea can't light by it: skyOf names it in its warning */
export const LEFT_OUT = "raeaLeftOut";
/** a stand-in for three's RoomEnvironment: its blur (fromScene's sigma), which skyOf makes <Environment room blur> */
export const ROOM = "raeaRoom";

function standIn(src: string | undefined, by: string): THREE.WebGLRenderTarget {
  const t = new THREE.WebGLRenderTarget(1, 1);
  t.texture.mapping = THREE.CubeUVReflectionMapping;
  if (src) t.texture.userData.src = src;
  else t.texture.userData[LEFT_OUT] = by;
  return t;
}

/** three's RoomEnvironment (three/examples/jsm/environments/RoomEnvironment.js), which names its scene so */
const isRoom = (scene: THREE.Object3D) => (scene as THREE.Scene).isScene === true && scene.name === "RoomEnvironment";

/** fromScene: the room as raea's own, anything else left out. A cube camera placed away from fromScene's default, the
 * origin (options.position), is drawn from the origin, and said so once. */
function fromScene(scene: THREE.Object3D, sigma = 0, _near?: number, _far?: number, options?: { position?: THREE.Vector3 }): THREE.WebGLRenderTarget {
  if (!isRoom(scene)) return standIn(undefined, "PMREMGenerator.fromScene");
  const t = new THREE.WebGLRenderTarget(1, 1);
  t.texture.mapping = THREE.CubeUVReflectionMapping;
  t.texture.userData[ROOM] = Number.isFinite(sigma) && sigma > 0 ? sigma : 0;
  const p = options?.position;
  if (p && p.lengthSq() > 0 && !warnedAt) {
    warnedAt = true;
    console.warn(`raea/r3f: PMREMGenerator.fromScene(RoomEnvironment) at position [${p.toArray().join(", ")}] is lit as from the origin, fromScene's default (raea makes the room from there)`);
  }
  return t;
}
let warnedAt = false;

let done = false;

/** PMREMGenerator's makers as stand-ins, once */
export function standInPMREM() {
  if (done) return;
  done = true;
  const pm = THREE.PMREMGenerator.prototype as unknown as Record<string, unknown>;
  pm.fromScene = fromScene;
  pm.fromSceneAsync = async (...a: Parameters<typeof fromScene>) => fromScene(...a);
  pm.fromEquirectangular = (texture: THREE.Texture) => standIn(srcOf(texture), "PMREMGenerator.fromEquirectangular");
  pm.fromEquirectangularAsync = async (texture: THREE.Texture) => standIn(srcOf(texture), "PMREMGenerator.fromEquirectangular");
  pm.fromCubemap = () => standIn(undefined, "PMREMGenerator.fromCubemap");
  pm.fromCubemapAsync = async () => standIn(undefined, "PMREMGenerator.fromCubemap");
  pm.compileCubemapShader = () => {};
  pm.compileEquirectangularShader = () => {};
}
