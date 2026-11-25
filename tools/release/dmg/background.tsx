// raea's installer window (tools/release/dmg/dmg.sh): the picture behind raea.app and the Applications folder in the
// disk image's Finder window, drawn by raea itself. One WGSL shader over the whole window: paper, a pixel falloff
// dithered into the corners, raea's cat leaping from the app toward Applications as onion-skinned frames (the pixels
// thin out behind it, Bayer-dithered), the rest of its arc as dots with a pixel arrowhead, and a second cat asleep in
// the corner. The line under it is the page's text. Finder draws the icons' names in black over any picture (macOS 27,
// light or dark appearance), so the window stays paper-light.
//
// Finder draws the window at 1x on a plain display and 2x on Retina, from one multi-resolution TIFF, so it renders
// twice: "dmg-1x" (680x440) and "dmg-2x" (1360x880); dmg.sh joins them with tiffutil. "dmg-loop" is the same scene
// moving (the cat makes its leap, the other one breathes), for what can move: Finder can't animate a window's picture.
//
//   raea render background.tsx --composition dmg-2x --frame 0 -o background@2x.png
//
// Geometry is in the window's points (1x pixels). Finder centres each icon on the point dmg.sh gives it, so APP and
// APPS here must match dmg.sh's --app-at and --apps-at.
import { AbsoluteFill, Composition, registerRoot, useCurrentFrame } from "raea";
import { Canvas3D, ShaderMesh, ToneMapping } from "raea/3d";
import { Camera3d } from "raea/camera";
import { loadFont } from "raea/text";
import { Transform } from "raea/transform";

export const W = 680;
export const H = 440;
export const APP = [170, 232] as const;
export const APPS = [510, 232] as const;
const LINE_Y = 384;
const LOOP_FPS = 60;
const LOOP_FRAMES = 180; // 3 s: the leap takes the first 1.1 s, then the cat is gone into Applications and the arc rests

// raea's cat (docs/logo/cat-black.svg on its 36-unit grid, cropped): '#' is ink, its eyes are holes
const CAT = [
  "......###.....................", ".....####.....................", "....####......................",
  "....####......###.............", "....####.....#####.....###....", "....#####....#####....####....",
  ".....#####..##############....", ".....#####..##############....", "......#####.##############....",
  "......#####.##############....", "......####################....", ".....#############.####.###...",
  "....##############.####.###...", "....##############.####.###...", "...########################...",
  "...########################...", "..#########################...", "..#########################...",
  "..########################....", "..#######################.....", ".#########################....",
  ".#########################....", ".##########################...", "######.####################...",
  "#####....###################..", "#####.....#########...######..", "####......######.......######.",
  "####......######........#####.", ".##.......#####...........##..", "..........#####...............",
  "..........#####...............", "...........###................",
];
// the other cat, asleep
const NAP = [
  ".......................##..", "...#....#...............#..", "...##..##...............#..",
  "...######.....#######...#..", "..########..#############..", "..#..##..###############...",
  "..######################...", "...#####################...", "....###################....",
];
const ZED = ["#####", "...#.", "..#..", ".#...", "#####"];

/** A '#'/'.' grid as a texture: white where '#', nearest-sampled. */
function grid(rows: string[]) {
  const width = rows[0].length;
  return { width, height: rows.length, pixels: rows.flatMap((r) => [...r].map((c) => (c === "#" ? "#ffffff" : "#000000"))), nearest: true };
}

