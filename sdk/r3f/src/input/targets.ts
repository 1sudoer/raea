// What an app draws into a render target itself, done on the CPU where it's a pass raea knows, so the picture a
// material shows from that target is there in raea's frame (docs/r3f.feature R3F13). raea's engine draws the scene;
// three never does, so a target an app fills in useFrame (gl.setRenderTarget(t); gl.render(...)) would otherwise
// be empty. The passes done here are the ones drei's <ContactShadows> makes, which are the common ones:
//
//   a depth pass: the scene drawn with a MeshDepthMaterial as scene.overrideMaterial from an orthographic camera, as
//     three draws it (no depth test, no blending: the last triangle drawn wins, opaque objects front to back), its
//     colour plain depth or drei's tint (ucolor * z * 2, alpha 1 - z, read from the material's onBeforeCompile)
//   a blur pass: three's HorizontalBlurShader or VerticalBlurShader (9 taps, h or v apart in uv) over a target
//
// Anything else drawn into a target is drawn by raea's engine: recorded here as a pass (passes.ts makes its parts),
// a camera into the target's image (docs/r3f.feature R3F23).
import * as THREE from "three";
import type { Part } from "./material";

/** a target's pixels: RGBA floats, row 0 at v = 0 (as three's targets hold them) */
interface Pixels {
  w: number;
  h: number;
  data: Float32Array;
}

const drawn = new WeakMap<THREE.Texture, Pixels>();

/** raea's working size for a target (the passes are in uv, so a smaller one blurs the same) */
const MAX = 256;

function pixelsFor(rt: THREE.RenderTarget): Pixels {
  const w = Math.max(1, Math.min(MAX, rt.width)), h = Math.max(1, Math.min(MAX, rt.height));
  let p = drawn.get(rt.texture);
  if (!p || p.w !== w || p.h !== h) {
    p = { w, h, data: new Float32Array(w * h * 4) };
    drawn.set(rt.texture, p);
  }
  return p;
}

/** a target's picture as raea's frame shows it, or undefined when nothing raea knows was drawn into it */
export function targetPixels(t: THREE.Texture): Pixels | undefined {
  return drawn.get(t);
}

const BLUR = [0.051, 0.0918, 0.12245, 0.1531, 0.1633, 0.1531, 0.12245, 0.0918, 0.051];

/** `gl.render(scene, camera)` into `rt` (the stand-in renderer `gl`'s, renderer.ts): done here when it's a pass raea
 *  does on the CPU, otherwise drawn by raea's engine (recordPass) */
export function renderInto(rt: THREE.RenderTarget, scene: THREE.Object3D, camera: THREE.Camera, clear: [number, number, number, number], gl?: THREE.WebGLRenderer) {
  if (!onCpu(rt, scene, camera, clear) && gl) recordPass(rt, scene, camera, clear, gl);
}

/** `gl.render(scene, camera)` into `rt` done here, when it's a pass raea does on the CPU: whether it was */
function onCpu(rt: THREE.RenderTarget, scene: THREE.Object3D, camera: THREE.Camera, clear: [number, number, number, number]): boolean {
  const mat = (scene as THREE.Mesh).material as THREE.ShaderMaterial | undefined;
  const u = mat?.uniforms;
  if ((scene as THREE.Mesh).isMesh && u?.tDiffuse && ("h" in u || "v" in u)) {
    const src = u.tDiffuse.value as THREE.Texture | null;
    const from = src ? drawn.get(src) : undefined;
    // a blur of a picture drawn here is done here; one of a target raea's engine draws is drawn there too
    if (from || !(src && onGpu(src))) {
      if (from) blur(from, pixelsFor(rt), "h" in u ? Number(u.h.value) : 0, "v" in u ? Number(u.v.value) : 0);
      return true;
    }
  }
  const over = (scene as THREE.Scene).overrideMaterial;
  if ((scene as THREE.Scene).isScene && (over as THREE.MeshDepthMaterial | null)?.isMeshDepthMaterial && (camera as THREE.OrthographicCamera).isOrthographicCamera) {
    const out = pixelsFor(rt);
    for (let i = 0; i < out.data.length; i += 4) out.data.set(clear, i);
    depth(scene as THREE.Scene, camera as THREE.OrthographicCamera, over as THREE.MeshDepthMaterial, out);
    return true;
  }
  return false;
}

/** a render into a target raea's engine draws: what was drawn, as it was then (its parts, made by passes.ts as the
 *  render is asked for, since an app changes its uniforms between passes), into which target, the k-th into it this
 *  frame */
