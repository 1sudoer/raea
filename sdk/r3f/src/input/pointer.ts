// The pointer in a live play (docs/r3f.feature R3F27): raea's input, frame by frame, as the DOM events a browser would
// have sent R3F's canvas. R3F's own event manager (its `events`, connected to the stand-in canvas) raycasts each one
// through the R3F camera and calls onClick, onPointerOver, onPointerMove and the rest on the mesh it hits; drei's
// <OrbitControls> (three's controls) listen on the same canvas and its document. The events come from the input
// timeline, a frame's against the frame before, so a render with no pointer sends none, and a root stepped again from
// frame 0 gets the same events on the same frames: a recorded input log replays to the same frames.
import type * as THREE from "three";
import type { FrameInput } from "@raea/app/clock";
import { hasNative, native } from "@raea/app/native";

type Listener = (e: unknown) => void;

/** a DOM event as R3F and three's controls read it: its type, the pointer, the buttons and the wheel */
export type DomEvent = Record<string, unknown> & { type: string };

/** a minimal EventTarget: listeners by type, called in the order added (an object's handleEvent as one function) */
class Target {
  private on = new Map<string, Set<Listener>>();
  private fnOf(fn: Listener | { handleEvent: Listener; __raea?: Listener }): Listener {
    return typeof fn === "function" ? fn : (fn.__raea ??= (e) => fn.handleEvent(e));
  }
  addEventListener(type: string, fn: Listener | { handleEvent: Listener } | null) {
    if (!fn) return;
    let set = this.on.get(type);
    if (!set) this.on.set(type, (set = new Set()));
    set.add(this.fnOf(fn));
  }
  removeEventListener(type: string, fn: Listener | { handleEvent: Listener } | null) {
    if (fn) this.on.get(type)?.delete(this.fnOf(fn));
  }
  dispatchEvent(e: { type: string }): boolean {
    for (const f of [...(this.on.get(e.type) ?? [])]) f(e);
    return true;
  }
}

/** What R3F and three's controls are given as the canvas: an event target the size of the <R3F>, whose document gets
 *  the events that bubble (a control's pointermove and pointerup after its pointerdown) */
export class EventCanvas extends Target {
  style: Record<string, string> = {};
  readonly ownerDocument = Object.assign(new Target(), { body: {}, documentElement: {} });
  constructor(public width: number, public height: number) {
    super();
  }
  get clientWidth() {
    return this.width;
  }
  get clientHeight() {
    return this.height;
  }
  getBoundingClientRect() {
    const { width, height } = this;
    return { left: 0, top: 0, width, height, x: 0, y: 0, right: width, bottom: height };
  }
  getRootNode() {
    return this.ownerDocument;
  }
  setPointerCapture() {}
  releasePointerCapture() {}
  hasPointerCapture() {
    return false;
  }
  focus() {}
  getContext() {
    return null;
  }
}

/** the events that bubble from the canvas to its document */
const BUBBLES = new Set(["pointerdown", "pointermove", "pointerup", "click", "contextmenu", "wheel"]);

/** a DOM button's number, by Bevy's MouseButton name (FrameInput's `buttons`) */
const BUTTON: Record<string, number> = { Middle: 1, Right: 2, Back: 3, Forward: 4 };

/** the DOM's `buttons` bitmask for what's held */
function held(i: FrameInput | undefined): number {
  let b = i?.pointer?.down ? 1 : 0;
  for (const n of i?.buttons ?? []) b |= n === "Right" ? 2 : n === "Middle" ? 4 : n === "Back" ? 8 : n === "Forward" ? 16 : 0;
  return b;
}

/** The DOM events that take R3F's canvas from input `prev` to `now`, in the order a browser sends them: the move, the
 *  presses, the releases (a primary release a click as well, a right press the context menu), the wheel; the pointer
 *  gone, a pointerleave. x and y are frame px, the canvas's own when the <R3F> fills the frame. */
