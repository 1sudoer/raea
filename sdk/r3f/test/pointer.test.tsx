// R3F's pointer events and controls in a live play (docs/r3f.feature R3F27, src/input/pointer.ts): raea's input, frame
// by frame, sent to R3F's canvas as a browser's DOM events, so R3F's handlers fire on the mesh under the pointer and
// drei's <OrbitControls> turn the camera; no input sends nothing, and the same input stepped again gives the same frames.
import { describe, expect, test } from "bun:test";
import { OrbitControls } from "@react-three/drei";
import type { ReactNode } from "react";
import * as THREE from "three";
import type { FrameInput } from "@raea/app/clock";
import { eventsBetween, inCanvas } from "../src/input/pointer";
import { end, stepTo } from "../src/input/session";

const at = (x: number, y: number, down = false, more: Partial<FrameInput> = {}): FrameInput => ({ keys: [], pointer: { x, y, down }, ...more });
const types = (prev: FrameInput | undefined, now: FrameInput | undefined) => eventsBetween(prev, now).map((e) => `${e.type}:${e.button}`);

describe("raea's input as DOM events", () => {
  test("a move, a press, a release and its click, in a browser's order", () => {
    expect(types(undefined, at(10, 20))).toEqual(["pointermove:-1"]);
    expect(types(at(10, 20), at(10, 20))).toEqual([]);
    expect(types(at(10, 20), at(12, 20, true))).toEqual(["pointermove:-1", "pointerdown:0"]);
    expect(types(at(12, 20, true), at(12, 20))).toEqual(["pointerup:0", "click:0"]);
    expect(eventsBetween(at(0, 0), at(30, 40))[0]).toMatchObject({ offsetX: 30, offsetY: 40, clientX: 30, pointerType: "mouse", buttons: 0 });
  });

  test("the other buttons, the wheel, the modifier keys and the pointer gone", () => {
    expect(types(at(5, 5), at(5, 5, false, { buttons: ["Right"] }))).toEqual(["pointerdown:2", "contextmenu:2"]);
    expect(types(at(5, 5, false, { buttons: ["Right"] }), at(5, 5))).toEqual(["pointerup:2"]);
    const wheel = eventsBetween(at(5, 5), at(5, 5, false, { wheel: { x: 0, y: 120 } }));
    expect(wheel).toMatchObject([{ type: "wheel", deltaY: 120 }]);
    expect(eventsBetween(at(5, 5), at(6, 5, true, { keys: ["Shift"] }))[1]).toMatchObject({ type: "pointerdown", shiftKey: true, buttons: 1 });
    expect(types(at(5, 5), undefined)).toEqual(["pointerleave:0"]);
    expect(types(undefined, undefined)).toEqual([]);
  });

  test("in a box, the pointer is the box's, scaled to R3F's size; off it, gone unless pressed", () => {
    const box = [100, 50, 400, 300];
    expect(inCanvas(at(300, 200), [400, 300], box)!.pointer).toEqual({ x: 200, y: 150, down: false });
    expect(inCanvas(at(300, 200), [800, 600], box)!.pointer).toEqual({ x: 400, y: 300, down: false });
    expect(inCanvas(at(20, 20), [400, 300], box)!.pointer).toBeUndefined();
    expect(inCanvas(at(20, 20, true), [400, 300], box)!.pointer).toEqual({ x: -80, y: -30, down: true });
    expect(inCanvas(at(20, 20), [400, 300], null)!.pointer).toEqual({ x: 20, y: 20, down: false });
  });
});

let n = 0;
/** the root stepped to `frame` with input `input(i)` at frame i (400 x 400, R3F's default camera at z 5) */
function stepped(scene: () => ReactNode, frame: number, input?: (i: number) => FrameInput | undefined, key = `pointer${n++}`) {
  return stepTo(key, frame, 30, 400, 400, {}, scene, input);
}

describe("R3F's events on the mesh under the pointer", () => {
  const said: string[] = [];
  const Box = () => (
    <mesh
      onPointerOver={() => said.push("over")}
      onPointerOut={() => said.push("out")}
      onPointerDown={() => said.push("down")}
      onClick={(e) => said.push(`click ${e.object.type} ${e.point.z.toFixed(2)}`)}
      onPointerMissed={() => said.push("missed")}
    >
      <boxGeometry />
      <meshBasicMaterial />
    </mesh>
  );
  // over the box from frame 1, pressed at 3, let go at 4, off it at 6, a press and release there at 7 and 8
  const script = (i: number): FrameInput | undefined =>
    i < 1 ? undefined : i < 6 ? at(200, 200, i === 3) : at(20, 20, i === 7);

  test("over, down, click with the hit's point, out, and a click on nothing missed", () => {
    said.length = 0;
    const key = "pointer-box";
    stepped(Box, 9, script, key);
    end(key);
    expect(said).toEqual(["over", "down", "click Mesh 0.50", "out", "missed"]);
  });

  test("a render with no input sends nothing, and the same input stepped again says the same", () => {
    said.length = 0;
    stepped(Box, 9);
    expect(said).toEqual([]);
    const once = (key: string) => {
      said.length = 0;
      stepped(Box, 9, script, key);
      end(key);
      return [...said];
    };
    expect(once("pointer-a")).toEqual(once("pointer-b"));
  });
});

describe("drei's <OrbitControls>", () => {
  const scene = () => (
    <>
      <OrbitControls />
      <mesh><boxGeometry /><meshBasicMaterial /></mesh>
    </>
  );
  // a drag 100 px to the right from frame 2 to 6, then a turn of the wheel at 8
  const drag = (i: number): FrameInput => at(200 + Math.max(0, Math.min(i, 6) - 2) * 25, 200, i >= 2 && i < 6, i === 8 ? { wheel: { x: 0, y: 100 } } : {});
  const cameraAt = (input?: (i: number) => FrameInput, frame = 9) => {
    const key = `orbit${n++}`;
    const s = stepped(scene, frame, input, key);
    const c = (s.state.camera as THREE.Camera).position.clone();
    end(key);
    return c;
  };

  test("a drag turns the camera around the target and the wheel moves it back; no input leaves it", () => {
    const still = cameraAt();
    expect(still.toArray().map((v) => +v.toFixed(3))).toEqual([0, 0, 5]);
    const turned = cameraAt(drag, 7);
    expect(turned.length()).toBeCloseTo(5, 3);
    expect(turned.x).toBeLessThan(-1);
    const wheeled = cameraAt(drag, 9);
    expect(wheeled.length()).toBeGreaterThan(5.2);
  });

  test("the same drag stepped again ends in the same place", () => {
    expect(cameraAt(drag).toArray()).toEqual(cameraAt(drag).toArray());
  });
});
