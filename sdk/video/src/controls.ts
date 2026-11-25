// A composition's controls (docs/realtime.feature RT4): the defaultProps a panel tunes, each one's kind and how a
// change to it reaches the picture, and the check a registered composition's controls pass (registry.tsx).

/** How a change to a control reaches the picture. `live` (the default): the running stage draws it at once, as a
 *  prop. `reopen`: the stage opens the file again first, for what a running 3D world keeps from its first frame (a
 *  shape, a model's file, an instance count, a picture map, a shader). `resim`: everything from the frame it changes
 *  on is worked out again, for state carried from frame to frame (physics, particles, a game's fold). */
export type ControlCost = "live" | "reopen" | "resim";

interface ControlBase {
  /** what the panel calls it (the key when left out) */
  label?: string;
  /** the panel's group it sits in ("Grade", "Camera"); ungrouped controls come first */
  group?: string;
  /** how a change reaches the picture: "live" unless said */
  cost?: ControlCost;
  /** one line on what it changes, for the panel's tooltip and an agent reading the code */
  about?: string;
}

/** A number, dragged on a slider when it has both ends. */
export interface NumberControl extends ControlBase {
  /** what it is: a number */
  kind: "number";
  /** the least it may be */
  min?: number;
  /** the most it may be */
  max?: number;
  /** what one step of the slider or an arrow key moves it by (above 0) */
  step?: number;
  /** its unit, shown after it ("px", "°", "s") */
  unit?: string;
}

/** On or off: a boolean. */
export interface SwitchControl extends ControlBase {
  /** what it is: on or off */
  kind: "switch";
}

/** One of a list: a string or a number from `options`. */
export interface ChoiceControl extends ControlBase {
  /** what it is: one of a list */
  kind: "choice";
  /** what it may be, in the order the panel lists them (at least one, none twice) */
  options: readonly (string | number)[];
}

/** A colour: a hex string, "#rgb", "#rgba", "#rrggbb" or "#rrggbbaa". */
export interface ColourControl extends ControlBase {
  /** what it is: a colour */
  kind: "colour";
}

/** A string: a title, a caption, a name. */
export interface TextControl extends ControlBase {
  /** what it is: a string */
  kind: "text";
  /** a box of several lines rather than one */
  multiline?: boolean;
  /** the most characters it may hold */
  maxLength?: number;
}

export type Control = NumberControl | SwitchControl | ChoiceControl | ColourControl | TextControl;

/** A composition's controls, by the defaultProps key each one tunes. */
export type Controls<P = Record<string, unknown>> = { readonly [K in keyof P & string]?: Control };

/** A control as a registered composition holds it: its cost said. */
export type RegisteredControl = Control & { cost: ControlCost };

const KINDS: readonly Control["kind"][] = ["number", "switch", "choice", "colour", "text"];
const COSTS: readonly ControlCost[] = ["live", "reopen", "resim"];
const HEX = /^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

/** The controls checked against the defaultProps they tune: each names one of its keys, says a kind and a cost
 *  raea knows, and its value fits that kind. `fail(field, why)` throws. */
