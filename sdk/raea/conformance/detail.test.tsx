// docs/realism.feature LT21: a part's `detail` is raea-sys-lod's Detail on it, the mesh keeping its shape.
import { describe, expect, test } from "bun:test";
import { Cuboid, Model } from "../src/3d";
import { Mesh3d, StandardMaterial } from "@raea/render/bevy3d";
import { renderFrame } from "../src/test";

const html = (el: React.ReactElement) => renderFrame(el, { frame: 0, width: 320, height: 240, fps: 30, durationInFrames: 30 });
const props = (out: string, kind: string) => [...out.matchAll(new RegExp(`data-3d="${kind}" data-props="([^"]*)"`, "g"))].map((m) => JSON.parse(m[1]!.replaceAll("&quot;", '"')));

describe("LT21: detail", () => {
  test("a Mesh3d's detail is a Detail component, every field given, and the mesh keeps its shape", () => {
    const out = html(<Mesh3d detail={{ far: 120, minPx: 4 }}><Cuboid /><StandardMaterial /></Mesh3d>);
    expect(out).toContain('data-shape="box(1, 1, 1)"');
    expect(props(out, "BevyComponent")).toEqual([{ type: "raea_sys_lod::Detail", value: { near: 0, far: 120, min_px: 4, max_px: 0, fade: 0.1, shadow: true, size: 0, edge: false } }]);
  });

  test("a Model's detail is a component inside it, and not a prop of its row", () => {
    const out = html(<Model src="a.glb" detail={{ near: 120, shadow: false, edge: true }} />);
    expect(props(out, "Model")[0]).not.toHaveProperty("detail");
    expect(props(out, "BevyComponent")[0].value).toMatchObject({ near: 120, shadow: false, edge: true });
  });

  test("a shape inside a fragment is the mesh's shape", () => {
    expect(html(<Mesh3d><><Cuboid /><StandardMaterial /></></Mesh3d>)).toContain('data-shape="box(1, 1, 1)"');
  });

  test("a detail that can't be is refused", () => {
    expect(() => html(<Mesh3d detail={{ near: 50, far: 20 }}><Cuboid /></Mesh3d>)).toThrow("must be past");
    expect(() => html(<Model src="a.glb" detail={{ fade: 2 }} />)).toThrow("0 to 1");
  });
});
