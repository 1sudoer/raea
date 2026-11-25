// raea/wear's demo (docs/wear.feature WR7): the corner house (house.ts) baked once and shaded. It is drawn new,
// after 80 years of a coal-burning city, and as a ruin. Each cause appears in false colour, and the house stands
// under each of WR9's eight conditions by day and at dusk. A shower crosses it as a timelapse (rain-passes). `paint`
// is the other wear, worn paint with no bake (<WearExtension>, a material's `wear`; docs/examples.feature EX9).
//   bun examples/materials/wear/make-house.ts            (once: writes media/house.glb)
//   raea render examples/materials/wear/wear.tsx --composition after
// The south front faces +z and the east front +x, so north is -z. The corner stands at (12, 0, 0).
import { AbsoluteFill, Composition, registerRoot, staticFile, useCurrentFrame } from "raea";
import { Cuboid, Cylinder, Model, ToneMapping } from "raea/3d";
import { ExtendedMaterial, ShaderExtension, StandardMaterial, WearExtension, type WearExtensionProps } from "raea/pbr";
import { Camera3d } from "raea/camera";
import { AmbientLight, DirectionalLight } from "raea/light";
import { Mesh3d } from "raea/mesh";
import { Quat, Transform } from "raea/transform";
import { WEAR_CONDITIONS, wearBake, wearDebris, wearShader, weatherAt, type WearAge, type WearCause, type WearCondition, type WearShaderOptions, type WearSpell } from "raea/wear";
import { CLASSES, SOUTH } from "./house";

type V3 = [number, number, number];

/** The house's bake: each material is its own class, Berlin's latitude, the walk round the corner trodden. */
export function bakeHouse() {
  return wearBake({
    src: staticFile("house.glb"),
    env: { up: [0, 1, 0], north: [0, 0, -1], density: 16, walk: ["ground"], sun: { lat: 52.5 } },
    classes: Object.fromEntries(CLASSES.map((c) => [c, c])),
  });
}

/** The ages the demo shows: a fresh coat, 80 years in a coal-burning city, and 90 years left to go. */
export const AGES = {
  new: { years: 0 },
  old: { years: 80, soot: 0.85, upkeep: 0.35 },
  ruin: { years: 90, soot: 0.85, upkeep: 0.05 },
} as const satisfies Record<string, WearAge>;

/** The light for a time of day and the sky's weather: the sun and its colour, the sky's fill, the backdrop. */
function lightOf(dusk: boolean, sky: "clear" | "overcast" | "storm") {
  // the sun from the south-west, so the south front is lit and the east one in shade; at dusk low in the west
  const sun: V3 = dusk ? [-30, 4, 12] : [-8, 14, 20];
  const dim = sky === "clear" ? 1 : sky === "overcast" ? 0.25 : 0.1;
  return {
    sun,
    lux: (dusk ? 1800 : 9000) * dim,
    color: dusk ? "#ffb070" : "#f2ece0",
    fill: (dusk ? 140 : 500) * (sky === "clear" ? 1 : 1.4),
    fillColor: dusk ? "#5a6890" : sky === "clear" ? "#c4ccd6" : "#a9b0b8",
    backdrop: dusk ? (sky === "clear" ? "#3a4568" : "#2a2f3a") : sky === "clear" ? "#aab8c8" : "#8c939b",
  };
}

const SKY: Record<WearCondition, "clear" | "overcast" | "storm"> = { spring: "clear", summer: "clear", autumn: "overcast", winter: "clear", snow: "overcast", blizzard: "storm", rain: "overcast", thunder: "storm" };

interface HouseProps {
  look?: Omit<WearShaderOptions, "baked">;
  debug?: WearCause;
  dusk?: boolean;
  sky?: "clear" | "overcast" | "storm";
  eye?: V3;
  at?: V3;
  fov?: number;
}

/** The corner house from across the junction, weathered by `look` (or a cause in false colour), with the debris its
 *  age has brought down on the walk. */
