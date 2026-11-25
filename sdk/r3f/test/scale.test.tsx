// An object scaled to 0 (#164): three draws it by its matrix, as nothing (or flat), where three's own Matrix4.decompose
// gives a matrix with no volume the scale 1 and no rotation. sdk/r3f reads transforms through decompose.ts, so a scale
// of 0 stays 0, and a flattened object keeps its turn.
import { describe, expect, test } from "bun:test";
import { useFrame } from "@react-three/fiber";
import { useRef } from "react";
import * as THREE from "three";
import { decompose } from "../src/input/decompose";
import { convert } from "../src/input/scene";
import { end, stepTo } from "../src/input/session";
import type { Part } from "../src/parts";
import type { ReactNode } from "react";

const flat = (parts: Part[]): Part[] => parts.flatMap((p) => [p, ...flat(p.children ?? [])]);
let n = 0;
function partsOf(scene: (i: number) => ReactNode, frame = 0) {
  const key = `scale${n++}`;
  const s = stepTo(key, frame, 24, 640, 360, {}, scene);
  const st = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
  const parts = flat(convert(st.scene, st.camera, st.gl, 360).parts);
  end(key);
  return parts;
}
const named = (parts: Part[], name: string) => parts.find((p) => String(p.props.name).split(":")[1]?.replace(/\/$/, "") === name)!;
/** a part's transform composed again, as raea's engine composes it (core/scene props.rs local) */
const matrixOf = (p: Part) => {
  const s = p.props.scale as number | number[];
  return new THREE.Matrix4().compose(new THREE.Vector3(...(p.props.position as number[])), new THREE.Quaternion(...(p.props.quaternion as number[])), typeof s === "number" ? new THREE.Vector3(s, s, s) : new THREE.Vector3(...s));
};
const same = (a: THREE.Matrix4, b: THREE.Matrix4) => a.elements.every((x, i) => Math.abs(x - b.elements[i]) < 1e-9);
const box = <><boxGeometry /><meshBasicMaterial /></>;

describe("decompose", () => {
  test("a matrix with volume: three's decompose, the same numbers", () => {
    const m = new THREE.Matrix4().compose(new THREE.Vector3(1, 2, 3), new THREE.Quaternion().setFromEuler(new THREE.Euler(0.3, -1.1, 2)), new THREE.Vector3(-2, 0.5, 1e-4));
    const [p, q, s, p3, q3, s3] = [new THREE.Vector3(), new THREE.Quaternion(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Quaternion(), new THREE.Vector3()];
    decompose(m, p, q, s);
    m.decompose(p3, q3, s3);
    expect([...p.toArray(), ...q.toArray(), ...s.toArray()]).toEqual([...p3.toArray(), ...q3.toArray(), ...s3.toArray()]);
  });

  test("a matrix with no volume composes back to itself: its scale 0, its turn kept", () => {
    const turn = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.7, 0.2, -1.3));
    const scales = [[0, 0, 0], [1, 0, 2], [0, 3, 0], [0, 0, 0.5], [-1, 0, 1], [2, 2, 0], [1e-110, 1e-110, -1e-110]];
    for (const sc of scales) {
      const m = new THREE.Matrix4().compose(new THREE.Vector3(4, -5, 6), turn, new THREE.Vector3(...sc));
      const [p, q, s] = [new THREE.Vector3(), new THREE.Quaternion(), new THREE.Vector3()];
      decompose(m, p, q, s);
      expect(p.toArray()).toEqual([4, -5, 6]);
      expect(Math.abs(q.length() - 1)).toBeLessThan(1e-12);
      // each axis's length (to rounding), an axis scaled to 0 exactly 0
      expect([sc, s.toArray().map((x, i) => Math.abs(Math.abs(x) - Math.abs(sc[i])) <= 1e-12 * Math.abs(sc[i]))]).toEqual([sc, [true, true, true]]);
      const back = new THREE.Matrix4().compose(p, q, s);
      // the tiny scale's elements are about 1e-110: compared relative to it
      expect(back.elements.every((x, i) => Math.abs(x - m.elements[i]) <= 1e-9 * Math.max(1e-200, ...sc.map(Math.abs)))).toBe(true);
    }
    // three's own gives each of these the scale 1, which drew a scale of 0 at full size
    const zero = new THREE.Vector3();
    new THREE.Matrix4().makeScale(0, 0, 0).decompose(new THREE.Vector3(), new THREE.Quaternion(), zero);
    expect(zero.toArray()).toEqual([1, 1, 1]);
  });
});

function PopIn() {
  const ref = useRef<THREE.Mesh>(null!);
  useFrame(() => ref.current.scale.setScalar(0));
  return <mesh ref={ref} name="frame">{box}</mesh>;
}

describe("an object scaled to 0 (#164)", () => {
  test("is sent scaled to 0: a group's scale={0}, a mesh's [0, 0, 0], scale.set, a pop-in before it starts, useFrame", () => {
    const set = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicMaterial());
    set.scale.set(0, 0, 0);
    const parts = partsOf((i) => (
      <>
        <group name="group" scale={0}><mesh>{box}</mesh></group>
        <mesh name="mesh" position={[1, 0, 0]} scale={[0, 0, 0]}>{box}</mesh>
        <primitive name="set" object={set} />
        <mesh name="pop" scale={Math.min(1, Math.max(0, (i - 10) / 10))}>{box}</mesh>
        <PopIn />
        <mesh name="half" scale={0.5}>{box}</mesh>
      </>
    ));
    for (const name of ["group", "mesh", "set", "pop", "frame"]) {
      const s = named(parts, name).props.scale;
      expect([name, s]).toEqual([name, [0, 0, 0]]);
    }
    expect(named(parts, "mesh").props.position).toEqual([1, 0, 0]);
    expect(named(parts, "half").props.scale).toBe(0.5);
  });

  test("a flattened object keeps its turn: scale [1, 0, 2] turned 45 degrees is the same matrix in raea", () => {
    const parts = partsOf(() => <mesh name="flat" position={[0, 1, 0]} rotation={[0, Math.PI / 4, 0]} scale={[1, 0, 2]}>{box}</mesh>);
    const p = named(parts, "flat");
    expect(p.props.scale).toEqual([1, 0, 2]);
    const three = new THREE.Matrix4().compose(new THREE.Vector3(0, 1, 0), new THREE.Quaternion().setFromEuler(new THREE.Euler(0, Math.PI / 4, 0)), new THREE.Vector3(1, 0, 2));
    expect(same(matrixOf(p), three)).toBe(true);
  });

  test("an instance scaled to 0 is shrunk to nothing, a sprite in a group scaled to 0 too", () => {
    const im = new THREE.InstancedMesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial(), 2);
    const m = new THREE.Matrix4();
    im.setMatrixAt(0, m.makeTranslation(1, 0, 0));
    im.setMatrixAt(1, m.makeScale(0, 0, 0).setPosition(2, 0, 0));
    const parts = partsOf(() => (
      <>
        <primitive object={im} />
        <group scale={0}><sprite name="sprite"><spriteMaterial /></sprite></group>
      </>
    ));
    const at = parts.find((p) => p.type === "Instances")!.props.at as { position: number[]; scale: number[] }[];
    expect(at[0].scale).toEqual([1, 1, 1]);
    expect(at[1].scale).toEqual([1e-5, 1e-5, 1e-5]);
    expect(at[1].position).toEqual([2, 0, 0]);
    expect(named(parts, "sprite").props.scale).toEqual([0, 0, 1]);
  });
});