export function eventsBetween(prev: FrameInput | undefined, now: FrameInput | undefined): DomEvent[] {
  const p = now?.pointer ?? prev?.pointer;
  if (!p) return [];
  const keys = now?.keys ?? [];
  const base = (type: string, button = 0, buttons = held(now)) => ({
    type, offsetX: p.x, offsetY: p.y, clientX: p.x, clientY: p.y, pageX: p.x, pageY: p.y, screenX: p.x, screenY: p.y,
    button, buttons, pointerId: 1, pointerType: "mouse", isPrimary: true, deltaX: 0, deltaY: 0, deltaMode: 0,
    shiftKey: keys.includes("Shift"), ctrlKey: keys.includes("Control"), altKey: keys.includes("Alt"), metaKey: keys.includes("Meta"),
  });
  if (!now?.pointer) return [base("pointerleave", 0, 0)];
  const out: DomEvent[] = [];
  const was = prev?.pointer;
  if (!was || was.x !== p.x || was.y !== p.y) out.push(base("pointermove", -1));
  // presses before releases: a button pressed and another let go in one frame
  if (p.down && !was?.down) out.push(base("pointerdown", 0));
  for (const n of now.buttons ?? []) if (!prev?.buttons?.includes(n) && BUTTON[n] !== undefined) {
    out.push(base("pointerdown", BUTTON[n]));
    if (n === "Right") out.push(base("contextmenu", 2));
  }
  if (!p.down && was?.down) out.push(base("pointerup", 0), base("click", 0));
  for (const n of prev?.buttons ?? []) if (!now.buttons?.includes(n) && BUTTON[n] !== undefined) out.push(base("pointerup", BUTTON[n]));
  if (now.wheel && (now.wheel.x || now.wheel.y)) out.push({ ...base("wheel", 0), deltaX: now.wheel.x, deltaY: now.wheel.y });
  return out;
}

/** The box the <R3F>'s camera draws its view in, [x, y, w, h] in frame px, as the page last laid it out: a camera in a
 *  page element picks in its box (sdk/render events3d's page.viewBox). None when it fills the frame, or from a host
 *  that can't say */
export function viewBox(): number[] | null {
  if (!hasNative()) return null;
  try {
    return (native as (n: string, a: object) => number[] | null)("page.viewBox", {});
  } catch {
    return null;
  }
}

/** `i` with its pointer in R3F's canvas px (`size`): the frame's, or in a box, the box's, scaled to R3F's size. Off
 *  the box, the pointer is gone, unless it's pressed (a drag goes on outside the canvas, as a browser's does) */
export function inCanvas(i: FrameInput | undefined, size: [number, number], box: number[] | null): FrameInput | undefined {
  const p = i?.pointer;
  if (!p || !box) return i;
  const [x, y, w, h] = box;
  const off = p.x < x || p.y < y || p.x >= x + w || p.y >= y + h;
  if (off && !p.down) return { ...i, pointer: undefined };
  return { ...i, pointer: { x: ((p.x - x) * size[0]) / w, y: ((p.y - y) * size[1]) / h, down: p.down } };
}

/** Each event dispatched on `canvas` (and its document, for those that bubble), with the scene's and camera's matrices
 *  as the last frame left them (three's renderer updates them as it draws, before an event raycasts against them) */
export function dispatchInput(canvas: EventCanvas, drawn: (THREE.Object3D | undefined)[], events: DomEvent[]) {
  if (!events.length) return;
  for (const o of drawn) o?.updateMatrixWorld();
  for (const raw of events) {
    let stopped = false;
    const e = Object.assign(raw, {
      target: canvas, currentTarget: canvas, timeStamp: 0,
      preventDefault() {}, stopPropagation() { stopped = true; },
    });
    canvas.dispatchEvent(e);
    if (BUBBLES.has(e.type) && !stopped) canvas.ownerDocument.dispatchEvent(e);
  }
}
