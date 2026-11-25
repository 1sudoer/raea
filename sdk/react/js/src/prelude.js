// The host side of bevy-react's bridge on our own V8 (no deno_core): __bevyHost's five calls, timers and microtasks.
(() => {
  const g = globalThis;
  g.console = { log: (...a) => __raea_log(0, a.map(String).join(" ")), info: (...a) => __raea_log(0, a.map(String).join(" ")),
    warn: (...a) => __raea_log(1, a.map(String).join(" ")), error: (...a) => __raea_log(2, a.map((x) => (x && x.stack) || String(x)).join(" ")),
    debug: () => {} };
  g.queueMicrotask = (fn) => { Promise.resolve().then(fn); };
  // timers: kept here, run by the host's loop through __raea_timers(now) (returns ms to the next one, or -1)
  let seq = 1; const timers = new Map();
  g.setTimeout = (fn, ms = 0, ...args) => { const id = seq++; timers.set(id, { at: __raea_now() + Math.max(0, ms), fn, args }); return id; };
  g.clearTimeout = (id) => { timers.delete(id); };
  // an interval re-arms before it runs (as engine/native's js.rs does), so a clearInterval inside it still stops it;
  // it re-armed after only while its id was in `timers`, which __raea_timers had just deleted, so it ran once
  g.setInterval = (fn, ms = 0, ...args) => { const id = seq++; const ev = Math.max(1, +ms || 0); const t = { at: __raea_now() + ev, fn: () => { t.at = __raea_now() + ev; timers.set(id, t); fn(...args); }, args: [] }; timers.set(id, t); return id; };
  g.clearInterval = g.clearTimeout;
  g.__raea_timers = (now) => {
    for (const [id, t] of [...timers].sort((a, b) => a[1].at - b[1].at)) {
      if (t.at > now) break;
      timers.delete(id);
      try { t.fn(...t.args); } catch (e) { console.error(e); }
    }
    let next = -1;
    for (const t of timers.values()) next = next < 0 ? t.at - now : Math.min(next, t.at - now);
    return next;
  };
  g.performance = { now: () => __raea_now() };
  g.process = { env: { NODE_ENV: "production" } };
  g.__bevyHost = {
    op_flush: (json, devtools) => __raea_flush(json),
    op_emit: () => {},
    op_request: () => {},
    op_animate: () => {},
    // events from Bevy: queued until the bridge asks, or handed to the waiting ask (null = Bevy shut down)
    op_next_event: () => (inbox.length ? Promise.resolve(inbox.shift()) : new Promise((r) => { waiter = r; })),
  };
  const inbox = []; let waiter = null;
  g.__raea_deliver = (json) => {
    const m = json === null ? null : JSON.parse(json);
    // the World's shared values that changed (B3 H4): to @raea/app's hooks.ts store, not the bridge
    if (m && m.t === "event" && m.name === "raea-world") { if (g.__raea_world) g.__raea_world(m.value); return; }
    // 1: handed to the bridge's waiting loop; 0: queued with no one waiting. Between the host's calls the loop always
    // waits on op_next_event, so a 0 means it has stopped, and the host says so if a frame then never comes (#158)
    if (waiter) { const w = waiter; waiter = null; w(m); return 1; }
    inbox.push(m);
    return 0;
  };
  // the bridge's event loop ended by a throw (an error out of an event handler's flushSync, which nothing caught): put
  // where Rust reads a composition's error, so the render fails naming it, not waiting on a loop that stopped (#158)
  g.__raea_loop_died = (e) => {
    const msg = "the composition's event loop stopped: " + (e && e.message ? (e.name || "Error") + ": " + e.message + (e.stack ? "\n" + e.stack : "") : String(e));
    console.error("[js] " + msg);
    __raea_flush(JSON.stringify([{ op: "create", id: 2 ** 52 - 1, kind: "div", props: { "data-raea-error": msg.slice(0, 4000) } }]));
  };
  // Number's toLocaleString through one Intl.NumberFormat per locale and options, kept: the spec's toLocaleString(l, o)
  // is new Intl.NumberFormat(l, o).format(n), and JavaScriptCore made that formatter at every call. A counter a
  // timeline formats each frame was most of its frame (lightspeed T3: 56% of titles' JS). Same strings, made once
  const numberFormats = new Map();
  const toLocale = Number.prototype.toLocaleString;
  Object.defineProperty(Number.prototype, "toLocaleString", {
    value: function toLocaleString(locales, options) {
      "use strict";
      if (typeof this !== "number") return toLocale.call(this, locales, options);
      let key;
      if (options === undefined) key = locales === undefined ? "" : typeof locales === "string" ? locales : null;
      else if (typeof options === "object" && options !== null && (locales === undefined || typeof locales === "string")) {
        try { key = (locales || "") + "\u0000" + JSON.stringify(options); } catch { key = null; }
      } else key = null;
      if (key === null) return toLocale.call(this, locales, options);
      let f = numberFormats.get(key);
      if (f === undefined) {
        f = new Intl.NumberFormat(locales, options);
        if (numberFormats.size < 64) numberFormats.set(key, f);
      }
      return f.format(this);
    },
    writable: true,
    configurable: true,
  });
})();
