// `raea bug`: an agent tells raea about a bug it thinks it found in raea itself (Shane 11-16: "a way for the agent to
// safely submit bug reports to our server ... any time an agent using raea even headlessly outside the app can report
// a bug it thinks it found in raea"). It goes to raea's own server (raea.io/v1/bugs/agent), words only, with no
// account needed: raea's version and the Mac's OS are added, and paths, emails and anything shaped like a key are
// taken out here, before it leaves (the server takes them out again). The same bug sent again is counted on the first
// report. --dry-run prints what would go and sends nothing. The host (op "bug", native.ts) adds the version and sends.

type Ask = (op: string, args: Record<string, unknown>) => any;

export const BUG_USAGE = `raea bug TITLE... --text TEXT [--command CMD] [--error TEXT] [--agent NAME] [--dry-run]
  TITLE: what's wrong, in a line. TEXT: what you did, what you expected, what happened. CMD: the raea command that
  went wrong. --error: what it printed. A bug in raea itself (a command that fails or hangs, a wrong result, help that says one thing while raea does
  another), not in your own composition. Words only; paths, emails and keys are taken out before it's sent. Five a day
  from one machine; the same bug again is counted on the first report.
`;

/** What an agent wrote, safe to send: a path (/Users/..., ~/...) as <path>, an email as <email>, a key or token as
 *  <secret>. The server (apps/site/api src/bugs.ts scrubAgent) does the same again. */
export function scrub(text: string): string {
  return text
    .replace(/\b(?:Bearer|token|key|secret|password)\s*[:=]?\s*["']?[A-Za-z0-9._~+/=-]{12,}["']?/gi, "<secret>")
    .replace(/\b(?:sk|pk|rk|ghp|gho|ghs|github_pat|xox[abpr]|AKIA|AIza)[-_A-Za-z0-9]{10,}/g, "<secret>")
    .replace(/\b[A-Za-z0-9+/_-]{32,}={0,2}/g, (m) => (/[0-9]/.test(m) && /[A-Za-z]/.test(m) ? "<secret>" : m))
    .replace(/[\w.+-]+@[\w-]+\.[\w.-]+/g, "<email>")
    .replace(/(?:file:\/\/)?(?:~|\/(?:Users|home|private|var|tmp|Volumes|opt))\/[^\s'"`()<>,;]*(?: [^\s'"`()<>,;/]+\/[^\s'"`()<>,;]*)*/g, (p) => {
      const i = p.indexOf("raea.app/Contents/");
      return i >= 0 ? p.slice(i) : "<path>";
    });
}

const flagOf = (args: string[], name: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const BARE = new Set(["--dry-run"]);
const plain = (args: string[]) => args.filter((a, i) => !a.startsWith("--") && (!args[i - 1]?.startsWith("--") || BARE.has(args[i - 1])));

export function bugVerb(ask: Ask, args: string[], env: Record<string, string | undefined> = {}): string {
  const title = plain(args).join(" ").trim();
  const text = flagOf(args, "--text")?.trim();
  if (!title || !text) throw new Error(`a report needs a line saying what's wrong and --text saying what you did, what you expected and what happened.\n${BUG_USAGE}`);
  const harness = (() => {
    try {
      return (ask("harness", {}) as { name?: string } | undefined)?.name;
    } catch {
      return undefined;
    }
  })();
  const opt = (s: string | undefined, n: number) => (s?.trim() ? scrub(s.trim()).slice(0, n) : undefined);
  const report = {
    title: scrub(title).slice(0, 160),
    text: scrub(text).slice(0, 4000),
    command: opt(flagOf(args, "--command"), 300),
    error: opt(flagOf(args, "--error"), 4000),
    agent: flagOf(args, "--agent") ?? env.RAEA_AGENT ?? harness,
  };
  const dry = args.includes("--dry-run");
  const r = ask("bug", { report, dry }) as { sent?: Record<string, unknown>; id?: string; count?: number; known?: boolean; status?: number; message?: string };
  if (dry) return `Would send to raea (nothing sent):\n${JSON.stringify(r.sent ?? report, null, 2)}\n`;
  if (!r.id) throw new Error(`raea's server didn't take it${r.status ? ` (${r.status})` : ""}: ${r.message ?? "no answer"}. github.com/shinyobjectz/raea/issues takes it too.`);
  return r.known
    ? `Sent: raea already had this one (${r.id}); it's counted, ${r.count} reports now. Thank you.\n`
    : `Sent to raea as ${r.id}. Thank you. What was sent:\n${JSON.stringify(r.sent ?? report, null, 2)}\n`;
}
