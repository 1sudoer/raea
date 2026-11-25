// What an R3F scene throws is the composition's error, and three's PMREMGenerator doesn't take the scene with it
// (#131: any error inside <R3F> was swallowed, the scene drew nothing and the render ended with 0 errors).
import { describe, expect, test } from "bun:test";
import { useThree } from "@react-three/fiber";
import { useEffect, useMemo, type ReactNode } from "react";
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { convert } from "../src/input/scene";
import { end, stepTo } from "../src/input/session";
import { standInPMREM } from "../src/input/pmrem";
import type { Part } from "../src/parts";

standInPMREM();

const flat = (parts: Part[]): Part[] => parts.flatMap((p) => [p, ...flat(p.children ?? [])]);
let n = 0;
function partsOf(scene: (i: number) => ReactNode, key = `e${n++}`) {
  const s = stepTo(key, 0, 30, 400, 400, {}, scene);
  const st = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
  return flat(convert(st.scene, st.camera, st.gl, 400).parts);
}
const Box = () => (
  <mesh>
    <boxGeometry />
    <meshBasicMaterial color="red" />
  </mesh>
);

describe("an error inside the scene", () => {
  test("thrown while rendering, it's thrown out of the frame with its message, and the next frame starts again", () => {
    const Boom = (): null => {
      throw new Error("mesh threw");
    };
    expect(() => partsOf(() => (<><Box /><Boom /></>), "boom")).toThrow("mesh threw");
    // the same key without the throw draws: the failed root was ended, not kept
    expect(partsOf(() => <Box />, "boom").some((p) => p.type === "Mesh")).toBe(true);
    end("boom");
  });

  test("thrown in an effect, it's thrown out too", () => {
    const Late = () => {
      useEffect(() => {
        throw new Error("effect threw");
      }, []);
      return null;
    };
    expect(() => partsOf(() => (<><Box /><Late /></>))).toThrow("effect threw");
  });
});

describe("three's PMREMGenerator", () => {
  const warnings = (fn: () => void) => {
    const warned: string[] = [];
    const warn = console.warn;
    console.warn = (m: string) => void warned.push(String(m));
    try {
      fn();
    } finally {
      console.warn = warn;
    }
    return warned;
  };

  test("fromScene of three's RoomEnvironment lights the scene as raea's <Environment room>, with its blur (#165)", () => {
    let parts: Part[] = [];
    const warned = warnings(() => {
      const Studio = () => {
        const { gl, scene } = useThree();
        useMemo(() => {
          const pm = new THREE.PMREMGenerator(gl);
          scene.environment = pm.fromScene(new RoomEnvironment(), 0.04).texture;
          scene.environmentIntensity = 0.8;
          scene.environmentRotation.set(0, 0.5, 0);
        }, [gl, scene]);
        return null;
      };
      parts = partsOf(() => (<><Studio /><Box /></>));
    });
    expect(parts.some((p) => p.type === "Mesh")).toBe(true);
    const env = parts.filter((p) => p.type === "Environment");
    expect(env.map((p) => p.props)).toEqual([{ room: true, blur: 0.04, intensity: 0.8, rotation: 0.5 }]);
    expect(warned.filter((w) => w.includes("environment"))).toEqual([]);
  });

  test("the room as the background too is drawn behind the scene; as the background alone it lights nothing", () => {
    const Room = ({ light }: { light: boolean }) => {
      const { gl, scene } = useThree();
      useMemo(() => {
        const t = new THREE.PMREMGenerator(gl).fromScene(new RoomEnvironment()).texture;
        if (light) scene.environment = t;
        scene.background = t;
      }, [gl, scene, light]);
      return null;
    };
    const both = partsOf(() => (<><Room light /><Box /></>)).find((p) => p.type === "Environment");
    expect(both?.props).toMatchObject({ room: true, blur: 0, intensity: 1, background: true, backgroundIntensity: 1 });
    const behind = partsOf(() => (<><Room light={false} /><Box /></>)).find((p) => p.type === "Environment");
    expect(behind?.props).toMatchObject({ room: true, intensity: 0, background: true });
  });

  test("fromScene of any other scene leaves the environment out with a warning and the scene draws", () => {
    let parts: Part[] = [];
    const warned = warnings(() => {
      const Own = () => {
        const { gl, scene } = useThree();
        useMemo(() => {
          const own = new THREE.Scene();
          own.add(new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicMaterial({ color: "white" })));
          scene.environment = new THREE.PMREMGenerator(gl).fromScene(own).texture;
        }, [gl, scene]);
        return null;
      };
      parts = partsOf(() => (<><Own /><Box /></>));
    });
    expect(parts.some((p) => p.type === "Mesh")).toBe(true);
    expect(parts.some((p) => p.type === "Environment")).toBe(false);
    expect(warned.some((w) => w.includes("PMREMGenerator.fromScene"))).toBe(true);
  });

  test("fromEquirectangular of a panorama lights the scene by that panorama", () => {
    const Sky = () => {
      const { gl, scene } = useThree();
      useMemo(() => {
        const hdr = new THREE.Texture();
        hdr.userData.src = "media/studio.hdr";
        scene.environment = new THREE.PMREMGenerator(gl).fromEquirectangular(hdr).texture;
      }, [gl, scene]);
      return null;
    };
    const env = partsOf(() => (<><Sky /><Box /></>)).find((p) => p.type === "Environment");
    expect(env?.props.src).toBe("media/studio.hdr");
  });
});

