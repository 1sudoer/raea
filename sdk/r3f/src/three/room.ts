// <Environment room> on the three.js side (#165): three's RoomEnvironment through a renderer's PMREMGenerator, the way
// a three.js scene gets soft studio light with no file. scene.ts calls room() for the part and again from apply()
// once the page has its renderer, which the PMREMGenerator needs.
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { EXPOSURE } from "../units";

type Props = Record<string, unknown>;

/** three's RoomEnvironment filtered by a renderer's PMREMGenerator, made once a renderer and blur */
const rooms = new WeakMap<THREE.WebGLRenderer, Map<number, THREE.Texture>>();
function roomOf(renderer: THREE.WebGLRenderer, blur: number): THREE.Texture {
  let made = rooms.get(renderer);
  if (!made) rooms.set(renderer, (made = new Map()));
  let t = made.get(blur);
  if (!t) {
    const pm = new THREE.PMREMGenerator(renderer);
    const room = new RoomEnvironment();
    t = pm.fromScene(room, blur).texture;
    room.dispose();
    pm.dispose();
    made.set(blur, t);
  }
  return t;
}

/** the scene lit by the room (and with `background` showing it), its keys kept in userData.raeaEnvironment for
 *  apply() and the source export; with no renderer yet the scene has no environment until one comes */
export function room(scene: THREE.Scene, p: Props, renderer?: THREE.WebGLRenderer) {
  const blur = (p.blur as number) ?? 0, intensity = (p.intensity as number) ?? 1, rotation = (p.rotation as number) ?? 0;
  const sky = (p.backgroundIntensity as number) ?? intensity;
  scene.userData.raeaEnvironment = { room: true, blur, intensity, rotation, background: !!p.background, backgroundIntensity: sky };
  const t = renderer ? roomOf(renderer, blur) : null;
  scene.environment = intensity > 0 ? t : null;
  scene.environmentIntensity = intensity * 1000 * EXPOSURE;
  scene.environmentRotation.set(0, rotation, 0);
  if (p.background) {
    scene.background = t;
    scene.backgroundIntensity = sky * 1000 * EXPOSURE;
    scene.backgroundRotation.set(0, rotation, 0);
  }
}
