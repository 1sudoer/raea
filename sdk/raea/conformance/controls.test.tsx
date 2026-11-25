// A composition's controls (docs/realtime.feature RT4a, RT4c): each names a defaultProps key and fits its kind, the
// registry hands them on with their cost said, and useControl reads the value the frame is drawn with.
import { beforeEach, expect, test } from "bun:test";
import { Composition, Sequence, getCompositions, registerRoot, useControl, useCurrentFrame } from "../src";
import { renderFrame, resetRegistry } from "../src/test";

beforeEach(() => resetRegistry());
function Card() { useCurrentFrame(); return null; }
const base = { id: "card", component: Card, width: 640, height: 360, fps: 30, durationInFrames: 60 } as const;
const register = (props: Record<string, unknown>) => registerRoot(() => <Composition {...base} {...(props as object)} />);

test("each kind of control is registered with its cost, in the order declared", () => {
  register({
    defaultProps: { warmth: 0.2, rain: false, model: "goose", tint: "#ff8800", title: "Swamp", size: 2 },
    controls: {
      warmth: { kind: "number", min: -1, max: 1, step: 0.05, group: "Grade" },
      rain: { kind: "switch", cost: "resim" },
      model: { kind: "choice", options: ["goose", "mayor"], cost: "reopen" },
      tint: { kind: "colour", label: "Tint" },
      title: { kind: "text", maxLength: 40 },
    },
  });
  const c = getCompositions()[0];
  expect(Object.keys(c.controls)).toEqual(["warmth", "rain", "model", "tint", "title"]);
  expect(c.controls.warmth).toEqual({ kind: "number", min: -1, max: 1, step: 0.05, group: "Grade", cost: "live" });
  expect(c.controls.rain.cost).toBe("resim");
  expect(c.controls.model).toMatchObject({ kind: "choice", options: ["goose", "mayor"], cost: "reopen" });
  expect(c.controls.tint).toMatchObject({ kind: "colour", label: "Tint", cost: "live" });
  // a prop with no control stays a prop
  expect(c.controls.size).toBeUndefined();
});

test("a composition with no controls registers none", () => {
  register({ defaultProps: { hue: 210 } });
  expect(getCompositions()[0].controls).toEqual({});
  resetRegistry();
  registerRoot(() => <Card />);
  expect(getCompositions()[0].controls).toEqual({});
});

test("a control names a defaultProps key", () => {
  expect(() => register({ defaultProps: { hue: 210 }, controls: { warmth: { kind: "number" } } })).toThrow(
    'Composition "card": controls.warmth names no defaultProps key: a control tunes a prop the composition has a value for (defaultProps has hue)',
  );
  expect(() => register({ controls: { warmth: { kind: "number" } } })).toThrow(/defaultProps has none/);
});

// each kind and the values that don't fit it, with what the error says
const misfits: [string, Record<string, unknown>, unknown, RegExp][] = [
  ["kind", { kind: "slider" }, 1, /controls.v.kind must be one of number, switch, choice, colour, text, got "slider"/],
  ["cost", { kind: "number", cost: "later" }, 1, /controls.v.cost must be one of live, reopen, resim/],
  ["number", { kind: "number" }, "1", /controls.v is a number, but defaultProps.v is "1"/],
  ["number range", { kind: "number", min: 0, max: 1 }, 2, /controls.v is 0 to 1, but defaultProps.v is 2/],
  ["min above max", { kind: "number", min: 2, max: 1 }, 1.5, /has min 2 above max 1/],
  ["step", { kind: "number", step: 0 }, 1, /controls.v.step must be above 0/],
  ["switch", { kind: "switch" }, 0, /controls.v is a switch, but defaultProps.v is 0/],
  ["choice options", { kind: "choice", options: [] }, "a", /controls.v.options must be a non-empty list/],
  ["choice twice", { kind: "choice", options: ["a", "a"] }, "a", /lists "a" twice/],
  ["choice value", { kind: "choice", options: ["a", "b"] }, "c", /controls.v is one of "a", "b", but defaultProps.v is "c"/],
  ["colour", { kind: "colour" }, "orange", /controls.v is a colour, but defaultProps.v is "orange"/],
  ["colour length", { kind: "colour" }, "#ff888", /is a colour/],
  ["text", { kind: "text" }, 3, /controls.v is text, but defaultProps.v is 3/],
  ["text length", { kind: "text", maxLength: 3 }, "four", /holds at most 3 characters, but defaultProps.v has 4/],
  ["label", { kind: "text", label: 3 }, "a", /controls.v.label must be a string/],
];
for (const [what, spec, value, why] of misfits) {
  test(`a control's value must fit its kind: ${what}`, () => {
    resetRegistry();
    expect(() => register({ defaultProps: { v: value }, controls: { v: spec } })).toThrow(why);
  });
}

test("colours take three, four, six or eight hex digits", () => {
  for (const tint of ["#f80", "#f80c", "#ff8800", "#FF8800cc"]) {
    resetRegistry();
    register({ defaultProps: { tint }, controls: { tint: { kind: "colour" } } });
    expect(getCompositions()[0].controls.tint.kind).toBe("colour");
  }
});

test("controls must be an object", () => {
  expect(() => register({ defaultProps: { v: 1 }, controls: [{ kind: "number" }] })).toThrow(/controls must be an object of controls by defaultProps key/);
});

test("useControl reads the value the frame is drawn with, inside a Sequence too", () => {
  const seen: unknown[] = [];
  function Deep() { seen.push(useControl("warmth")); return null; }
  const opts = { frame: 10, fps: 30, width: 640, height: 360, durationInFrames: 60 };
  renderFrame(<Sequence from={5}><Deep /></Sequence>, { ...opts, defaultProps: { warmth: 0.2 } });
  renderFrame(<Deep />, { ...opts, defaultProps: { warmth: 0.7 } });
  expect(seen).toEqual([0.2, 0.7]);
});

test("useControl names a prop the composition has", () => {
  function Wrong() { useControl("warmth"); return null; }
  expect(() => renderFrame(<Wrong />, { frame: 0, fps: 30, width: 64, height: 64, durationInFrames: 1, defaultProps: { hue: 1 } })).toThrow(
    'useControl("warmth"): the composition has no prop "warmth" (its defaultProps has hue)',
  );
});