const WGSL = /* wgsl */ `
const PAPER = vec3<f32>(0.980, 0.980, 0.976);
const INK = vec3<f32>(0.067, 0.067, 0.071);
const FALL = vec3<f32>(0.918, 0.918, 0.910);
const TRAIL = vec3<f32>(0.600, 0.600, 0.590);
const NAPC = vec3<f32>(0.680, 0.680, 0.670);

fn lin(c: vec3<f32>) -> vec3<f32> {
  return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// 8x8 ordered dither, 0..1: the bit-reversed interleave of x^y and y
fn bayer(c: vec2<i32>) -> f32 {
  let x = u32(c.x) & 7u;
  let y = u32(c.y) & 7u;
  let v = x ^ y;
  let b = ((v & 1u) << 5u) | ((y & 1u) << 4u) | ((v & 2u) << 2u) | ((y & 2u) << 1u) | ((v & 4u) >> 1u) | ((y & 4u) >> 2u);
  return (f32(b) + 0.5) / 64.0;
}

// the leap: a quadratic from the app's right shoulder, over the gap, down to Applications' left
fn arc(t: f32) -> vec2<f32> {
  let a = vec2<f32>(${APP[0] + 70}.0, 196.0);
  let c = vec2<f32>(${(APP[0] + APPS[0]) / 2}.0, -4.0);
  let b = vec2<f32>(${APPS[0] - 70}.0, 196.0);
  let u = 1.0 - t;
  return u * u * a + 2.0 * u * t * c + t * t * b;
}

// is the cell of a grid sprite under q set? sprite's top-left at o, cell size k points
fn cat_at(q: vec2<f32>, o: vec2<f32>, k: f32) -> bool {
  let g = floor((q - o) / k);
  if (g.x < 0.0 || g.y < 0.0 || g.x >= 30.0 || g.y >= 32.0) { return false; }
  return textureLoad(tex_cat, vec2<i32>(g), 0).r > 0.5;
}
fn nap_at(q: vec2<f32>, o: vec2<f32>, k: f32) -> bool {
  let g = floor((q - o) / k);
  if (g.x < 0.0 || g.y < 0.0 || g.x >= 27.0 || g.y >= 9.0) { return false; }
  return textureLoad(tex_nap, vec2<i32>(g), 0).r > 0.5;
}
fn zed_at(q: vec2<f32>, o: vec2<f32>, k: f32) -> bool {
  let g = floor((q - o) / k);
  if (g.x < 0.0 || g.y < 0.0 || g.x >= 5.0 || g.y >= 5.0) { return false; }
  return textureLoad(tex_zed, vec2<i32>(g), 0).r > 0.5;
}

fn shade(in: VertexOutput) -> vec4<f32> {
  let s = u_s();
  let size = vec2<f32>(${W}.0, ${H}.0) * s;
  let px = floor(in.uv * size);          // the device pixel
  let q = (px + 0.5) / s;                // its centre in points
  let unit = vec2<i32>(floor(q));        // the 1-point cell the dither reads
  let phase = u_phase();                 // where the hero is on its arc, 0..1
  var col = PAPER;

  // the falloff: 2-point pixels dithered into the corners, none in the middle
  let cell = floor(q / 2.0);
  let e = (cell * 2.0 + 1.0 - vec2<f32>(${W / 2}.0, ${H / 2}.0)) / vec2<f32>(${W / 2}.0, ${H / 2}.0);
  let r = length(e * vec2<f32>(1.0, 0.92));
  if (bayer(vec2<i32>(cell)) < smoothstep(0.86, 1.5, r) * 0.5) { col = FALL; }

  let k = 2.0;                            // the cat's cell, in points
  let hs = vec2<f32>(15.0, 16.0) * k;
  let hero = arc(phase);

  // the rest of the arc, ahead of it: 2-point dots and a pixel arrowhead into Applications
  let ahead = phase + 0.12;
  for (var i = 0; i < 10; i = i + 1) {
    let t = ahead + f32(i) * 0.038;
    if (t > 0.94) { break; }
    let d = floor(arc(t) / 2.0) * 2.0;
    if (all(q >= d) && all(q < d + 2.0)) { col = TRAIL; }
  }
  let tip = floor(arc(0.975) / 2.0) * 2.0;
  for (var j = 0; j < 5; j = j + 1) {
    let a = tip - vec2<f32>(f32(j) * 2.0, 0.0);
    let b = tip - vec2<f32>(0.0, f32(j) * 2.0);
    if ((all(q >= a) && all(q < a + 2.0)) || (all(q >= b) && all(q < b + 2.0))) { col = TRAIL; }
  }

  // onion skins: the frames behind it, thinning out (dithered at 1 point) the further back they are
  for (var g = 3; g >= 1; g = g - 1) {
    let t = phase - f32(g) * 0.155;
    if (t < 0.02 || u_on() < 0.5) { continue; }
    let o = floor((arc(t) - hs) / k) * k;
    if (cat_at(q, o, k) && bayer(unit) < 0.62 - f32(g) * 0.16) { col = INK; }
  }
  // and the cat itself
  if (u_on() > 0.5 && cat_at(q, floor((hero - hs) / k) * k, k)) { col = INK; }

  // the one asleep in the corner, breathing, with its z's
  let breath = u_breath();
  let no = vec2<f32>(${W - 108}.0, ${H - 52}.0 - floor(breath * 2.0 + 0.5));
  if (nap_at(q, no, 3.0)) { col = NAPC; }
  // each z rises from the head and thins out (dithered) as it goes
  let z1 = fract(u_zed());
  let z2 = fract(u_zed() + 0.5);
  if (zed_at(q, no + vec2<f32>(-2.0 - floor(z1 * 10.0), -12.0 - floor(z1 * 18.0)), 2.0) && bayer(unit) < 1.25 - z1) { col = NAPC; }
  if (zed_at(q, no + vec2<f32>(-14.0 - floor(z2 * 10.0), -26.0 - floor(z2 * 18.0)), 3.0) && bayer(unit) < 1.25 - z2) { col = NAPC; }

  return vec4<f32>(lin(col), 1.0);
}`;