export interface Pass {
  target: THREE.RenderTarget;
  scene: THREE.Object3D;
  camera: THREE.Camera;
  /** the renderer's clear colour, linear rgba, when the render was asked for */
  clear: [number, number, number, number];
  autoClear: boolean;
  /** what was drawn (drawnOf), and its place among this frame's passes of the same into the same target */
  what: string;
  k: number;
  parts: Part[];
}

/** what a pass draws, by identity: its scene or object, the material a lone mesh is drawn with (GPUComputationRenderer
 *  swaps one quad's material between its pass-through and each variable's shader) and the scene's override. A pass's
 *  rows (its camera, its parts) are those of the same thing drawn into the same target, frame to frame */
function drawnOf(scene: THREE.Object3D): string {
  const mat = (scene as THREE.Mesh).isMesh ? (scene as THREE.Mesh).material : undefined;
  const mats = Array.isArray(mat) ? mat.map((m) => m.uuid).join(",") : mat?.uuid;
  const over = (scene as THREE.Scene).overrideMaterial?.uuid;
  return [scene.uuid, mats, over].filter(Boolean).join(":");
}

/** the targets a pass was drawn into on raea's GPU (recordPass), ever, by their texture */
const onGpuTargets = new WeakMap<THREE.Texture, THREE.RenderTarget>();

let partsOf: ((p: Omit<Pass, "parts">, gl: THREE.WebGLRenderer) => Part[]) | undefined;

/** how a pass becomes parts (passes.ts, set as it loads: it walks scenes as scene.ts does, which reads targets) */
export function drawPassesWith(fn: (p: Omit<Pass, "parts">, gl: THREE.WebGLRenderer) => Part[]) {
  partsOf = fn;
}

/** the passes the stand-in renderer `gl` drew on raea's GPU this frame, in draw order (its __raea.passes), with the
 *  step() that empties them as each frame steps (session.ts, program.ts) */
function passesFor(gl: THREE.WebGLRenderer): Pass[] {
  const own = gl as unknown as { __raea?: { passes?: Pass[]; step?: () => void } };
  const raea = (own.__raea ??= {});
  if (!raea.passes) {
    const passes: Pass[] = [];
    raea.passes = passes;
    raea.step = () => void (passes.length = 0);
  }
  return raea.passes;
}

/** `gl.render(scene, camera)` into `rt` that raea's engine draws, added to the frame's passes with its parts as they
 *  are now */
function recordPass(rt: THREE.RenderTarget, scene: THREE.Object3D, camera: THREE.Camera, clear: [number, number, number, number], gl: THREE.WebGLRenderer) {
  // a cube or 3D target (a CubeCamera's, drei's <Environment> with children) isn't a 2D image a camera draws: left out
  // as before, the scene drawn without it (lights.ts skyOf says so for an environment)
  if ((rt as { isWebGLCubeRenderTarget?: boolean }).isWebGLCubeRenderTarget || (rt.texture as { isCubeTexture?: boolean }).isCubeTexture || rt.depth > 1) return;
  const passes = passesFor(gl);
  const p = { target: rt, scene, camera, clear: [...clear] as Pass["clear"], autoClear: gl.autoClear !== false, what: drawnOf(scene), k: 0 };
  p.k = passes.filter((x) => x.target === rt && x.what === p.what).length;
  onGpuTargets.set(rt.texture, rt);
  passes.push({ ...p, parts: partsOf ? partsOf(p, gl) : [] });
  // a renderer whose frames nobody steps (step() not called) keeps its latest passes, not every one ever drawn
  if (passes.length > 1024) passes.splice(0, passes.length - 1024);
}

/** whether `t` is a render target's that a pass was drawn into on raea's GPU (not one drawn on the CPU here, nor a
 *  target nothing drew into: three's PMREMGenerator's stand-in, pmrem.ts): an image a camera draws into, which counts
 *  rows from the top where three's target counts them from the bottom */
export function onGpu(t: THREE.Texture): boolean {
  return onGpuTargets.has(t) && !drawn.has(t);
}

/** the target a pass drew `t` into on raea's GPU, if one did */
export function gpuTargetOf(t: THREE.Texture): THREE.RenderTarget | undefined {
  return drawn.has(t) ? undefined : onGpuTargets.get(t);
}