export function House({ look, debug, dusk = false, sky = "clear", eye = [24, 4, 17], at = [9, 6.5, -2], fov = 52 }: HouseProps) {
  const baked = bakeHouse();
  const shader = look || debug ? wearShader({ baked, ...look, debug }) : undefined;
  const debris = look?.age && !debug ? wearDebris(baked, look.age) : [];
  const l = lightOf(dusk, sky);
  const flat = debug !== undefined;
  return (
    <AbsoluteFill style={{ background: flat ? "#202020" : l.backdrop }}>
      <>
        <Camera3d transform={Transform.fromXyz(...eye).lookingAt(at)} projection={{ Perspective: { fov: (fov * Math.PI) / 180, near: 0.1, far: 300 } }} />
        <ToneMapping mode={flat ? "none" : "agx"} />
        <AmbientLight brightness={flat ? 0 : l.fill} color={l.fillColor} />
        <DirectionalLight transform={Transform.fromXyz(...l.sun).lookingAt([6, 4, -4])} illuminance={flat ? 0 : l.lux} color={l.color} shadow_maps_enabled />
        <Model name={`corner-house-${debug ?? "look"}`} src={baked.file}>
          {shader ? <ExtendedMaterial base={<StandardMaterial />} extension={<ShaderExtension {...shader} />} /> : null}
        </Model>
        {debris.flatMap((d, i) =>
          // a heap is a few lumps of render lying along its front's foot (the east front's runs along z), more and
          // bigger under a looser coat; each rests on the walk, which the clipping check reads as passing into the house
          lumps(d.loss ?? 0.5, i).map(([u, w, size, yaw], k) => {
            const alongX = d.at[0] <= SOUTH.x1;
            const half: V3 = [size * (alongX ? 1.6 : 1), size * 0.5, size * (alongX ? 1 : 1.6)];
            return (
              <Mesh3d key={`${i}-${k}`} allowOverlap transform={Transform.fromXyz(d.at[0] + (alongX ? u : w), d.at[1] + half[1], d.at[2] + (alongX ? w : u)).withRotation(Quat.fromEuler("XYZ", 0, yaw, 0))}>
                <Cuboid half_size={half} />
                <StandardMaterial base_color={k % 2 ? "#b8ab92" : "#8f8270"} perceptual_roughness={0.95} />
              </Mesh3d>
            );
          }),
        )}
      </>
    </AbsoluteFill>
  );
}

/** A heap's lumps: [along the wall, out from it, size, yaw], three to five by its loss potential, placed by its index. */
function lumps(loss: number, seed: number): [number, number, number, number][] {
  const r = (k: number) => {
    const x = Math.sin(seed * 12.9898 + k * 78.233) * 43758.5453;
    return x - Math.floor(x);
  };
  const n = 3 + Math.round(2 * loss);
  return Array.from({ length: n }, (_, k) => [(r(4 * k) - 0.5) * 0.5, (r(4 * k + 1) - 0.5) * 0.18, 0.025 + 0.04 * r(4 * k + 2) * loss, (r(4 * k + 3) - 0.5) * 1.4]);
}

const old = { age: AGES.old };
/** Where the losses are easiest to read: the east front's ground floor and its foot, up close from the walk. */
const NEAR = { eye: [16.8, 1.7, -0.8] as V3, at: [12, 1.2, -4.6] as V3, fov: 50 };
const DOOR = { eye: [8.5, 1.8, 6.5] as V3, at: [6, 1.9, 0] as V3, fov: 50 };

/** A shower crossing the house: dry, raining from minute 6 to 40, drying after. A frame is half a minute. */
export const SHOWER: WearSpell[] = [{ at: 0 }, { at: 6, rain: 1 }, { at: 40 }];

function RainPasses() {
  const minutes = useCurrentFrame() * 0.5;
  const weather = weatherAt({ day: 130 }, SHOWER, minutes);
  const raining = minutes >= 6 && minutes < 40;
  return <House look={{ ...old, weather }} sky={raining ? "overcast" : "clear"} />;
}

/** Worn paint with no bake: street iron painted green, its edges chipped to the metal, grime low and rust run down,
 *  by how worn each is (a bollard, a cabinet turned on the walk, a railing post). */
