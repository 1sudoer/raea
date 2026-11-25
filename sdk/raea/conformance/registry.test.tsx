// registerRoot, from spec/motion.feature ("Declaring and registering"), including a root with no Composition.
import { beforeEach, expect, test } from "bun:test";
import { Composition, Sequence, getCompositions, registerRoot, useCurrentFrame, useVideoConfig } from "../src";
import { renderFrame, resetRegistry } from "../src/test";

beforeEach(() => resetRegistry());
function Uses() { useCurrentFrame(); return null; }

test("compositions are found in the root", () => {
  registerRoot(() => (<><Composition id="ad-1" component={Uses} width={1080} height={1350} fps={30} durationInFrames={90} /></>));
  const c = getCompositions();
  expect(c.length).toBe(1);
  expect(c[0]).toMatchObject({ id: "ad-1", width: 1080, height: 1350, fps: 30, durationInFrames: 90 });
});

test("a bad id fails", () => {
  expect(() => registerRoot(() => <Composition id="ad 1" component={Uses} width={1080} height={1920} fps={30} durationInFrames={90} />)).toThrow();
});

test("registering twice fails", () => {
  registerRoot(() => <Composition id="a" component={Uses} width={1080} height={1920} fps={30} durationInFrames={9} />);
  expect(() => registerRoot(() => null)).toThrow();
});

test("a root with no Composition is itself one, sized from its Sequences", () => {
  registerRoot(() => (
    <>
      <Sequence from={0} durationInFrames={120}><Uses /></Sequence>
      <Sequence from={100} durationInFrames={80}><Uses /></Sequence>
    </>
  ));
  const c = getCompositions();
  expect(c.length).toBe(1);
  expect(c[0]).toMatchObject({ id: "main", fps: 30, width: 1080, height: 1920, durationInFrames: 180 });
});

test("without finite Sequences the length is 300", () => {
  registerRoot(() => <Uses />);
  expect(getCompositions()[0]).toMatchObject({ id: "main", durationInFrames: 300 });
});

test("an edit plays its segments one after another, and the length is their sum", () => {
  const seen: number[] = [];
  function Reads() { seen.push(useCurrentFrame()); return null; }
  registerRoot(() => <Composition id="cut" component={Reads} width={1920} height={1080} fps={30} durationInFrames={90} edit={{ segments: [[0, 15], [45, 90]] }} />);
  const c = getCompositions()[0];
  expect(c.durationInFrames).toBe(60);
  expect(c.component.name).toBe("Edited");
});

test("an edit's segment must sit inside the composition", () => {
  expect(() => registerRoot(() => <Composition id="cut" component={Uses} width={1920} height={1080} fps={30} durationInFrames={90} edit={{ segments: [[60, 120]] }} />)).toThrow(/edit.segments/);
});

test("an edit's scale draws the composition smaller, laid out at its own size", () => {
  let cfg: any;
  function Reads() { cfg = useVideoConfig(); return <div>drawn</div>; }
  registerRoot(() => <Composition id="draft" component={Reads} width={1920} height={1080} fps={30} durationInFrames={90} edit={{ segments: [[0, 30]], scale: 0.5 }} />);
  const c = getCompositions()[0];
  expect(c).toMatchObject({ width: 960, height: 540, durationInFrames: 30 });
  const html = renderFrame(<c.component />, { frame: 0, fps: 30, width: c.width, height: c.height, durationInFrames: c.durationInFrames });
  // inside, the composition sees the size it says, in a box that size drawn at half
  expect(cfg).toMatchObject({ width: 1920, height: 1080 });
  expect(html).toContain("width:1920px");
  expect(html).toContain("transform:scale(0.5, 0.5)");
  expect(html).toContain("drawn");
});

test("an edit's scale of 1 changes nothing, and one outside (0, 1] fails", () => {
  registerRoot(() => <Composition id="same" component={Uses} width={1080} height={1920} fps={30} durationInFrames={9} edit={{ scale: 1 }} />);
  expect(getCompositions()[0]).toMatchObject({ width: 1080, height: 1920 });
  expect(getCompositions()[0].component).toBe(Uses);
  for (const scale of [0, 1.5, Number.NaN]) {
    resetRegistry();
    expect(() => registerRoot(() => <Composition id="bad" component={Uses} width={1080} height={1920} fps={30} durationInFrames={9} edit={{ scale }} />)).toThrow(/edit.scale/);
  }
});

// A hook inside a component written inline (component={() => ...}) runs when React renders that component, not when
// the root is called, so the root is still looked inside: before 11-17 its source's hook made the whole root content,
// and every such program registered one composition, "main".
test("a hook in an inline component doesn't hide the root's compositions", () => {
  registerRoot(() => (
    <>
      <Composition id="block" component={() => { const f = useCurrentFrame(); return <>{f}</>; }} width={1080} height={1920} fps={30} durationInFrames={60} />
      <Composition id="expr" component={() => <>{useVideoConfig().width}</>} width={1920} height={1080} fps={30} durationInFrames={90} />
      <Composition id="fn" component={function Inline() { useCurrentFrame(); return null; }} width={720} height={720} fps={24} durationInFrames={48} />
    </>
  ));
  expect(getCompositions().map((c) => c.id)).toEqual(["block", "expr", "fn"]);
});

test("a root that calls a hook itself is still content", () => {
  registerRoot(() => { useCurrentFrame(); return <Composition id="never" component={Uses} width={1080} height={1920} fps={30} durationInFrames={60} />; });
  expect(getCompositions().map((c) => c.id)).toEqual(["main"]);
});

test("a component in the tree with an inline render prop is expanded, and its own hook still stops the walk", () => {
  function Shell({ children }: { children?: unknown }) { return <>{children}</>; }
  function Clocked() { useCurrentFrame(); return <Composition id="hidden" component={Uses} width={1080} height={1920} fps={30} durationInFrames={60} />; }
  registerRoot(() => (
    <Shell>
      <Composition id="shown" component={(p: { n?: number }) => <>{useCurrentFrame() + (p.n ?? 0)}</>} width={1080} height={1920} fps={30} durationInFrames={30} />
      <Clocked />
    </Shell>
  ));
  expect(getCompositions().map((c) => c.id)).toEqual(["shown"]);
});

test("braces and arrows in strings, a regex, a comment and a template don't throw the reading off", () => {
  registerRoot(() => {
    const label = "a { b => c ) ]";
    const tick = `${label.length} } =>`;
    const half = label.length / 2; // } => (
    const braces = /[{}()]=>/g.test(label + tick + half);
    return (
      <>
        <Composition id="one" component={() => { const f = useCurrentFrame(); return <>{f + (braces ? 1 : 0)}</>; }} width={1080} height={1920} fps={30} durationInFrames={60} />
        <Composition id="two" component={() => <>{`${useVideoConfig().fps} }`}</>} width={1080} height={1920} fps={30} durationInFrames={60} />
      </>
    );
  });
  expect(getCompositions().map((c) => c.id)).toEqual(["one", "two"]);
});
