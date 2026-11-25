//! #158: a stepped range's frames are each taken by their own settle when the composition reads its layout while they're
//! still queued: what step_frames relies on (here, as raea-runtime-react is at its size limit). React steps a range
//! (raea-frames) back to back without waiting on Rust, and a component that measures
//! itself in a layout effect blocks its thread on page.layout; settle answered that read by applying every queued
//! commit at once, so the frames between went by unread and the settle for the first of them waited on a frame already
//! gone (for good on 0.1.4; 20 s and "made no progress" on 0.1.5).
//!
//! The bundle is the bridge's wire by hand, no React: its settings marker, then for raea-frames each frame's two
//! commits (its marker, then its done mark, as the entry's Marker makes them) and one layout read after the last. The
//! test waits until the read is asked, so the whole range and the read are queued before settle looks: the ordering
//! the render met by chance.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use raea_runtime_react::{OneBatch, ReactOutbox, ReactPlugin, ReactProps, layout, scene};

const BUNDLE: &str = r#"
const host = globalThis.__bevyHost;
const flush = (ops) => host.op_flush(JSON.stringify(ops), false);
const conf = JSON.stringify({ width: 2, height: 2, fps: 30, frames: 4 });
flush([{ op: "create", id: 1, kind: "div", props: { "data-raea-frame": -1, "data-raea-config": conf } }, { op: "append", parent: 0, child: 1 }]);
(async () => {
  for (;;) {
    const m = await host.op_next_event();
    if (m === null) return;
    if (m.t !== "event" || m.name !== "raea-frames") continue;
    for (let f = m.value.from; f < m.value.to; f++) {
      flush([{ op: "update", id: 1, kind: "div", props: { "data-raea-frame": f } }]);
      flush([{ op: "update", id: 1, kind: "div", props: { "data-raea-done": f } }]);
    }
    // a component measuring itself after the last commit: the thread waits here for settle's answer
    __raea_call("page.layout", "{}");
  }
})();
"#;

/// The marker's (frame, done) as the World has it.
fn marker(app: &mut App) -> (i64, i64) {
    let world = app.world_mut();
    let mut q = world.query::<&ReactProps>();
    let p = q.iter(world).find(|p| p.0.contains_key("data-raea-frame")).expect("the marker");
    let n = |k: &str| p.0.get(k).and_then(|v| v.as_f64()).map_or(-2, |v| v as i64);
    (n("data-raea-frame"), n("data-raea-done"))
}

#[test]
fn a_layout_read_behind_queued_frames_lets_settle_take_each_one() {
    let dir = std::env::temp_dir().join(format!("raea-settle-reads-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let js = dir.join("bundle.js");
    std::fs::write(&js, BUNDLE).unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ReactPlugin::new(&js));
    app.insert_resource(OneBatch::default());
    scene::settle(&mut app, Some(-1), 20.0).unwrap().expect("the settings");

    // the range asked for, and stepped by the composition up to its read before settle looks
    let ask = app.world().resource::<layout::LayoutAsk>().0.clone();
    app.world().resource::<ReactOutbox>().send(serde_json::json!({"t": "event", "name": "raea-frames", "value": {"from": 0, "to": 4}}));
    let t0 = Instant::now();
    let mut seen = raea_runtime_react::flushed();
    while !ask.asked() {
        assert!(t0.elapsed() < Duration::from_secs(20), "the composition never made its read");
        seen = raea_runtime_react::wait_flush(seen, Duration::from_millis(100), Some(&*ask));
    }

    // each frame taken by its own settle, the World at that frame when it is (as step_frames reads it), in seconds. No
    // Stepping: its watchdog would ask for a lost frame again, and the frames must come without that
    let t0 = Instant::now();
    for f in 0..4 {
        scene::settle_with(&mut app, Some(f), 5.0, &mut layout::nothing_drawn()).unwrap_or_else(|e| panic!("frame {f}: {e}"));
        assert_eq!(marker(&mut app), (f, f), "frame {f}'s settle returned with the World at another frame");
    }
    assert!(t0.elapsed() < Duration::from_secs(4), "the frames took {:?}", t0.elapsed());
    // and the read answered once every commit before it was applied
    assert!(!ask.asked(), "the read is still waiting after its range");
    std::fs::remove_dir_all(&dir).ok();
}
