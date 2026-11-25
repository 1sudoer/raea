// The WGSL text a TSL node material compiles to, read and rewritten (tsl.ts, docs/r3f.feature R3F21): its structs,
// entry parameters and texture samples, words and indexed reads renamed to the engine's contract (systems/three-shader),
// and the contract's own vertex attributes and stored types.

/** a refusal: what the graph needs that the engine's contract doesn't carry */
export class Unsupported extends Error {}

/** the struct member types of every `struct NAME { ... }` in a source */
export function structs(src: string): Map<string, Map<string, string>> {
  const out = new Map<string, Map<string, string>>();
  for (const s of src.matchAll(/struct\s+(\w+)\s*\{([^}]*)\}/g)) {
    const fields = new Map<string, string>();
    for (const f of s[2].split(",")) {
      const m = f.replace(/@\w+\s*\([^)]*\)/g, "").trim().match(/^(\w+)\s*:\s*(.+)$/);
      if (m) fields.set(m[1], m[2].trim());
    }
    out.set(s[1], fields);
  }
  return out;
}

/** `name` as a whole word in `src`, replaced (WGSL words: letters, digits, underscore) */
export function word(src: string, name: string, to: string): string {
  return src.replace(new RegExp(`(?<![A-Za-z0-9_])${name}(?![A-Za-z0-9_])`, "g"), to);
}

/** each call of `fn` whose first argument is one of `textures`, its coordinate (third argument) flipped in v */
export function flipSamples(src: string, flipped: Set<string>): string {
  if (!flipped.size) return src;
  let out = "";
  let i = 0;
  const re = /\b(textureSample\w*)\s*\(/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    const open = m.index + m[0].length;
    // the call's arguments, split at its own top-level commas
    const args: [number, number][] = [];
    let depth = 0, start = open, j = open;
    for (; j < src.length; j++) {
      const ch = src[j];
      if (ch === "(" || ch === "[") depth++;
      else if (ch === ")" || ch === "]") {
        if (depth === 0) break;
        depth--;
      } else if (ch === "," && depth === 0) {
        args.push([start, j]);
        start = j + 1;
      }
    }
    args.push([start, j]);
    const tex = src.slice(args[0][0], args[0][1]).trim();
    if (args.length < 3 || !flipped.has(tex)) continue;
    const [a, z] = args[2];
    out += src.slice(i, a) + ` tsl_flip_v( ${src.slice(a, z).trim()} )`;
    i = z;
  }
  return out + src.slice(i);
}

export const FLIP = "fn tsl_flip_v(c: vec2<f32>) -> vec2<f32> { return vec2<f32>(c.x, 1.0 - c.y); }\n";

/** the contract's vertex attributes (ThreeVertex), by three's attribute name */
export const ATTRIBUTES: Record<string, { field: string; type: string }> = {
  position: { field: "position", type: "vec3<f32>" },
  normal: { field: "normal", type: "vec3<f32>" },
  uv: { field: "uv", type: "vec2<f32>" },
  color: { field: "color", type: "vec4<f32>" },
};

/** a value of `type` read as `field` (a vec4 colour as a vec3 three declared, say) */
export function fit(field: string, from: string, to: string): string {
  if (from === to) return field;
  const n = (t: string) => Number(t.match(/^vec(\d)/)?.[1] ?? 1);
  if (to.startsWith("vec") && from.startsWith("vec") && n(to) < n(from)) return `${field}.${"xyzw".slice(0, n(to))}`;
  if (from.startsWith("vec") && (to === "f32" || to === "i32" || to === "u32")) return to === "f32" ? `${field}.x` : `${to}(${field}.x)`;
  throw new Unsupported(`an attribute declared ${to} where raea gives ${from}`);
}

/** a uniform's slot count in vec4s */
export const slotsOf = (type: string, array = 1) => (type.startsWith("mat4") ? 4 : type.startsWith("mat3") ? 3 : 1) * array;

/** a uniformArray's element type (three's name) as WGSL's */
export const ELEMENT: Record<string, string> = { float: "f32", vec2: "vec2<f32>", vec3: "vec3<f32>", vec4: "vec4<f32>", color: "vec3<f32>" };
/** the mesh's own attributes the engine carries (raea-sys-three-shader's ATTRIBUTES) */
export const ATTRIBUTES_OWN = 4;

export const STORED: Record<string, string> = {
  f32: "f32", i32: "i32", u32: "u32", bool: "u32",
  "vec2<f32>": "vec2<f32>", "vec3<f32>": "vec3<f32>", "vec4<f32>": "vec4<f32>",
  "mat3x3<f32>": "mat3x3<f32>", "mat4x4<f32>": "mat4x4<f32>",
};

/** each `name[ EXPR ]` in `src` (EXPR to its own closing bracket) as `to(EXPR)` */
export function indexed(src: string, name: string, to: (i: string) => string): string {
  let out = "";
  let i = 0;
  const re = new RegExp(`(?<![A-Za-z0-9_])${name.replace(/[.]/g, "\\.")}\\s*\\[`, "g");
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    let depth = 0, j = m.index + m[0].length;
    for (; j < src.length; j++) {
      if (src[j] === "[" || src[j] === "(") depth++;
      else if (src[j] === "]" || src[j] === ")") {
        if (depth === 0) break;
        depth--;
      }
    }
    out += src.slice(i, m.index) + to(src.slice(m.index + m[0].length, j).trim());
    i = j + 1;
    re.lastIndex = i;
  }
  return out + src.slice(i);
}

/** an entry point's parameters: `@location( 0 ) uv : vec2<f32>`, `@builtin( front_facing ) isFront : bool` */
export function params(list: string): { name: string; type: string; location?: number; builtin?: string }[] {
  return list
    .split(/,(?=\s*@)/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => {
      const m = p.match(/^((?:@\w+\s*\([^)]*\)\s*)+)(\w+)\s*:\s*(.+)$/);
      if (!m) throw new Unsupported(`an entry parameter three wrote as ${JSON.stringify(p)}`);
      const loc = m[1].match(/@location\(\s*(\d+)\s*\)/);
      const bi = m[1].match(/@builtin\(\s*(\w+)\s*\)/);
      return { name: m[2], type: m[3].trim(), location: loc ? Number(loc[1]) : undefined, builtin: bi?.[1] };
    });
}
