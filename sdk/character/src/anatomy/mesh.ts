// The shapes anatomy is made of, as triangle meshes in plain arrays (a <BufferMesh>'s positions, normals and indices):
// a tube along a path with an oval section (a rib, a long bone, a vessel, a blade of bone), a blob (an ellipsoid shaped
// by a function of direction: an organ), and a loft through closed rings (a lung that fills the rib cage).
export type V3 = [number, number, number];
export interface Geo { positions: number[]; normals: number[]; indices: number[] }

export const v = {
  add: (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]],
  sub: (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]],
  mul: (a: V3, s: number): V3 => [a[0] * s, a[1] * s, a[2] * s],
  dot: (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
  cross: (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  len: (a: V3) => Math.hypot(a[0], a[1], a[2]),
  norm: (a: V3): V3 => { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / l, a[1] / l, a[2] / l]; },
  lerp: (a: V3, b: V3, t: number): V3 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t],
};

/** each vertex's normal from the faces round it (area-weighted), outward for a mesh wound counter-clockwise */
export function withNormals(positions: number[], indices: number[]): Geo {
  const n = new Array(positions.length).fill(0);
  for (let i = 0; i < indices.length; i += 3) {
    const [a, b, c] = [indices[i], indices[i + 1], indices[i + 2]];
    const p = (k: number): V3 => [positions[3 * k], positions[3 * k + 1], positions[3 * k + 2]];
    const f = v.cross(v.sub(p(b), p(a)), v.sub(p(c), p(a)));
    for (const k of [a, b, c]) for (let d = 0; d < 3; d++) n[3 * k + d] += f[d];
  }
  for (let k = 0; k < n.length; k += 3) {
    const l = Math.hypot(n[k], n[k + 1], n[k + 2]) || 1;
    n[k] /= l; n[k + 1] /= l; n[k + 2] /= l;
  }
  return { positions, normals: n, indices };
}

export function merge(...gs: Geo[]): Geo {
  const out: Geo = { positions: [], normals: [], indices: [] };
  for (const g of gs) {
    const o = out.positions.length / 3;
    out.positions.push(...g.positions); out.normals.push(...g.normals);
    for (const i of g.indices) out.indices.push(i + o);
  }
  return out;
}

/** a path through points, smooth (Catmull-Rom), `n` points evenly in its parameter */
export function smoothPath(pts: V3[], n: number): V3[] {
  if (pts.length < 3) return Array.from({ length: n }, (_, i) => v.lerp(pts[0], pts[pts.length - 1], i / (n - 1)));
  const out: V3[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / (n - 1)) * (pts.length - 1), k = Math.min(pts.length - 2, Math.floor(t)), u = t - k;
    const p0 = pts[Math.max(0, k - 1)], p1 = pts[k], p2 = pts[k + 1], p3 = pts[Math.min(pts.length - 1, k + 2)];
    const c = (a: number, b: number, c_: number, d: number) => 0.5 * (2 * b + (-a + c_) * u + (2 * a - 5 * b + 4 * c_ - d) * u * u + (-a + 3 * b - 3 * c_ + d) * u * u * u);
    out.push([c(p0[0], p1[0], p2[0], p3[0]), c(p0[1], p1[1], p2[1], p3[1]), c(p0[2], p1[2], p2[2], p3[2])]);
  }
  return out;
}

/**
 * A tube along `path` with an oval section: `r(t)` gives its two half-widths at t (0 at the start, 1 at the end), the
 * first along `hint` (laid square to the path: up for a rib, so its section stands tall) and the second across it.
 * Its ends are closed.
 */
export function tube(path: V3[], r: (t: number) => [number, number], sides = 10, hint: V3 = [0, 1, 0]): Geo {
  const P: number[] = [], I: number[] = [], n = path.length;
  let prevN: V3 | null = null;
  for (let i = 0; i < n; i++) {
    const T = v.norm(v.sub(path[Math.min(n - 1, i + 1)], path[Math.max(0, i - 1)]));
    let N = v.sub(hint, v.mul(T, v.dot(hint, T)));
    if (v.len(N) < 1e-4) N = prevN ?? v.cross(T, Math.abs(T[0]) < 0.9 ? [1, 0, 0] : [0, 0, 1]);
    N = v.norm(N);
    if (prevN && v.dot(N, prevN) < 0) N = v.mul(N, -1);
    prevN = N;
    const B = v.cross(T, N), [ra, rb] = r(i / (n - 1));
    for (let s = 0; s < sides; s++) {
      const a = (2 * Math.PI * s) / sides;
      P.push(...v.add(path[i], v.add(v.mul(N, ra * Math.cos(a)), v.mul(B, rb * Math.sin(a)))));
    }
  }
  for (let i = 0; i < n - 1; i++)
    for (let s = 0; s < sides; s++) {
      const a = i * sides + s, b = i * sides + ((s + 1) % sides), c = a + sides, d = b + sides;
      I.push(a, b, c, b, d, c);
    }
  const body = withNormals(P, I);
  // the ends, each a fan of its own vertices (so the side's normals stay round)
  const cap = (i: number, out: number): Geo => {
    const ps: number[] = [...path[i], ...P.slice(i * sides * 3, (i + 1) * sides * 3)], ix: number[] = [];
    for (let s = 0; s < sides; s++) out > 0 ? ix.push(0, 1 + s, 1 + ((s + 1) % sides)) : ix.push(0, 1 + ((s + 1) % sides), 1 + s);
    return withNormals(ps, ix);
  };
  return outward(merge(body, cap(0, -1), cap(n - 1, 1)));
}

