// The stage's side of the realtime bench (bench/realtime/stage.sh): a headless raea window running stage/page.tsx, its
// stage driven over the window's MCP server as the app's useStage drives it. For one case:
//   1. the composition A (its defaultProps as written) shown, paused, seeked to frame N, captured after each of
//      UPDATES draws of that frame (a paused stage draws again on a pointer move: one engine update each)
//   2. stage.props {PROP: B} on that running world: the frame drawn again with them (ab), captured the same way, and
//      the time from the call to the stage's next painted frame
//   3. the same frame seeked to again, as the app's useStage did after props on a paused stage (abs): the time from
//      the props call to the frame it paints, and captures as above
//   4. a drag (RT4e): DRAGS props changes one after another, A and B in turn (ending on B), each waited for until
//      painted: the milliseconds from each call to the window's next painted frame (over MCP, an upper bound) and the
//      stage's own (status.props.ms: the call taken to the frame in the stage's texture), p50 and p95
//   5. a burst: BURST props sent at once from the page (rt.calls), ending on B, as a drag sends them faster than a
//      frame draws: how many the stage passed over (newest wins), how many it drew, the values it ends on, and the
//      frame captured once (burst)
//   6. the copy with B written into defaultProps shown over it (RT4f): taken in place when only values changed and
//      the 3D keeps none of them (status.last_show "adopted"), else opened; the time to its frame, captured (ad)
//   7. the copy shown fresh (the stage cleared first, so it is opened) and captured the same way (b)
// Captures are the window's own frame (frame_png), one window point a stage pixel. Writes OUT/{a,ab,abs,burst,ad,b}-K.png
// and OUT/stage.json ({ props_ms, drag, burst, adopt, steps, window, scale }).
//   node bench/realtime/stage.mjs PORT OUT A.tsx B.tsx PROP A_JSON B_JSON FRAME W H UPDATES [MODE]
// MODE: render (the default: stage.show {render: true}, drawn as `raea render` draws it) or live (as a game: the
// stage's own session). RT_DRAGS (30) and RT_BURST (12) size steps 4 and 5.
import { writeFileSync } from "node:fs";

const [port, out, fileA, fileB, prop, aJson, valueJson, frameS, wS, hS, updatesS, modeS] = process.argv.slice(2);
const N = Number(frameS), W = Number(wS), H = Number(hS), UPDATES = Number(updatesS ?? 3);
const was0 = JSON.parse(aJson), value = JSON.parse(valueJson);
const RENDER = (modeS ?? "render") !== "live";
const DRAGS = Number(process.env.RT_DRAGS ?? 30), BURST = Number(process.env.RT_BURST ?? 12);
let id = 0;
async function tool(name, args = {}) {
  const r = await fetch(`http://127.0.0.1:${port}/mcp`, {
    method: "POST",
    headers: { "content-type": "application/json", accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }),
  });
  const j = await r.json();
  if (j.error) throw new Error(`${name}: ${j.error.message}`);
  return j.result;
}
const text = (res) => res?.content?.find((c) => c.type === "text")?.text;
/** a stage host call through the page's one action */
async function stage(name, args) {
  const res = await tool("action", { name: "rt.call", args: { name, args } });
  const t = text(res);
  let v;
  try {
    v = t ? JSON.parse(t) : res;
  } catch {
    // the window's own answer when the page didn't answer in time (a stage busy opening a file): an error, retried
    // by `until`
    throw new Error(`${name}: ${t}`);
  }
  if (v && typeof v === "object" && "error" in v && Object.keys(v).length === 1) throw new Error(`${name}: ${v.error}`);
  return v?.value !== undefined && Object.keys(v).length === 1 ? v.value : v;
}
/** the stage's status, asked again while the page is too busy to answer (up to a minute) */
async function status() {
  const end = Date.now() + 60_000;
  for (;;) {
    try {
      return await stage("stage.status", null);
    } catch (e) {
      if (Date.now() > end) throw e;
      await new Promise((ok) => setTimeout(ok, 50));
    }
  }
}
async function png(file) {
  const res = await tool("frame_png");
  const data = res?.content?.find((c) => c.type === "image")?.data;
  if (!data) throw new Error("frame_png: no image");
  writeFileSync(file, Buffer.from(data, "base64"));
}
const pause = (ms) => new Promise((ok) => setTimeout(ok, ms));
/** until `check` answers, polling the stage's status; the bound is the work's own (an opening can bundle for seconds) */
async function until(what, check, ms) {
  const end = Date.now() + ms;
  for (;;) {
    let s;
    try {
      s = await status();
    } catch (e) {
      if (Date.now() > end) throw e;
      await pause(50);
      continue;
    }
    if (s?.error) throw new Error(`${what}: the stage says ${s.error}`);
    if (check(s)) return s;
    if (Date.now() > end) throw new Error(`${what}: not within ${ms} ms (status ${JSON.stringify(s)})`);
    await pause(5);
  }
}

