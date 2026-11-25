// Plain three.js programs (docs/r3f.feature R3F19): a single-file demo's script as a function, its renderer its own.
// In raea, <Three program> runs it with raea's stand-in renderer (the bundler resolves this file's "three" to
// src/input/vanilla.ts); in Chrome, the harness page runs it on a real canvas with three's WebGLRenderer, its
// requestAnimationFrame stepped 1/fps a frame. Both draw frame `frame`, compared by SSIM (tools/parity.sh).
import * as THREE from "three";

export interface ProgramCase {
  program: (canvas: HTMLCanvasElement) => void;
  frame: number;
}

/** three's "getting started" grown a little: a lit, shadowed knot turning on a floor, a ring of boxes around it */
function vanilla(canvas: HTMLCanvasElement) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(1);
  renderer.setSize(640, 360, false);
  renderer.shadowMap.enabled = true;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#20242e");
  const camera = new THREE.PerspectiveCamera(45, 640 / 360, 0.1, 100);
  camera.position.set(0, 2.4, 6.5);
  camera.lookAt(0, 0.8, 0);

  scene.add(new THREE.HemisphereLight("#cfe0ff", "#2a2420", 0.8));
  const sun = new THREE.DirectionalLight("#ffffff", 2.4);
  sun.position.set(3, 6, 2);
  sun.castShadow = true;
  scene.add(sun);

  const floor = new THREE.Mesh(new THREE.PlaneGeometry(14, 14), new THREE.MeshStandardMaterial({ color: "#cfd6e0", roughness: 0.9 }));
  floor.rotation.x = -Math.PI / 2;
  floor.receiveShadow = true;
  scene.add(floor);

  const knot = new THREE.Mesh(new THREE.TorusKnotGeometry(0.6, 0.22, 160, 24), new THREE.MeshStandardMaterial({ color: "#e05a2b", roughness: 0.35, metalness: 0.1 }));
  knot.position.y = 1.1;
  knot.castShadow = true;
  scene.add(knot);

  const ring = new THREE.Group();
  for (let i = 0; i < 10; i++) {
    const a = (i / 10) * Math.PI * 2;
    const box = new THREE.Mesh(new THREE.BoxGeometry(0.35, 0.35, 0.35), new THREE.MeshStandardMaterial({ color: new THREE.Color().setHSL(i / 10, 0.6, 0.55) }));
    box.position.set(Math.cos(a) * 2.2, 0.18, Math.sin(a) * 2.2);
    box.castShadow = true;
    ring.add(box);
  }
  scene.add(ring);

  renderer.setAnimationLoop((time) => {
    // by the loop's time, and by a step a call: both come out the same when each frame is one call
    knot.rotation.y = time / 1000;
    knot.rotation.x += 0.03;
    ring.rotation.y = -time / 2000;
    renderer.render(scene, camera);
  });
}

export const PROGRAMS: Record<string, ProgramCase> = {
  vanilla: { program: vanilla, frame: 24 },
};
