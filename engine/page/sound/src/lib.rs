//! A page's interaction sounds (docs/hardware.feature HW0): raea/tone events a page fires, played now on a mixer of
//! the page's own, the same mixer core (Firewheel) a render bounces and the stage plays. raea's window has no Web Audio
//! graph (sdk/ui/src/platform/audio.ts), so this is how a page in it is heard:
//!
//!   sound.fire {graph?: string, evs: [..]} -> null
//!   sound.report {} -> {fired, late}
//!
//! `graph` is raea/tone's graph JSON (as `<audio data-tone-graph>` holds it), sent when it changed; `evs` are its events,
//! each at its own time ("t", seconds) after now. The call answers at once; the mixer runs on a thread of its own,
//! opens the device on the first fire and stops its stream after IDLE with nothing fired (the stage's rest).

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use raea_runtime_js::calls::HostCalls;
use serde_json::{Value, json};

/// How long the mixer runs with nothing fired before its stream stops.
const IDLE: Duration = Duration::from_secs(10);

enum Msg {
    Fire(Option<String>, Vec<Value>),
    Report(Sender<Value>),
}

/// The mixer's thread, started by the first call.
fn mixer() -> &'static Mutex<Sender<Msg>> {
    static TX: OnceLock<Mutex<Sender<Msg>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("raea page sound".into()).spawn(move || run(rx));
        Mutex::new(tx)
    })
}

fn run(rx: Receiver<Msg>) {
    let mut live: Option<raea_sys_mix::live::Live> = None;
    let mut failed = false;
    let (mut fired, mut last) = (0u64, Instant::now());
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Msg::Fire(graph, evs)) => {
                if live.is_none() && !failed {
                    match raea_sys_mix::live::Live::open() {
                        Ok(l) => live = Some(l),
                        Err(e) => {
                            failed = true;
                            eprintln!("raea page sound: no output device, the page's sounds stay silent ({e})");
                        }
                    }
                }
                if let Some(l) = live.as_mut() {
                    l.fire(graph.as_deref(), &evs);
                    fired += 1;
                    last = Instant::now();
                }
            }
            Ok(Msg::Report(back)) => {
                let late = live.as_ref().map_or(0, |l| l.late_events());
                let _ = back.send(json!({ "fired": fired, "late": late }));
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(l) = live.as_mut() {
                    l.tend();
                    if last.elapsed() > IDLE {
                        l.rest();
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn send(m: Msg) -> Result<(), String> {
    mixer().lock().map_err(|_| "the page's mixer stopped".to_string())?.send(m).map_err(|_| "the page's mixer stopped".to_string())
}

/// The calls, onto the page's HostCalls.
fn calls(mut calls: ResMut<HostCalls>) {
    calls
        .on("sound.fire", |_w, a, answer| {
            let graph = a.get("graph").and_then(Value::as_str).map(str::to_string);
            let evs = a.get("evs").and_then(Value::as_array).cloned().unwrap_or_default();
            match send(Msg::Fire(graph, evs)) {
                Ok(()) => answer.ok(Value::Null),
                Err(e) => answer.err(e),
            }
        })
        .on("sound.report", |_w, _a, answer| {
            let (tx, rx) = channel();
            if let Err(e) = send(Msg::Report(tx)) {
                return answer.err(e);
            }
            std::thread::spawn(move || match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(v) => answer.ok(v),
                Err(_) => answer.err("the page's mixer didn't answer"),
            });
        });
}

/// The page's sound calls (HW0a).
pub struct PageSoundPlugin;

impl Plugin for PageSoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HostCalls>().add_systems(Startup, calls);
    }
}