const result = { case: { fileA, fileB, prop, value, frame: N, size: [W, H] }, steps: [] };
// the draws captured: every one up to 8, then 12, 16, 24, 32, 48, 64, ... and the last
const KS = new Set([...Array.from({ length: Math.min(UPDATES, 9) }, (_, k) => k), ...[12, 16, 24, 32, 48, 64, 96, 128].filter((k) => k < UPDATES), UPDATES - 1]);
/** the frame drawn again `UPDATES` times (k = 0 is the draw that put it there), captured after the draws in KS */
async function captures(tag) {
  for (let k = 0; k < UPDATES; k++) {
    if (k > 0) {
      const was = (await status()).painted;
      await stage("stage.pointer", { x: 1 + (k % 2), y: 1, down: false });
      await until(`${tag} update ${k}`, (s) => s.painted > was, 30_000);
    }
    if (!KS.has(k)) continue;
    // the window composites the stage's newest frame on its next frame: two window frames on, so the capture is it
    await tool("frame_png");
    await png(`${out}/${tag}-${k}.png`);
    console.error(`captured ${tag}-${k}`);
  }
}
const pct = (xs, p) => {
  const v = [...xs].sort((a, b) => a - b);
  return v.length ? Math.round(v[Math.min(v.length - 1, Math.floor((p / 100) * v.length))] * 10) / 10 : null;
};
async function show(file, tag) {
  const t0 = Date.now();
  // answered once sent; an answer that didn't come back in time (the stage thread opening the last file) is waited
  // out by the status below
  await stage("stage.show", { src: file, w: W, h: H, render: RENDER }).catch((e) => console.error(`show: ${e.message}`));
  await until(`${tag} opened`, (s) => s.showing === file && s.painted > 0, 180_000);
  const opened = Date.now() - t0;
  await stage("stage.play", { on: false });
  const was = (await status()).painted;
  await stage("stage.seek", { frame: N });
  const s = await until(`${tag} at ${N}`, (s) => s.frame === N && s.painted > was && !s.playing, 60_000);
  result.steps.push({ tag, opened_ms: opened, frame: s.frame });
  await captures(tag);
}

