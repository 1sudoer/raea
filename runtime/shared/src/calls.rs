//! The process's JS calls, counted across its runtimes: settle's no-progress guard where an App has no JsState (a
//! replay's), and the call a hang's refusal names. Each runtime's own are its JsState (stall.rs).

use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::time::{Duration, Instant};

use crate::JsState;

/// When the JS threads' current calls began, in ms after JS_BASE plus 1 (0: none is in a call): a render's settle counts
/// a call still running as progress (a frame's render under load can take seconds before its first flush). A render runs
/// several runtimes, each with a JS thread, so JS_CALLS counts the calls under way and the time is cleared only when the
/// last ends: one thread's call ending zeroed it while another's ran on (film.tsx frame 1334 under load, 10-27).
static JS_CALL_SINCE: AtomicU64 = AtomicU64::new(0);
static JS_BASE: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
static JS_CALLS: AtomicU64 = AtomicU64::new(0);
/// Which call began the current run of calls: its runtime (numbered as each JS thread starts, from 1) times 2, plus 1
/// for an event's delivery (__raea_deliver: a frame's render) or 0 for timers (__raea_timers), so a hang's refusal
/// names it
static JS_CALL_WHO: AtomicU64 = AtomicU64::new(0);
static JS_RUNTIMES: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static JS_RUNTIME: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// This thread is a runtime's JS thread: numbered, from 1, for js_call_who.
pub fn js_runtime_starts() {
    JS_RUNTIME.set(JS_RUNTIMES.fetch_add(1, SeqCst) + 1);
}

/// The call that began the current run of calls, in words: "JS runtime 2 of 3's __raea_deliver (a frame's render)".
pub fn js_call_who() -> String {
    let who = JS_CALL_WHO.load(SeqCst);
    let what = if who % 2 == 1 { "__raea_deliver (a frame's render)" } else { "__raea_timers (its timers and effects)" };
    format!("JS runtime {} of {}'s {what}", who / 2, JS_RUNTIMES.load(SeqCst))
}

/// How long the JS threads have been in calls without a break, if one is in a call (JS_CALLS decides: an ending call can
/// clear the time just after another began, which reads as a call just begun).
pub fn js_call_for() -> Option<Duration> {
    if JS_CALLS.load(SeqCst) == 0 {
        return None;
    }
    let at = JS_CALL_SINCE.load(SeqCst);
    Some(if at == 0 { Duration::ZERO } else { JS_BASE.elapsed().saturating_sub(Duration::from_millis(at - 1)) })
}

pub fn js_call_begins(deliver: bool, state: &JsState) {
    state.in_call(true);
    if JS_CALLS.fetch_add(1, SeqCst) == 0 {
        JS_CALL_WHO.store(JS_RUNTIME.get() * 2 + deliver as u64, SeqCst);
        JS_CALL_SINCE.store(JS_BASE.elapsed().as_millis() as u64 + 1, SeqCst);
    }
}

pub fn js_call_ends(state: &JsState) {
    state.in_call(false);
    if JS_CALLS.fetch_sub(1, SeqCst) == 1 {
        JS_CALL_SINCE.store(0, SeqCst);
    }
}
