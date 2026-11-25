// `raea show --on ITEM --working ID`: what a session sends the raea app's host for the person's comments (docs/review.feature
// R3), and where a note read from one stands (docs/direction.feature DR5, DR6): its asks, its badge and each ask's,
// and a clip under an ask for the person to sign. The host (apps/app/host/tasks.ts) is faked: this is the command's side.
import { describe, expect, test } from "bun:test";
import { labelsIn, showVerb } from "../src/show.ts";

/** showVerb with a host that keeps each op it was asked, and answers a task as the host does. */
function run(args: string[]) {
  const ops: [string, Record<string, unknown>][] = [];
  const ask = (op: string, b: Record<string, unknown>) => {
    ops.push([op, b]);
    if (op === "harness") return { name: "Claude Code", pid: 7 };
    return { item: b.item, project: "shop", comments: b.comments, state: b.state };
  };
  const out = showVerb(ask, args, "/work/shop", { CLAUDECODE: "1", CLAUDE_CODE_SESSION_ID: "abcdef123" });
  return { out, tasks: ops.filter(([op]) => op === "task").map(([, b]) => b) };
}

const HASH = "a".repeat(64);

describe("raea show --working, for a note", () => {
  test("a note to restate goes with its badge and no asks", () => {
    const { out, tasks } = run(["--on", "i-show-1", "--working", "cm-1", "--status", "to restate"]);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]).toMatchObject({ item: "i-show-1", comments: ["cm-1"], state: "working", status: "to restate" });
    expect(tasks[0].asks).toBeUndefined();
    expect(out).toBe(`Marked cm-1 working on i-show-1: the person sees Claude Code on them; it reads "to restate".\n`);
  });

  test("its asks go with the note's badge and each ask's", () => {
    const { out, tasks } = run(["--on", "i-show-1", "--working", "cm-1", "--ask", "the lyric is flat type", "--ask", "it arrives as a lyric video's", "--status", "to check", "--states", "to check, failing"]);
    expect(tasks[0]).toMatchObject({ asks: ["the lyric is flat type", "it arrives as a lyric video's"], status: "to check", states: ["to check", "failing"] });
    expect(out).toContain(`it reads "to check" (its asks: to check, failing).`);
  });

  test("a clip to sign goes under its ask, its path made whole", () => {
    const { out, tasks } = run(["--on", "i-show-1", "--working", "cm-1", "--sign", "2", "--clip", ".raea/notes-clips/n1.2.mp4", "--render", HASH]);
    expect(tasks[0]).toMatchObject({ state: "working", sign: { ask: 2, clip: "/work/shop/.raea/notes-clips/n1.2.mp4", render: HASH } });
    expect(out).toBe(`Shown ask 2's clip on cm-1 of i-show-1 for the person to sign (render aaaaaaaaaaaa…): their sign comes to the inbox, and raea notes take keeps it.\n`);
  });

  test("a mark without these sends none of them, as before", () => {
    const { tasks } = run(["--on", "i-show-1", "--working", "cm-1", "--done", "cm-2"]);
    expect(tasks.map((t) => [t.state, t.status, t.states, t.sign])).toEqual([["working", undefined, undefined, undefined], ["done", undefined, undefined, undefined]]);
  });

  test("what doesn't fit is said before anything reaches the app", () => {
    const refused = (args: string[], says: string) => expect(() => run(["--on", "i-show-1", ...args])).toThrow(says);
    refused(["--done", "cm-1", "--status", "passing"], "--status, --states and --sign go with --working");
    refused(["--working", "cm-1", "--working", "cm-2", "--sign", "1", "--clip", "/c.mp4", "--render", HASH], "--sign N shows one comment's clip");
    refused(["--working", "cm-1", "--sign", "1", "--clip", "/c.mp4"], "--sign N shows one comment's clip");
    refused(["--working", "cm-1", "--sign", "1", "--clip", "/c.mp4", "--render", HASH, "--done", "cm-2"], "--sign N shows one comment's clip");
    refused(["--working", "cm-1", "--clip", "/c.mp4"], "--clip and --render go with --sign N");
    refused(["--working", "cm-1", "--ask", "one", "--states", "to check,failing"], "one state for each --ask: 1 asks, 2 states");
  });
});

// labels on what's shown (docs/callouts.feature CO2b): what the command sends the host, which reads each place
describe("raea show --label", () => {
  const shown = (args: string[], listed?: unknown[]) => {
    const ops: [string, Record<string, unknown>][] = [];
    const ask = (op: string, b: Record<string, unknown>) => {
      ops.push([op, b]);
      if (op === "harness") return { name: "Claude Code", pid: 7 };
      return { kind: "item", id: "i-1", type: "look", title: "Three cuts", project: "shop", workspace: "Work", inbox: "/inbox.jsonl" };
    };
    showVerb(ask, args, "/work/shop", { CLAUDECODE: "1", CLAUDE_CODE_SESSION_ID: "abcdef123" }, listed);
    return ops.find(([op]) => op === "show")?.[1];
  };

  test("each --label is a place and its tag, a --detail the paragraph of the one before it", () => {
    const b = shown(["out/three.png", "--label", "0,0,.5,1 v1, hook first", "--label", ".5,0,1,1#2@3-5 v2", "--detail", "Opens on the product.", "--title", "Three cuts"]);
    expect(b?.files).toEqual(["/work/shop/out/three.png"]);
    expect(b?.labels).toEqual([
      { place: "0,0,.5,1", text: "v1, hook first" },
      { place: ".5,0,1,1#2@3-5", text: "v2", detail: "Opens on the product." },
    ]);
    expect(b?.title).toBe("Three cuts");
  });

  test("a scene part's key keeps its colons", () => {
    expect(shown(["room.tsx", "--label", "part=room.tsx:46:7 the crate"])?.labels).toEqual([{ place: "part=room.tsx:46:7", text: "the crate" }]);
  });

  test("no labels: none sent, so a sheet's own OUT.labels.json comes along", () => {
    expect(shown(["out/sheet.png"])).not.toHaveProperty("labels");
  });

  test("--labels sends the file's list as it is", () => {
    const list = [{ text: "0:01.5", box: [0, 0, 0.25, 0.5] }];
    expect(shown(["out/sheet.png", "--labels", "mine.json"], list)?.labels).toEqual(list);
  });

  test("what isn't a label is refused, saying what one is", () => {
    expect(() => shown(["a.png", "--label", "v1"])).toThrow(/--label "v1" is a place, then its tag/);
    expect(() => shown(["a.png", "--detail", "why"])).toThrow(/--detail goes after the --label/);
    expect(() => shown(["a.png", "--label", "0,0 a", "--detail", "x", "--detail", "y"])).toThrow(/one --detail for the label "a"/);
    expect(() => shown(["--question", "Which?", "--choice", "A", "--choice", "B", "--label", "0,0 a"])).toThrow(/labels go on the files you show/);
    expect(() => shown(["a.png", "--label", "0,0 a", "--labels", "l.json"], [])).toThrow(/--label or --labels, not both/);
  });
});

describe("labelsIn", () => {
  test("a list, or { labels }", () => {
    expect(labelsIn('[{"text":"a"}]', "l.json")).toEqual([{ text: "a" }]);
    expect(labelsIn('{"labels":[{"text":"b"}]}', "l.json")).toEqual([{ text: "b" }]);
  });
  test("anything else is refused, naming the file", () => {
    expect(() => labelsIn("{", "l.json")).toThrow(/--labels l.json isn't JSON/);
    expect(() => labelsIn('{"text":"a"}', "l.json")).toThrow(/--labels l.json is a list of labels/);
  });
});