export const PAINTED: { at: V3; yaw: number; shape: "bollard" | "cabinet" | "post"; wear: WearExtensionProps }[] = [
  { at: [-1.4, 0, 0], yaw: 0, shape: "bollard", wear: { amount: 0.5 } },
  { at: [0.2, 0, -0.3], yaw: 0.6, shape: "cabinet", wear: { amount: 0.8, grime: 0.7, seed: 3 } },
  { at: [1.6, 0, 0.2], yaw: 0, shape: "post", wear: { amount: 1, chips: 0.9, rust: 0.7, rust_color: "#7e4220", seed: 5 } },
];

function Paint() {
  const green = <StandardMaterial base_color="#2e5a3c" perceptual_roughness={0.45} metallic={0.1} />;
  return (
    <AbsoluteFill style={{ background: "#b9c2cc" }}>
      <>
        <Camera3d transform={Transform.fromXyz(0.4, 1.6, 4.6).lookingAt([0.2, 0.6, 0])} projection={{ Perspective: { fov: 0.7, near: 0.05, far: 50 } }} />
        <ToneMapping mode="agx" />
        <AmbientLight brightness={500} color="#c4ccd6" />
        <DirectionalLight transform={Transform.fromXyz(-3, 5, 4).lookingAt([0, 0, 0])} illuminance={9000} color="#f2ece0" shadow_maps_enabled />
        {PAINTED.map(({ at, yaw, shape, wear }) => (
          <Mesh3d key={shape} transform={Transform.fromXyz(at[0], at[1] + (shape === "cabinet" ? 0.6 : shape === "post" ? 0.55 : 0.45), at[2]).withRotation(Quat.fromEuler("XYZ", 0, yaw, 0))}>
            {shape === "cabinet" ? <Cuboid half_size={[0.4, 0.6, 0.22]} /> : <Cylinder radius={shape === "post" ? 0.06 : 0.11} half_height={shape === "post" ? 0.55 : 0.45} />}
            <ExtendedMaterial base={green} extension={<WearExtension {...wear} />} />
          </Mesh3d>
        ))}
        <Mesh3d transform={Transform.fromXyz(0, -0.05, 0)}><Cuboid half_size={[4, 0.05, 3]} /><StandardMaterial base_color="#8d8a84" perceptual_roughness={0.9} /></Mesh3d>
      </>
    </AbsoluteFill>
  );
}

const CAUSES: WearCause[] = ["class", "sky", "sun", "cavity", "edge", "flow", "deposit", "touch", "ground", "element", "open", "winter", "drift", "litter", "crack", "loss"];
const CONDITIONS = Object.keys(WEAR_CONDITIONS) as WearCondition[];

export const SHOTS: Record<string, () => JSX.Element> = {
  before: () => <House />,
  new: () => <House look={{ age: AGES.new }} />,
  after: () => <House look={{ ...old, weather: { day: 172 } }} />,
  ruin: () => <House look={{ age: AGES.ruin, weather: { day: 172 } }} />,
  "ruin-near": () => <House look={{ age: AGES.ruin, weather: { day: 172 } }} {...NEAR} />,
  door: () => <House look={{ ...old, weather: { day: 172 } }} {...DOOR} />,
  paint: () => <Paint />,
  ...Object.fromEntries(CAUSES.map((c) => [`debug-${c}`, () => <House debug={c} />])),
  ...Object.fromEntries(CONDITIONS.flatMap((c) => [
    [c, () => <House look={{ ...old, weather: WEAR_CONDITIONS[c] }} sky={SKY[c]} />],
    [`${c}-dusk`, () => <House look={{ ...old, weather: WEAR_CONDITIONS[c] }} sky={SKY[c]} dusk />],
  ])),
};

registerRoot(() => (
  <>
    {Object.entries(SHOTS).map(([id, Shot]) => (
      <Composition key={id} id={id} width={1280} height={800} fps={30} durationInFrames={1} component={Shot} />
    ))}
    <Composition id="rain-passes" width={1280} height={800} fps={30} durationInFrames={120} component={RainPasses} />
  </>
));
