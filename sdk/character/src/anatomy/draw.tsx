// Drawing a body's anatomy where its pose puts it, and the X-ray that shows it: the skin and what the body wears made
// translucent over bright bone and tinted organs, the path of a shot through them, and the wounds where it went in
// and out.
//
//   const A = anatomyOf(sentrySkel), shot = hits(sentrySkel, pose, A, muzzle, dir);
//   <Character body=... pose={pose} ghost={xrayGhost(t)} parts={xrayParts(parts, t)} />
//   <Anatomy skel={sentrySkel} pose={pose} anatomy={A} name="sentry" xray={t} />
//   <ShotPath from={muzzle} to={shot.at(-1)!.exit} name="sentry" />  <Wounds hits={shot} name="sentry" />
import { Fragment } from "react";
import { pose as posed, qmul, qyaw, skinJoints, toScene, type CharacterGhost, type CharacterPart, type Placement, type Pose, type Skeleton } from "../character/all";
import { BufferMesh } from "@raea/render/3d";
import { LinearRgba, Mesh3d, Srgba, StandardMaterial, Transform } from "@raea/render/bevy3d";
import type { Hit } from "./hits";
import { blob, tube, type V3 } from "./mesh";
import type { Piece } from "./piece";

const mix = (a: number, b: number, t: number) => a + (b - a) * t;
const glow = (hex: string, s: number) => LinearRgba.from(Srgba.hex(hex)).mul(s);

/** what an X-ray shows of each kind: bone near white and bright, organs in their colour, dimmer */
const XRAY: Record<string, { color: string; glow: number }> = {
  bone: { color: "#f2f6ff", glow: 2.2 }, cartilage: { color: "#b9d4e6", glow: 1.0 }, organ: { color: "", glow: 0.9 }, vessel: { color: "", glow: 1.1 }, airway: { color: "#cfe3f2", glow: 0.8 },
};

export interface AnatomyProps {
  skel: Skeleton;
  pose: Pose;
  anatomy: { pieces: Piece[] };
  /** where the body stands (the same as its <Character>'s) */
  at?: Placement;
  /** names its meshes: `${name}-anatomy-${piece}` */
  name: string;
  /** which pieces to draw (all by default) */
  show?: (p: Piece) => boolean;
  /** 0 as the pieces are, 1 as an X-ray shows them (lit from within); between, a fade */
  xray?: number;
  /** the pieces a shot has gone through so far (their names, as hits() gives them), drawn flushed red and glowing */
  struck?: string[];
}

const STRUCK = "#ff3b2f";

export function Anatomy({ skel, pose, anatomy, at = {}, name, show, xray = 0, struck = [] }: AnatomyProps) {
  const hit = new Set(struck);
  const J = new Map(skinJoints(skel, posed(skel, pose)).map((j) => [j.name, j]));
  const turn = qyaw(at.yaw ?? 0), k = at.scale ?? 1;
  return (
    <>
      {anatomy.pieces.filter((p) => (show ? show(p) : true)).map((p) => {
        const j = J.get(p.joint);
        if (!j) return null;
        const x = XRAY[p.kind], plain = xray > 0 && x.color ? x.color : p.color, struckNow = hit.has(p.name);
        const color = struckNow ? STRUCK : plain;
        return (
          // (organs press on each other and the aorta leaves the heart: the pieces of a body touch by nature)
          <Mesh3d key={p.name} name={`${name}-anatomy-${p.name}`} allowOverlap transform={Transform.fromTranslation(toScene(at, j.pos) as V3).withRotation(qmul(turn, j.rot)).withScale(k)}>
            <BufferMesh positions={p.geo.positions} normals={p.geo.normals} indices={p.geo.indices} />
            <StandardMaterial base_color={color} perceptual_roughness={mix(p.roughness, 0.9, xray)} {...(struckNow ? { emissive: glow(STRUCK, 1.5 + 2.5 * xray) } : xray > 0 ? { emissive: glow(color, x.glow * xray) } : {})} />
          </Mesh3d>
        );
      })}
    </>
  );
}

/** the skin as an X-ray shows it: a pale translucent film over what is inside, more see-through as `t` goes to 1
 *  (undefined at 0: the skin as it is) */
export function xrayGhost(t: number, color = "#9cc6e4"): CharacterGhost | undefined {
  if (t <= 0) return undefined;
  return { color, opacity: mix(0.55, 0.1, t), emissive: "#1d3a52", emissiveIntensity: t, unlit: true };
}

/** what the body wears (and its head and hair) made translucent the same way */
export function xrayParts(parts: CharacterPart[], t: number, color = "#a9cde6"): CharacterPart[] {
  if (t <= 0) return parts;
  // (lit, not unlit: a <Model> takes an unlit material only drawn flat, as the ghost's body is, and a part is not)
  return parts.map((p) => ({ ...p, children: <StandardMaterial base_color_texture={null} base_color={Srgba.hex(color).with_alpha(mix(0.45, 0.07, t))} alpha_mode="Blend" perceptual_roughness={0.9} emissive={glow("#1d3a52", 1.5 * t)} /> }));
}