/** three's 9-tap blur, `h` and `v` apart in uv, sampled linearly with edges clamped */
function blur(from: Pixels, out: Pixels, h: number, v: number) {
  const src = from.data.slice();
  const { w, h: hh } = from;
  const res = new Float32Array(w * hh * 4);
  const dx = h * w, dy = v * hh;
  for (let y = 0; y < hh; y++) {
    for (let x = 0; x < w; x++) {
      const o = (y * w + x) * 4;
      for (let k = 0; k < 9; k++) {
        const wt = BLUR[k], s = k - 4;
        // the texel centre's sample position, in texels
        const fx = Math.min(w - 1, Math.max(0, x + s * dx)), fy = Math.min(hh - 1, Math.max(0, y + s * dy));
        const x0 = Math.floor(fx), y0 = Math.floor(fy), x1 = Math.min(w - 1, x0 + 1), y1 = Math.min(hh - 1, y0 + 1);
        const tx = fx - x0, ty = fy - y0;
        const a = (y0 * w + x0) * 4, b = (y0 * w + x1) * 4, c = (y1 * w + x0) * 4, d = (y1 * w + x1) * 4;
        for (let ch = 0; ch < 4; ch++) {
          const top = src[a + ch] * (1 - tx) + src[b + ch] * tx, bot = src[c + ch] * (1 - tx) + src[d + ch] * tx;
          res[o + ch] += wt * (top * (1 - ty) + bot * ty);
        }
      }
    }
  }
  if (out.w === w && out.h === hh) out.data.set(res);
}

/** the depth material's colour for depth z (0 near, 1 far): drei's tint when its onBeforeCompile puts one in */
function depthColour(m: THREE.MeshDepthMaterial): (z: number, px: Float32Array, o: number) => void {
  const shader = { uniforms: {} as Record<string, { value: unknown }>, vertexShader: "", fragmentShader: "void main() {\n\tgl_FragColor = vec4( vec3( 1.0 - fragCoordZ ), opacity );\n}" };
  try {
    m.onBeforeCompile?.(shader as unknown as THREE.WebGLProgramParametersWithUniforms, undefined as unknown as THREE.WebGLRenderer);
  } catch {
    // its patch asked for what the stand-in shader hasn't: plain depth
  }
  const tint = shader.uniforms.ucolor?.value as THREE.Color | undefined;
  if (tint && /ucolor \* fragCoordZ \* 2\.0, \( 1\.0 - fragCoordZ \)/.test(shader.fragmentShader)) {
    const { r, g, b } = tint;
    return (z, px, o) => {
      px[o] = r * z * 2;
      px[o + 1] = g * z * 2;
      px[o + 2] = b * z * 2;
      px[o + 3] = 1 - z;
    };
  }
  const a = m.opacity;
  return (z, px, o) => {
    px[o] = px[o + 1] = px[o + 2] = 1 - z;
    px[o + 3] = a;
  };
}

const v = new THREE.Vector3();
const mvp = new THREE.Matrix4();

