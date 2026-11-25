// Scrolling on the stand-in host (docs/ts-native.feature P18, src/dom): a scroll the page writes goes to the host and
// shows at once, and the host's scroll event, which comes a frame late, never puts back an offset the page wrote past.
import { beforeAll, describe, expect, test } from "bun:test";
import { install } from "../src/dom";
import { mount } from "../src/react";
import { fresh, snap } from "./host";

beforeAll(() => install());
const g = globalThis as any;

describe("P18 DOM surface: scrolling", () => {
  test("geometry from the snapshot; scroll writes to the host", async () => {
    const host = fresh();
    let el: any;
    mount(
      <div data-testid="root">
        <div data-testid="box" className="overflow-y-auto" ref={(e: any) => (el = e)}>
          <div data-testid="inner" />
        </div>
      </div>,
      0,
      { dom: true },
    );
    snap(host, { root: [0, 0, 800, 600], box: [10, 20, 200, 100, 0, 0, 200, 500], inner: [10, 20, 200, 500] });
    const r = el.getBoundingClientRect();
    expect([r.x, r.y, r.width, r.height, r.right, r.bottom]).toEqual([10, 20, 200, 100, 210, 120]);
    expect(el.clientHeight).toBe(100);
    expect(el.scrollHeight).toBe(500);
    expect(g.innerWidth).toBe(800);
    expect(g.devicePixelRatio).toBe(2);
    const from = host.batches.length;
    el.scrollTop = 1000;
    expect(el.scrollTop).toBe(400);
    await Promise.resolve();
    const ops = host.since(from);
    expect(ops.some((o: any) => o.op === "update" && o.set?.scrollTop === 400)).toBe(true);
  });

  test("a scroll event carrying an offset older than one the page wrote since leaves the newer one; the host's own stands", () => {
    const host = fresh();
    let el: any;
    const heard: number[] = [];
    mount(
      <div data-testid="root">
        <div data-testid="box" className="overflow-y-auto" ref={(e: any) => (el = e)} onScroll={(e: any) => heard.push(e.currentTarget.scrollTop)}>
          <div data-testid="inner" />
        </div>
      </div>,
      0,
      { dom: true },
    );
    snap(host, { root: [0, 0, 800, 600], box: [10, 20, 200, 100, 0, 0, 200, 500], inner: [10, 20, 200, 500] });
    el.scrollTop = 50;
    el.scrollTop = 120;
    // the host's event for the first write, after the second: the page's 120 stands, and its handler reads 120
    host.send("box", "scroll", { scrollTop: 50, scrollLeft: 0, scrollHeight: 500, scrollWidth: 200, clientHeight: 100, clientWidth: 200 });
    expect(el.scrollTop).toBe(120);
    host.send("box", "scroll", { scrollTop: 120, scrollLeft: 0, scrollHeight: 500, scrollWidth: 200, clientHeight: 100, clientWidth: 200 });
    expect(el.scrollTop).toBe(120);
    // a wheel the page never wrote: the host's offset stands
    host.send("box", "scroll", { scrollTop: 300, scrollLeft: 0, scrollHeight: 500, scrollWidth: 200, clientHeight: 100, clientWidth: 200 });
    expect(el.scrollTop).toBe(300);
    expect(heard).toEqual([120, 120, 300]);
  });

  test("an offset written reads back as the host will show it, to a hundredth, so steps smaller than that add up and arrive", () => {
    // (raea's window: the host's geometry each frame has every number to a hundredth, systems/page snapshot.rs. Read
    // back unrounded, a write of 74.9931 looked like a move, and a spring that keeps what hasn't moved yet, as
    // use-stick-to-bottom's does, threw that away; the frame then put 74.99 back. Its steps under 0.005 never
    // arrived, and it wrote every frame for good: a chat's last turn never held still)
    const host = fresh();
    let el: any;
    mount(
      <div data-testid="root">
        <div data-testid="box" className="overflow-y-auto" ref={(e: any) => (el = e)}>
          <div data-testid="inner" />
        </div>
      </div>,
      0,
      { dom: true },
    );
    const frame = () => snap(host, { root: [0, 0, 800, 600], box: [0, 0, 200, 100, 0, Math.round(el.scrollTop * 100) / 100, 200, 175], inner: [0, -el.scrollTop, 200, 175] });
    snap(host, { root: [0, 0, 800, 600], box: [0, 0, 200, 100, 0, 74.99, 200, 175], inner: [0, -74.99, 200, 175] });
    el.scrollTop = 74.9931;
    expect(el.scrollTop).toBe(74.99);
    // the spring: a step under a hundredth each frame, kept until the offset moves
    let kept = 0;
    let frames = 0;
    while (el.scrollTop < 75 && frames++ < 60) {
      const was = el.scrollTop;
      kept += 0.002;
      el.scrollTop = was + kept;
      if (el.scrollTop !== was) kept = 0;
      frame();
    }
    expect(el.scrollTop).toBe(75);
    expect(frames).toBeLessThan(10);
  });

  test("two scrollers kept in step both ways settle, though the host's scroll events come a frame late", async () => {
    // (a timeline's tracks and heads: they went round for good in raea's window, a drag's offsets coming back in turn)
    const host = fresh();
    let a: any, b: any;
    // each mirrors the other, as a timeline's tracks and heads did
    mount(
      <div data-testid="root">
        <div data-testid="a" className="overflow-y-auto" ref={(e: any) => (a = e)} onScroll={() => b.scrollTop !== a.scrollTop && (b.scrollTop = a.scrollTop)}>
          <div data-testid="ai" />
        </div>
        <div data-testid="b" className="overflow-hidden" ref={(e: any) => (b = e)} onScroll={() => a.scrollTop !== b.scrollTop && (a.scrollTop = b.scrollTop)}>
          <div data-testid="bi" />
        </div>
      </div>,
      0,
      { dom: true },
    );
    snap(host, { root: [0, 0, 800, 600], a: [0, 0, 200, 100, 0, 0, 200, 800], ai: [0, 0, 200, 800], b: [200, 0, 100, 100, 0, 0, 100, 800], bi: [200, 0, 100, 800] });
    // a frame: what the page wrote is committed, and each scroller it moved gets its scroll event a frame later, with
    // the offset the commit left it at (runtime/dom commit.rs)
    let from = host.batches.length;
    let late: [number, number][] = [];
    const frame = async () => {
      await Promise.resolve();
      const moved = new Map<number, number>();
      for (const o of host.since(from) as any[]) if (o.op === "update" && typeof o.set?.scrollTop === "number") moved.set(o.id, o.set.scrollTop);
      from = host.batches.length;
      const due = late;
      late = [...moved];
      for (const [id, top] of due) host.send(id, "scroll", { scrollTop: top, scrollLeft: 0, scrollHeight: 800, scrollWidth: 200, clientHeight: 100, clientWidth: 200 });
    };
    // a drag in five quick steps, a frame each, then the frames after it
    for (const top of [50, 100, 150, 200, 250]) {
      a.scrollTop = top;
      await frame();
    }
    // (as in Chrome, whose scroll events also come a frame late to handlers that read the offsets as they are now,
    // a sync both ways can lose a drag's last steps; it settles, where an older offset taken over kept it going round)
    let f = 0;
    while (late.length && f++ < 20) await frame();
    expect(late).toEqual([]);
    expect(a.scrollTop).toBe(b.scrollTop);
  });
});