export function validControls(given: unknown, props: Record<string, unknown>, fail: (field: string, why: string) => never): Record<string, RegisteredControl> {
  if (given === undefined) return {};
  if (given === null || typeof given !== "object" || Array.isArray(given) || Object.getPrototypeOf(given) !== Object.prototype) {
    fail("controls", "must be an object of controls by defaultProps key");
  }
  const out: Record<string, RegisteredControl> = {};
  for (const [key, c] of Object.entries(given as Record<string, unknown>)) {
    const at = `controls.${key}`;
    if (!Object.prototype.hasOwnProperty.call(props, key)) {
      const keys = Object.keys(props);
      fail(at, `names no defaultProps key: a control tunes a prop the composition has a value for (defaultProps has ${keys.length ? keys.join(", ") : "none"})`);
    }
    if (c === null || typeof c !== "object" || Array.isArray(c)) fail(at, `must be an object with a kind (${KINDS.join(", ")})`);
    const spec = c as Record<string, unknown>;
    if (!KINDS.includes(spec.kind as Control["kind"])) fail(`${at}.kind`, `must be one of ${KINDS.join(", ")}, got ${JSON.stringify(spec.kind)}`);
    if (spec.cost !== undefined && !COSTS.includes(spec.cost as ControlCost)) fail(`${at}.cost`, `must be one of ${COSTS.join(", ")}, got ${JSON.stringify(spec.cost)}`);
    for (const f of ["label", "group", "about", "unit"] as const) {
      if (spec[f] !== undefined && typeof spec[f] !== "string") fail(`${at}.${f}`, `must be a string, got ${JSON.stringify(spec[f])}`);
    }
    const v = props[key];
    const got = JSON.stringify(v);
    switch (spec.kind as Control["kind"]) {
      case "number": {
        for (const f of ["min", "max", "step"] as const) {
          if (spec[f] !== undefined && !(typeof spec[f] === "number" && Number.isFinite(spec[f]))) fail(`${at}.${f}`, `must be a finite number, got ${JSON.stringify(spec[f])}`);
        }
        const { min, max, step } = spec as { min?: number; max?: number; step?: number };
        if (min !== undefined && max !== undefined && min > max) fail(at, `has min ${min} above max ${max}`);
        if (step !== undefined && !(step > 0)) fail(`${at}.step`, `must be above 0, got ${step}`);
        if (typeof v !== "number" || !Number.isFinite(v)) fail(at, `is a number, but defaultProps.${key} is ${got}`);
        if ((min !== undefined && (v as number) < min) || (max !== undefined && (v as number) > max)) fail(at, `is ${min ?? "-∞"} to ${max ?? "∞"}, but defaultProps.${key} is ${got}`);
        break;
      }
      case "switch":
        if (typeof v !== "boolean") fail(at, `is a switch, but defaultProps.${key} is ${got} (it takes true or false)`);
        break;
      case "choice": {
        const options = spec.options;
        if (!Array.isArray(options) || options.length === 0 || !options.every((o) => typeof o === "string" || (typeof o === "number" && Number.isFinite(o)))) {
          fail(`${at}.options`, "must be a non-empty list of strings or numbers");
        }
        const opts = options as (string | number)[];
        const twice = opts.find((o, i) => opts.indexOf(o) !== i);
        if (twice !== undefined) fail(`${at}.options`, `lists ${JSON.stringify(twice)} twice`);
        if (!opts.includes(v as string | number)) fail(at, `is one of ${opts.map((o) => JSON.stringify(o)).join(", ")}, but defaultProps.${key} is ${got}`);
        break;
      }
      case "colour":
        if (typeof v !== "string" || !HEX.test(v)) fail(at, `is a colour, but defaultProps.${key} is ${got} (it takes "#rgb", "#rgba", "#rrggbb" or "#rrggbbaa")`);
        break;
      case "text": {
        const { maxLength } = spec as { maxLength?: unknown };
        if (maxLength !== undefined && !(Number.isInteger(maxLength) && (maxLength as number) > 0)) fail(`${at}.maxLength`, `must be a positive integer, got ${JSON.stringify(maxLength)}`);
        if (spec.multiline !== undefined && typeof spec.multiline !== "boolean") fail(`${at}.multiline`, `must be true or false, got ${JSON.stringify(spec.multiline)}`);
        if (typeof v !== "string") fail(at, `is text, but defaultProps.${key} is ${got}`);
        if (maxLength !== undefined && (v as string).length > (maxLength as number)) fail(at, `holds at most ${maxLength} characters, but defaultProps.${key} has ${(v as string).length}`);
        break;
      }
    }
    out[key] = { ...(spec as unknown as Control), cost: (spec.cost as ControlCost | undefined) ?? "live" };
  }
  return out;
}
