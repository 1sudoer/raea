// `raea bug`: what an agent's report of a bug in raea carries to the host, which sends it to raea's server
// (raea.io/v1/bugs/agent): a line and words, scrubbed of paths, emails and keys before it leaves, the harness as the
// agent, --dry-run sending nothing. The host (native.ts bug: raea's version, the OS, curl) is faked: this is the
// command's side; the server has its own test.
import { describe, expect, test } from "bun:test";
import { bugVerb, scrub } from "../src/bug.ts";

function run(args: string[], answer: Record<string, unknown> = { id: "bug_0123456789abcdef", count: 1, known: false }, env: Record<string, string> = {}) {
  const ops: { op: string; b: Record<string, unknown> }[] = [];
  const ask = (op: string, b: Record<string, unknown>) => {
    ops.push({ op, b });
    if (op === "harness") return { name: "claude-code" };
    return b.dry ? { sent: { ...(b.report as object), raea: "0.1.5" } } : answer;
  };
  return { out: bugVerb(ask, args, env), ops, sent: () => ops.find((o) => o.op === "bug")!.b };
}

describe("raea bug", () => {
  test("a report needs a line and --text", () => {
    expect(() => run(["render hangs"])).toThrow(/--text/);
    expect(() => run(["--text", "it hangs"])).toThrow(/a line saying what's wrong/);
  });

  test("it sends the line, the words, the command and the error, with the harness as the agent", () => {
    const r = run(["render", "hangs", "at", "frame", "12", "--text", "Rendered a teaser; it stopped.", "--command", "raea render t.tsx", "--error", "frame 12/240"]);
    expect(r.sent()).toEqual({ report: { title: "render hangs at frame 12", text: "Rendered a teaser; it stopped.", command: "raea render t.tsx", error: "frame 12/240", agent: "claude-code" }, dry: false });
    expect(r.out).toContain("Sent to raea as bug_0123456789abcdef");
    // RAEA_AGENT or --agent name it instead
    expect((run(["x y", "--text", "z"], undefined, { RAEA_AGENT: "codex" }).sent().report as { agent: string }).agent).toBe("codex");
    expect((run(["x y", "--text", "z", "--agent", "kimi"]).sent().report as { agent: string }).agent).toBe("kimi");
  });

  test("paths, emails and keys are taken out before it leaves", () => {
    const r = run(["render of /Users/you/Client Work/cut.tsx hangs", "--text", "Mail you@example.com; key sk-live1234567890abcdefXYZ; file ~/Movies/a.mov", "--error", "panicked at /Users/you/raea/src/x.rs:12"]);
    const sent = JSON.stringify(r.sent());
    for (const s of ["/Users/", "you@example.com", "sk-live", "Client Work", "~/Movies"]) expect(sent).not.toContain(s);
    expect(sent).toContain("<path>");
    expect(sent).toContain("<email>");
    expect(sent).toContain("<secret>");
    // raea's own files keep where in raea they are
    expect(scrub("at /Applications/raea.app/Contents/MacOS/raea")).toContain("raea.app/Contents/MacOS/raea");
  });

  test("--dry-run sends nothing and prints what would go", () => {
    const r = run(["render hangs", "--text", "words", "--dry-run"]);
    expect(r.sent().dry).toBe(true);
    expect(r.out).toContain("nothing sent");
    expect(r.out).toContain('"raea": "0.1.5"');
  });

  test("the same bug again says it was counted; a refusal says where else to report", () => {
    expect(run(["a b", "--text", "c"], { id: "bug_0123456789abcdef", count: 3, known: true }).out).toContain("counted, 3 reports now");
    expect(() => run(["a b", "--text", "c"], { status: 429, message: "This machine has sent 5 reports today" })).toThrow(/429.*5 reports today.*github.com/);
  });
});