const TEX = { cat: grid(CAT), nap: grid(NAP), zed: grid(ZED) };

/** Where the cat is on frame f of the loop: a leap with an eased take-off and landing, then gone (in Applications). */
function leapAt(f: number) {
  const t = f / (LOOP_FPS * 1.1);
  if (t >= 1) return { phase: 1, on: 0 };
  const e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
  return { phase: 0.02 + 0.96 * e, on: 1 };
}

function Window({ s, moving }: { s: number; moving?: boolean }) {
  const f = useCurrentFrame();
  const { fontFamily } = loadFont("Geist");
  // the still: the cat at the top of its leap, the z's mid-rise
  const pose = moving ? leapAt(f) : { phase: 0.52, on: 1 };
  const breath = moving ? (1 - Math.cos((f / LOOP_FPS) * Math.PI * 2 * 0.5)) / 2 : 0;
  const zed = moving ? (f / LOOP_FRAMES) * 2 : 0.15;
  return (
    <AbsoluteFill style={{ background: "#fafaf9" }}>
      <Canvas3D layered>
        <Camera3d transform={Transform.fromXyz(0, 0, 5).lookingAt([0, 0, 0])} projection={{ Orthographic: { scaling_mode: { FixedVertical: { viewport_height: 1 } } } }} />
        <ToneMapping mode="none" />
        <ShaderMesh
          shape={{ primitive: "Rectangle", half_size: [W / H / 2, 0.5] }}
          wgsl={WGSL}
          uniforms={{ s, phase: pose.phase, on: pose.on, breath, zed }}
          textures={TEX}
        />
      </Canvas3D>
      <div
        style={{
          position: "absolute", left: 0, right: 0, top: (LINE_Y - 9) * s, textAlign: "center", fontFamily,
          fontSize: 13 * s, lineHeight: `${18 * s}px`, fontWeight: 500, letterSpacing: "-0.01em", color: "#8a8a87",
          opacity: 1, transform: "scale(1)",
        }}
      >
        Drag raea into Applications
      </div>
    </AbsoluteFill>
  );
}

registerRoot(() => (
  <>
    <Composition id="dmg-1x" component={() => <Window s={1} />} durationInFrames={1} fps={30} width={W} height={H} />
    <Composition id="dmg-2x" component={() => <Window s={2} />} durationInFrames={1} fps={30} width={W * 2} height={H * 2} />
    <Composition id="dmg-loop" component={() => <Window s={2} moving />} durationInFrames={LOOP_FRAMES} fps={LOOP_FPS} width={W * 2} height={H * 2} />
  </>
));
