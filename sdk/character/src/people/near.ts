// Places by where they are: a grid of cells, so the nearest few of a kind, or all within a reach, are found by looking
// at the cells around a point, not at every place in the city (16,000 shops in a slice of Berlin; CH52).
import type { Place, V3 } from "./types";

const dist = (a: V3, b: V3) => Math.hypot(a[0] - b[0], a[2] - b[2]);

export class Grid {
  private cells = new Map<string, Place[]>();
  constructor(readonly places: Place[], readonly cell = 250) {
    for (const p of places) {
      const k = this.key(Math.floor(p.at[0] / cell), Math.floor(p.at[2] / cell));
      const c = this.cells.get(k);
      if (c) c.push(p); else this.cells.set(k, [p]);
    }
  }
  private key(i: number, j: number) { return `${i},${j}`; }
  /** the cells in the square ring `r` cells out from (i, j) */
  private ring(i: number, j: number, r: number, out: Place[]) {
    for (let a = -r; a <= r; a++) for (let b = -r; b <= r; b++) {
      if (Math.max(Math.abs(a), Math.abs(b)) !== r) continue;
      const c = this.cells.get(this.key(i + a, j + b));
      if (c) for (const p of c) out.push(p);
    }
  }
  /** the `k` nearest to `at`, nearest first (fewer when there are fewer) */
  nearest(at: V3, k: number): Place[] {
    if (this.places.length <= k) return [...this.places].sort((a, b) => dist(a.at, at) - dist(b.at, at));
    const i = Math.floor(at[0] / this.cell), j = Math.floor(at[2] / this.cell);
    const seen: Place[] = [];
    // a ring r cells out holds nothing nearer than (r - 1) cells; stop once k are found nearer than that
    for (let r = 0; ; r++) {
      this.ring(i, j, r, seen);
      if (seen.length >= k) {
        seen.sort((a, b) => dist(a.at, at) - dist(b.at, at));
        if (dist(seen[k - 1].at, at) <= r * this.cell) return seen.slice(0, k);
      }
      if (seen.length === this.places.length) return seen.sort((a, b) => dist(a.at, at) - dist(b.at, at)).slice(0, k);
    }
  }
  /** every place within `metres` of `at` */
  within(at: V3, metres: number): Place[] {
    const i = Math.floor(at[0] / this.cell), j = Math.floor(at[2] / this.cell), R = Math.ceil(metres / this.cell);
    const out: Place[] = [];
    for (let r = 0; r <= R; r++) this.ring(i, j, r, out);
    return out.filter((p) => dist(p.at, at) <= metres);
  }
}

const grids = new WeakMap<Place[], Grid>();
/** The grid over a list of places, made once per list. */
export function gridOf(places: Place[]): Grid {
  let g = grids.get(places);
  if (!g) grids.set(places, (g = new Grid(places)));
  return g;
}
