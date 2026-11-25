// <R3F>: React Three Fiber's scene drawn by raea. Its children are R3F's JSX (<mesh>, <boxGeometry>,
// <meshStandardMaterial>, useFrame, useThree, any three.js object by <primitive>), run by R3F's own reconciler headless
// (session.ts), and each frame its three.js scene becomes raea's parts (scene.ts), which raea's engine draws: Bevy in
// a render, a live play and the web export alike. It takes <Canvas>'s settings (camera, orthographic, shadows, flat,
// linear, scene); raea's hooks work inside it (bridge.tsx), and useFrame steps 1/fps a frame, from the composition's
// frame, never the wall clock.
import { useContext, useId, type ReactNode } from "react";
import { AbsoluteFill, useCurrentFrame, useVideoConfig } from "raea";
import { ClockContext, LocalTimeContext, type FrameInput } from "@raea/app/clock";
import { wireKeys } from "@raea/app/games";
import type * as THREE from "three";
import { FiberOf, useBridge } from "./bridge";
import { Held, useWake } from "./hold";
import { inCanvas, viewBox } from "./pointer";
import { stepTo, type Config } from "./session";
import { convert } from "./scene";
import { standInImages } from "./texture";
import { standInPMREM } from "./pmrem";
import { elementsOf } from "../parts";

standInImages();
standInPMREM();

export interface R3FProps extends Config {
  children?: ReactNode;
  /** a name of its own when one composition has more than one R3F scene that come and go */
  name?: string;
  /** the box's width and height in frame pixels, when the <R3F> sits in a box smaller than the frame: R3F's size
   *  (useThree's size and viewport), its camera's aspect and an orthographic camera's frame are the box's. raea draws
   *  the scene in the box it sits in either way (#130); by default R3F's size is the frame's */
  width?: number;
  height?: number;
}

function Inner({ fiber, children, name, width: boxW, height: boxH, ...config }: R3FProps & { fiber: Parameters<typeof useBridge>[0] }) {
  const frame = useCurrentFrame();
  const video = useVideoConfig();
  const { fps } = video;
  for (const [k, v] of [["width", boxW], ["height", boxH]] as const) if (v !== undefined && !(v > 0 && Number.isFinite(v))) throw new RangeError(`<R3F>: ${k} is the box's size in frame pixels, above 0, got ${v}`);
  const [width, height] = [boxW ?? video.width, boxH ?? video.height];
  const clock = useContext(ClockContext);
  const local = useContext(LocalTimeContext);
  const bridge = useBridge(fiber);
  // drawn again when a load it waited on comes (hold.tsx): the frame isn't taken before
  useWake();
  const id = useId();
  // the scene at local frame i: raea's clock as it was then, inside every context from outside
  const at = (i: number) => {
    const c = clock ? { ...clock, frame: clock.frame - frame + i } : null;
    const l = local ? { ...local, frame: i } : null;
    return bridge(
      <ClockContext value={c}>
        <LocalTimeContext value={l}>
          <Held>{children}</Held>
        </LocalTimeContext>
      </ClockContext>,
    );
  };
  // the input at local frame i, from raea's input timeline (by composition frame): sent to R3F's canvas as the pointer
  // events of a live play, or of a render given an input log, in the box the page laid out for it (pointer.ts)
  let box: number[] | null | undefined;
  const input = clock && ((i: number) => {
    const e = clock.input[clock.frame - frame + i];
    if (!e) return undefined;
    if (box === undefined) box = viewBox();
    return inCanvas(wireKeys({ ...e, keys: Array.isArray(e.keys) ? [...e.keys] : [] } as FrameInput), [width, height], box);
  });
  const s = stepTo(name ?? id, frame, fps, width, height, config, at, input ?? undefined);
  const { scene, camera, gl } = s.state as unknown as { scene: THREE.Scene; camera: THREE.Camera; gl: THREE.WebGLRenderer };
  // R3F's clock: what a node material's `time` reads (useFrame's own steps, 1/fps a frame)
  const r3fClock = (s.state as unknown as { clock: { elapsedTime: number } }).clock;
  const out = convert(scene, camera, gl, height, r3fClock.elapsedTime, 1 / fps);
  const drawn = elementsOf(out.parts, "r3f.");
  return out.background ? <AbsoluteFill style={{ background: out.background }}>{drawn}</AbsoluteFill> : <>{drawn}</>;
}

/** React Three Fiber's <Canvas>, drawn by raea */
export function R3F(props: R3FProps) {
  return <FiberOf>{(fiber) => <Inner fiber={fiber} {...props} />}</FiberOf>;
}