try {
  const state = JSON.parse(text(await tool("state")) ?? "{}");
  result.window = state.window ?? null;
  result.scale = state.scale ?? null;
  await show(fileA, "a");
  // the props on the running world: from the call to the stage's next painted frame
  const was = (await status()).painted;
  const t0 = performance.now();
  const answer = await stage("stage.props", { props: { [prop]: value } });
  const answered = performance.now() - t0;
  await until("props drawn", (s) => s.painted > was, 30_000);
  const after = await status();
  result.props_ms = { answered: Math.round(answered), painted: Math.round(performance.now() - t0), stage: after.props?.ms ?? null, answer };
  result.held = answer?.held ?? null;
  await captures("ab");
  // props, then the frame seeked to again behind them (the app sends the seek without waiting for the props' answer;
  // here it follows the answer, so the time is an upper bound by one round trip)
  const was2 = (await status()).painted;
  const t1 = performance.now();
  await stage("stage.props", { props: { [prop]: value } });
  await stage("stage.seek", { frame: N });
  await until("props and seek drawn", (s) => s.painted >= was2 + 2 && s.frame === N, 30_000);
  result.props_seek_ms = Math.round(performance.now() - t1);
  await captures("abs");
  // a drag: each change waited for until painted
  const wall = [], own = [];
  for (let i = 0; i < DRAGS; i++) {
    const v = i % 2 === 0 ? was0 : value;
    const w = (await status()).painted;
    const t = performance.now();
    await stage("stage.props", { props: { [prop]: v } });
    const s = await until(`drag ${i}`, (s) => s.painted > w && s.props?.ms > 0, 30_000);
    wall.push(performance.now() - t);
    own.push(s.props.ms);
  }
  result.drag = { n: DRAGS, wall_p50: pct(wall, 50), wall_p95: pct(wall, 95), stage_p50: pct(own, 50), stage_p95: pct(own, 95), stage_max: pct(own, 100) };
  console.error(`drag ${JSON.stringify(result.drag)}`);
  // a burst, ending on B (the drag ended on B too: the frame is compared against B's render either way)
  if (BURST > 0) {
    const s0 = await status();
    const calls = Array.from({ length: BURST }, (_, i) => ({ name: "stage.props", args: { props: { [prop]: (BURST - 1 - i) % 2 === 0 ? value : was0 } } }));
    const t = performance.now();
    const res = await tool("action", { name: "rt.calls", args: { calls } });
    const answers = JSON.parse(text(res) ?? "null");
    const answered = performance.now() - t;
    // each props set is passed over or drawn (painted in the stage's turn that took it)
    const s1 = await until("burst drawn", (s) => s.props.drawn + s.props.dropped >= s0.props.drawn + s0.props.dropped + BURST, 30_000);
    const list = Array.isArray(answers) ? answers : answers?.value;
    result.burst = { sent: BURST, answered_ms: Math.round(answered), dropped: s1.props.dropped - s0.props.dropped, drawn: s1.props.drawn - s0.props.drawn, ends_on: s1.values?.[prop], dropped_answers: Array.isArray(list) ? list.filter((a) => a?.value?.dropped).length : null };
    console.error(`burst ${JSON.stringify(result.burst)}`);
    await tool("frame_png");
    await png(`${out}/burst-0.png`);
  }
  // the copy written with B: taken in place, or opened when the 3D held the value
  {
    const s0 = await status();
    const t = performance.now();
    await stage("stage.show", { src: fileB, w: W, h: H, render: RENDER }).catch((e) => console.error(`adopt: ${e.message}`));
    const s1 = await until("copy shown", (s) => s.showing === fileB && (s.adopted > s0.adopted || s.opens > s0.opens) && s.painted > 0, 180_000);
    let s2 = s1;
    if (s1.opens > s0.opens) {
      // opened: paused and seeked back to N, as the app's useStage does after a reopen
      await stage("stage.play", { on: false });
      const w = (await status()).painted;
      await stage("stage.seek", { frame: N });
      s2 = await until("reopened at N", (s) => s.frame === N && s.painted > w && !s.playing, 60_000);
    } else await until("adopted drawn", (s) => s.painted > s0.painted, 30_000);
    result.adopt = { how: s1.last_show?.how, stage_ms: s1.last_show?.ms, wall_ms: Math.round(performance.now() - t), frame: s2.frame, playing: s2.playing, values: s2.values?.[prop] };
    console.error(`adopt ${JSON.stringify(result.adopt)}`);
    await captures("ad");
  }
  await stage("stage.show", { src: null, w: W, h: H }).catch((e) => console.error(`clear: ${e.message}`));
  await until("cleared", (s) => s.showing == null, 30_000);
  await show(fileB, "b");
  result.status = await status();
  result.ok = true;
} catch (e) {
  result.ok = false;
  result.error = String(e?.message ?? e);
  console.error(`FAIL ${result.error}`);
}
writeFileSync(`${out}/stage.json`, JSON.stringify(result, null, 1));
console.log(JSON.stringify({ ok: result.ok, props_ms: result.props_ms, props_seek_ms: result.props_seek_ms, held: result.held, drag: result.drag, burst: result.burst, adopt: result.adopt, steps: result.steps, window: result.window, scale: result.scale, error: result.error }));
process.exit(result.ok ? 0 : 1);
