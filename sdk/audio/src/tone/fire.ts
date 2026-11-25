// raea/tone in a page: a sound fired now (docs/hardware.feature HW0d), as <Sfx> is a game's one-shot on a frame.
import { type Ev, ctx } from "./all";

type Engine = { call: <T>(name: string, args?: unknown) => Promise<T> };

/** the graph the page's mixer has, as last sent */
let sent = "";
let fires = 0;
let said = false;

/**
 * raea: a page's one-shot, heard now. `play(time)` runs the way an <Sfx>'s does, its triggers become events, and the
 * page's mixer in raea's window plays them at once (the host call sound.fire, on raea's mixer core). Make the
 * instruments and effects once, at the top level of a module, as a composition does; `play` only triggers them.
 *   const click = new Tone.NoiseSynth({ envelope: { attack: 0.001, decay: 0.03, sustain: 0 } }).connect(Tone.buses.ui);
 *   button.onpointerdown = () => Tone.fire((t) => click.triggerAttackRelease("32n", t));
 * Returns false, with no sound, where no raea engine is under the page (a browser).
 */
export function fire(play: (time: number) => void): boolean {
  const engine = (globalThis as { __raea_engine?: Engine }).__raea_engine;
  const evs: Ev[] = [];
  const prev = [ctx.sink, ctx.sinkKey, ctx.seq, ctx.now] as const;
  ctx.sink = evs;
  // each fire's own keys: the mixer hears a key once
  ctx.sinkKey = `fire${++fires}`;
  ctx.seq = 0;
  ctx.now = 0;
  try {
    play(0);
  } finally {
    [ctx.sink, ctx.sinkKey, ctx.seq, ctx.now] = prev;
  }
  if (!engine) {
    if (!said) console.info("raea/tone fire: no raea engine under this page, so its sounds are silent");
    said = true;
    return false;
  }
  if (evs.length === 0) return true;
  const graph = ctx.graphJson();
  const args = graph === sent ? { evs } : { graph, evs };
  sent = graph;
  // a call that failed sends the graph again with the next
  engine.call("sound.fire", args).catch(() => void (sent = ""));
  return true;
}