/** the scene's meshes drawn as `m` from `cam` into `out`, as three would: front faces only, no depth test */
function depth(scene: THREE.Scene, cam: THREE.OrthographicCamera, m: THREE.MeshDepthMaterial, out: Pixels) {
  scene.updateMatrixWorld();
  cam.updateMatrixWorld();
  cam.updateProjectionMatrix();
  const colour = depthColour(m);
  const meshes: { mesh: THREE.Mesh; z: number }[] = [];
  const visit = (o: THREE.Object3D) => {
    if (!o.visible) return;
    const mesh = o as THREE.Mesh;
    if (mesh.isMesh && !(mesh as unknown as THREE.Points).isPoints && mesh.geometry?.attributes?.position) {
      v.setFromMatrixPosition(mesh.matrixWorld).applyMatrix4(cam.matrixWorldInverse);
      meshes.push({ mesh, z: -v.z });
    }
    for (const c of o.children) visit(c);
  };
  visit(scene);
  // three draws opaque objects front to back (its painter sort, by renderOrder, then distance)
  meshes.sort((a, b) => a.mesh.renderOrder - b.mesh.renderOrder || a.z - b.z);
  const { w, h, data } = out;
  const near = cam.near, far = cam.far;
  const sx = new Float32Array(3), sy = new Float32Array(3), sz = new Float32Array(3);
  const inst = new THREE.Matrix4();
  for (const { mesh } of meshes) {
    const pos = mesh.geometry.attributes.position as THREE.BufferAttribute;
    const index = mesh.geometry.index;
    const n = index ? index.count : pos.count;
    const im = mesh as THREE.InstancedMesh;
    const count = im.isInstancedMesh ? im.count : 1;
    for (let k = 0; k < count; k++) {
      mvp.multiplyMatrices(cam.matrixWorldInverse, mesh.matrixWorld);
      if (im.isInstancedMesh) {
        im.getMatrixAt(k, inst);
        mvp.multiply(inst);
      }
      const flip = mvp.determinant() < 0;
      for (let t = 0; t + 2 < n; t += 3) {
        for (let j = 0; j < 3; j++) {
          const i = index ? index.getX(t + j) : t + j;
          v.fromBufferAttribute(pos, i).applyMatrix4(mvp);
          // view space to the target's pixels (x right, y up from row 0) and depth 0..1 from near to far
          sx[j] = ((v.x - cam.left) / (cam.right - cam.left)) * w;
          sy[j] = ((v.y - cam.bottom) / (cam.top - cam.bottom)) * h;
          sz[j] = (-v.z - near) / (far - near);
        }
        const area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sx[2] - sx[0]) * (sy[1] - sy[0]);
        // a front face winds counter-clockwise on screen (and the other way under a mirroring matrix)
        if (area === 0 || (area > 0) === flip) continue;
        const x0 = Math.max(0, Math.floor(Math.min(sx[0], sx[1], sx[2]))), x1 = Math.min(w - 1, Math.ceil(Math.max(sx[0], sx[1], sx[2])));
        const y0 = Math.max(0, Math.floor(Math.min(sy[0], sy[1], sy[2]))), y1 = Math.min(h - 1, Math.ceil(Math.max(sy[0], sy[1], sy[2])));
        for (let y = y0; y <= y1; y++) {
          const py = y + 0.5;
          for (let x = x0; x <= x1; x++) {
            const px = x + 0.5;
            const w0 = ((sx[1] - px) * (sy[2] - py) - (sx[2] - px) * (sy[1] - py)) / area;
            const w1 = ((sx[2] - px) * (sy[0] - py) - (sx[0] - px) * (sy[2] - py)) / area;
            const w2 = 1 - w0 - w1;
            if (w0 < 0 || w1 < 0 || w2 < 0) continue;
            const z = w0 * sz[0] + w1 * sz[1] + w2 * sz[2];
            if (z < 0 || z > 1) continue;
            colour(z, data, (y * w + x) * 4);
          }
        }
      }
    }
  }
}

/** a render target's image as raea names it ("render:NAME:WxH", ":float" for a float target): by its texture and
 *  size, so a resized target is a new image */
export function imageOf(rt: THREE.RenderTarget): string {
  const w = Math.max(1, Math.round(rt.width)), h = Math.max(1, Math.round(rt.height));
  const t = rt.texture;
  const float = t.type === THREE.FloatType || t.type === THREE.HalfFloatType;
  return `render:${t.uuid}@${w}x${h}:${w}x${h}${float ? ":float" : ""}`;
}

/** the parts of what the app drew into targets on raea's GPU this frame (the stand-in renderer's passes), in order.
 *  With `seen` (the frame's parts), only the passes whose pictures it reads, or a pass it keeps reads */
export function passesOf(gl: THREE.WebGLRenderer, seen?: Part[]): Part[] {
  const passes = (gl as unknown as { __raea?: { passes?: Pass[] } }).__raea?.passes;
  if (!passes?.length) return [];
  if (!seen) return passes.flatMap((p) => p.parts);
  const want = readsOf(seen);
  const keep = new Set<Pass>();
  for (let grew = true; grew; ) {
    grew = false;
    for (const p of passes) {
      if (keep.has(p) || !want.has(p.target.texture.uuid)) continue;
      keep.add(p);
      readsOf(p.parts, want);
      grew = true;
    }
  }
  return passes.filter((p) => keep.has(p)).flatMap((p) => p.parts);
}

/** the render targets `parts` read, by texture uuid: every image name ("render:UUID@...", imageOf) in their props */
function readsOf(parts: Part[], out = new Set<string>()): Set<string> {
  const scan = (v: unknown): void => {
    if (typeof v === "string") {
      if (v.includes("render:")) for (const m of v.matchAll(/render:([^@:\s"]+)@/g)) out.add(m[1]);
    } else if (Array.isArray(v)) {
      // a geometry's numbers hold no names
      if (typeof v[0] !== "number") v.forEach(scan);
    } else if (v && typeof v === "object" && !ArrayBuffer.isView(v)) for (const x of Object.values(v)) scan(x);
  };
  for (const p of parts) {
    scan(p.props);
    if (p.children) readsOf(p.children, out);
  }
  return out;
}