// docs/r3f.feature R3F16: what three asks of a renderer raea's stand-ins answer, so these draw (named where raea can't
// do all of it yet) instead of refusing the render; R3F17: a sky at three's far plane goes to reverse-Z's
describe("what the 11-15 audit found refused", () => {
  const quiet = <T,>(fn: () => T): { out: T; warned: string[] } => {
    const warned: string[] = [];
    const warn = console.warn;
    console.warn = (m: string) => void warned.push(String(m));
    try {
      return { out: fn(), warned };
    } finally {
      console.warn = warn;
    }
  };

  test("drei's <MeshReflectorMaterial> (it sets gl.state.buffers.depth) draws its plane", async () => {
    const { MeshReflectorMaterial } = await import("@react-three/drei");
    const { out } = quiet(() => partsOf(() => (<><Box /><mesh rotation={[-Math.PI / 2, 0, 0]}><planeGeometry args={[4, 4]} /><MeshReflectorMaterial resolution={64} mirror={0.8} /></mesh></>)));
    // the box and the plane on the screen; its mirrored camera's pass draws the box (the plane hides itself) into its
    // target on a layer of its own (R3F23)
    expect(out.filter((p) => p.type === "Mesh" && !p.props.layers).length).toBe(2);
    const pass = out.find((p) => typeof p.props.target === "string");
    expect(pass?.props.target).toMatch(/^render:.+:64x64(:float)?$/);
    expect(out.filter((p) => p.type === "Mesh" && (p.props.layers as number[] | undefined)?.[0] === (pass?.props.layers as number[])[0]).length).toBe(1);
  });

  test("drei's <Environment> with Lightformers (a CubeCamera reads the renderer's coordinate system) draws the scene", async () => {
    const { Environment, Lightformer } = await import("@react-three/drei");
    const { out } = quiet(() => partsOf(() => (<><Box /><Environment resolution={16}><Lightformer intensity={4} position={[0, 3, -3]} /></Environment></>)));
    expect(out.some((p) => p.type === "Mesh")).toBe(true);
  });

  test("a TSL node material on an InstancedMesh (three asks the backend's device for its limits) draws, flat past a uniform buffer", async () => {
    const { MeshBasicNodeMaterial } = await import("three/webgpu");
    const { color } = await import("three/tsl");
    for (const count of [100, 2000]) {
      const m = new MeshBasicNodeMaterial();
      m.colorNode = color(0.4, 0.8, 1);
      const { out } = quiet(() => partsOf(() => <instancedMesh args={[new THREE.BoxGeometry(0.1, 0.1, 0.1), m, count]} />));
      expect(out.some((p) => p.type === "Mesh" || p.type === "Instances")).toBe(true);
    }
  });

  test("gl.compute of a node raea can't run is named, not thrown (a TSL ComputeNode runs: test/compute.test.tsx)", () => {
    const { useFrame } = require("@react-three/fiber") as typeof import("@react-three/fiber");
    const Dispatch = () => {
      useFrame(({ gl }) => (gl as unknown as { compute(n: unknown): void }).compute({ isComputeNode: true }));
      return <Box />;
    };
    const { out: s, warned } = quiet(() => (stepTo("compute", 0, 30, 400, 400, {}, () => <Dispatch />), stepTo("compute", 1, 30, 400, 400, {}, () => <Dispatch />)));
    const gl = (s.state as unknown as { gl: { __raea: { computes: unknown[] } } }).gl;
    expect(gl.__raea.computes.length).toBe(0);
    expect(warned.filter((w) => /gl\.compute dispatches a compute raea can't run/.test(w))).toHaveLength(1);
    expect(warned.every((w) => !/TypeError/.test(w))).toBe(true);
    end("compute");
  });

  test("drei's <Stars> (a ShaderMaterial on Points with a sampler it never sets) has that sampler read as opaque black", async () => {
    const { Stars } = await import("@react-three/drei");
    const { out } = quiet(() => partsOf(() => <Stars count={50} />));
    const shader = out.find((p) => p.type === "ThreeShaderMaterial");
    expect(shader).toBeDefined();
    expect(String((shader!.props.textures as Record<string, string>).pointTexture)).toMatch(/^data:image\/png;base64,/);
  });

  test("three's <Sky> writes its far plane as reverse-Z's (z = 0), not z = w, which Bevy's projection puts on the near plane", async () => {
    const { Sky } = await import("@react-three/drei");
    const { out } = quiet(() => partsOf(() => <Sky sunPosition={[100, 10, 100]} />));
    const shader = out.find((p) => p.type === "ThreeShaderMaterial");
    const vertex = String((shader!.props.glsl as { vertex: string }).vertex);
    expect(vertex).toContain("gl_Position.z = 0.0;");
    expect(vertex).not.toMatch(/gl_Position\.z\s*=\s*gl_Position\.w/);
  });
});