/**
 * A blob: an ellipsoid at `c` with axes `ax` (three directions) and half-lengths `rad`, each direction's radius
 * scaled by `shape(u)` (u the unit direction in the axes' own frame: u[2] along the third axis, the poles').
 */
export function blob(c: V3, ax: [V3, V3, V3], rad: V3, shape: (u: V3) => number = () => 1, nu = 28, nv = 18): Geo {
  const P: number[] = [], I: number[] = [];
  for (let j = 0; j <= nv; j++) {
    const th = (Math.PI * j) / nv;
    for (let i = 0; i < nu; i++) {
      const ph = (2 * Math.PI * i) / nu;
      const u: V3 = [Math.sin(th) * Math.cos(ph), Math.sin(th) * Math.sin(ph), Math.cos(th)];
      const s = shape(u);
      let p = c;
      for (let k = 0; k < 3; k++) p = v.add(p, v.mul(ax[k], rad[k] * u[k] * s));
      P.push(...p);
    }
  }
  for (let j = 0; j < nv; j++)
    for (let i = 0; i < nu; i++) {
      const a = j * nu + i, b = j * nu + ((i + 1) % nu), c_ = a + nu, d = b + nu;
      I.push(a, c_, b, b, c_, d);
    }
  return outward(withNormals(P, I));
}

/** a loft through closed rings of the same count (each ring in order round its loop), its two ends closed (each
 *  end's fan meeting at its ring's middle, or at the point given) */
export function loft(rings: V3[][], ends: [V3?, V3?] = [undefined, undefined]): Geo {
  const m = rings[0].length, P: number[] = [], I: number[] = [];
  for (const r of rings) for (const p of r) P.push(...p);
  for (let k = 0; k < rings.length - 1; k++)
    for (let s = 0; s < m; s++) {
      const a = k * m + s, b = k * m + ((s + 1) % m), c = a + m, d = b + m;
      I.push(a, c, b, b, c, d);
    }
  const side = withNormals(P, I);
  const cap = (k: number, out: number): Geo => {
    const r = rings[k], c = ends[k === 0 ? 0 : 1] ?? v.mul(r.reduce((s, p) => v.add(s, p), [0, 0, 0] as V3), 1 / m), ps: number[] = [...c], ix: number[] = [];
    for (const p of r) ps.push(...p);
    for (let s = 0; s < m; s++) out > 0 ? ix.push(0, 1 + ((s + 1) % m), 1 + s) : ix.push(0, 1 + s, 1 + ((s + 1) % m));
    return withNormals(ps, ix);
  };
  return outward(merge(side, cap(0, -1), cap(rings.length - 1, 1)));
}

/** a closed mesh turned to face out, if its faces were wound in (its signed volume negative) */
export function outward(g: Geo): Geo {
  let vol = 0;
  const P = g.positions, I = g.indices;
  for (let i = 0; i < I.length; i += 3) {
    const a = 3 * I[i], b = 3 * I[i + 1], c = 3 * I[i + 2];
    vol += P[a] * (P[b + 1] * P[c + 2] - P[b + 2] * P[c + 1]) - P[a + 1] * (P[b] * P[c + 2] - P[b + 2] * P[c]) + P[a + 2] * (P[b] * P[c + 1] - P[b + 1] * P[c]);
  }
  return vol < 0 ? flip(g) : g;
}

/** its faces turned the other way (and its normals) */
export function flip(g: Geo): Geo {
  for (let i = 0; i < g.indices.length; i += 3) [g.indices[i + 1], g.indices[i + 2]] = [g.indices[i + 2], g.indices[i + 1]];
  for (let k = 0; k < g.normals.length; k++) g.normals[k] = -g.normals[k];
  return g;
}

/** the mesh moved by -o (into a joint's frame at rest, its head at o) */
export function shifted(g: Geo, o: V3): Geo {
  const P = g.positions.slice();
  for (let k = 0; k < P.length; k += 3) { P[k] -= o[0]; P[k + 1] -= o[1]; P[k + 2] -= o[2]; }
  return { positions: P, normals: g.normals, indices: g.indices };
}