/** the path of a shot as a thin glowing line, from where it was fired to where it stops */
export function ShotPath({ from, to, name, color = "#ffb347", radius = 0.0025 }: { from: V3; to: V3; name: string; color?: string; radius?: number }) {
  const g = tube([from, to], () => [radius, radius], 8);
  return (
    <Mesh3d name={`${name}-shot-path`}>
      <BufferMesh positions={g.positions} normals={g.normals} indices={g.indices} />
      <StandardMaterial base_color={color} emissive={glow(color, 6)} unlit />
    </Mesh3d>
  );
}

/** the wounds a shot leaves: a small dark hole where it went into the skin and a larger torn one where it came out,
 *  each laid on the skin square to the path (a garment over the skin is marked `cloth` metres further out). With
 *  `travelled` (metres the bullet has gone from the shot's origin, as hits()'s t0 and t1 count), each appears only once
 *  the bullet is past it. */
export function Wounds({ hits, name, cloth = 0, travelled = Infinity }: { hits: Hit[]; name: string; cloth?: number; travelled?: number }) {
  const skin = hits.find((h) => h.part === "skin");
  if (!skin || travelled < skin.t0 - cloth) return null;
  const d = ((a: V3, b: V3) => { const v: V3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], l = Math.hypot(...v) || 1; return [v[0] / l, v[1] / l, v[2] / l] as V3; })(skin.entry, skin.exit);
  const mark = (at: V3, r: number, out: number) => {
    const p: V3 = [at[0] + d[0] * out, at[1] + d[1] * out, at[2] + d[2] * out];
    const a0: V3 = Math.abs(d[1]) < 0.9 ? [d[2], 0, -d[0]] : [1, 0, 0], l = Math.hypot(...a0), u: V3 = [a0[0] / l, a0[1] / l, a0[2] / l];
    const w: V3 = [d[1] * u[2] - d[2] * u[1], d[2] * u[0] - d[0] * u[2], d[0] * u[1] - d[1] * u[0]];
    return blob(p, [u, w, d], [r, r, r * 0.25], (q) => 1 + (r > 0.008 ? 0.18 * Math.sin(7 * Math.atan2(q[1], q[0])) : 0), 18, 8);
  };
  const out = travelled >= skin.t1;
  const marks = [
    { key: "entry", g: mark(skin.entry, 0.0045, -0.001), c: "#1a0605" },
    ...(out ? [{ key: "exit", g: mark(skin.exit, 0.012, 0.001), c: "#3a0907" }] : []),
    ...(cloth > 0 ? [{ key: "entry-cloth", g: mark(skin.entry, 0.005, -cloth), c: "#0d0806" }, ...(travelled >= skin.t1 + cloth ? [{ key: "exit-cloth", g: mark(skin.exit, 0.014, cloth), c: "#160b08" }] : [])] : []),
  ];
  return (
    <>
      {marks.map((m) => (
        <Fragment key={m.key}>
          <Mesh3d name={`${name}-wound-${m.key}`}>
            <BufferMesh positions={m.g.positions} normals={m.g.normals} indices={m.g.indices} />
            <StandardMaterial base_color={m.c} perceptual_roughness={0.35} />
          </Mesh3d>
        </Fragment>
      ))}
    </>
  );
}

/** The bullet itself, at `at` heading along `dir`: a spitzer (pointed) round, by default the German S-Patrone's 7.92 mm
 *  across and 28 mm long, cupro-nickel jacketed; `glow` lights it (an X-ray shows metal brightest of all). */
export function Round({ at, dir, name, caliber = 0.00792, length = 0.028, glow: g = 0 }: { at: V3; dir: V3; name: string; caliber?: number; length?: number; glow?: number }) {
  const l = Math.hypot(...dir) || 1, d: V3 = [dir[0] / l, dir[1] / l, dir[2] / l];
  // (its body a cylinder, then the ogive narrowing to a point over the front 45%, as the spitzer's does)
  const R = caliber / 2, tail: V3 = [at[0] - d[0] * length / 2, at[1] - d[1] * length / 2, at[2] - d[2] * length / 2];
  const path = Array.from({ length: 16 }, (_, i) => [tail[0] + d[0] * length * i / 15, tail[1] + d[1] * length * i / 15, tail[2] + d[2] * length * i / 15] as V3);
  const r = (t: number) => { const x = t < 0.55 ? (t < 0.06 ? 0.86 + 2.3 * t : 1) : Math.sqrt(Math.max(0, 1 - ((t - 0.55) / 0.45) ** 2)); return [R * Math.max(x, 0.02), R * Math.max(x, 0.02)] as [number, number]; };
  const geo = tube(path, r, 14);
  const c = "#b9a27a";
  return (
    <Mesh3d name={`${name}-round`} allowOverlap>
      <BufferMesh positions={geo.positions} normals={geo.normals} indices={geo.indices} />
      <StandardMaterial base_color={c} metallic={0.9} perceptual_roughness={0.3} {...(g > 0 ? { emissive: glow("#ffe6b0", 4 * g) } : {})} />
    </Mesh3d>
  );
}
