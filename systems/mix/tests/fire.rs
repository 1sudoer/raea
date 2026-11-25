//! HW0b (docs/hardware.feature): a page's fired sounds reach the device within the mixer's head. 200 presses, 15 ms
//! apart, each a short note fired now: none of their events reaches the audio thread after its sample, and the tap
//! (what the limiter gave, before the mute) holds all 200. It opens the real output device, muted, so it's ignored by
//! default: `cargo test -p raea-sys-mix --test fire -- --ignored`.

use std::time::Duration;

#[test]
#[ignore = "opens the audio output device"]
fn two_hundred_presses_none_late() {
    let tap = std::env::temp_dir().join(format!("raea-fire-tap-{}.wav", std::process::id()));
    // SAFETY: this test's process has no other thread reading the environment yet
    unsafe {
        std::env::set_var("RAEA_AUDIO_MUTE", "1");
        std::env::set_var("RAEA_AUDIO_TAP", &tap);
    }
    let graph = serde_json::json!({
        "nodes": [
            {"id": 0, "kind": "Destination", "options": {"volume": 0}},
            {"id": 1, "kind": "Channel", "name": "ui", "options": {"volume": 0, "pan": 0}},
            {"id": 2, "kind": "MembraneSynth", "options": {"pitchDecay": 0.008, "octaves": 2, "envelope": {"attack": 0.0005, "decay": 0.03, "sustain": 0, "release": 0.01}, "volume": -12}}
        ],
        "edges": [[2, 1, 0], [1, 0, 0]]
    })
    .to_string();
    let press = |i: usize| serde_json::json!({"k": format!("fire{i}:0"), "n": 2, "t": 0.0, "e": "ar", "f": [180.0], "d": 0.03, "v": 1.0});

    let mut live = raea_sys_mix::live::Live::open().expect("an output device");
    for i in 0..200 {
        // the graph goes with the first fire only, as raea/tone's fire() sends it
        live.fire(if i == 0 { Some(&graph) } else { None }, &[press(i)]);
        // the presses' own rhythm (not a wait for anything): a fast typist, 15 ms apart
        std::thread::sleep(Duration::from_millis(15));
        live.tend();
    }
    std::thread::sleep(Duration::from_millis(200));
    live.tend();
    let late = live.late_events();
    drop(live);

    assert_eq!(late, 0, "{late} of the fired events reached the audio thread after their sample");
    let a = raea_io_media::decode_audio_window(&tap, 0.0, 60.0).expect("the tap");
    let _ = std::fs::remove_file(&tap);
    let _ = std::fs::remove_file(tap.with_extension("json"));
    // an onset: the left channel crossing 0.05 after 5 ms under it (each press rings 30 ms, 15 ms apart: they overlap,
    // so count the attacks by their rise instead)
    let l = &a.channels[0];
    let (mut onsets, mut quiet) = (0, 0usize);
    let step = (a.rate as usize) / 1000;
    let mut prev = 0f32;
    for w in l.chunks(step) {
        let peak = w.iter().fold(0f32, |m, s| m.max(s.abs()));
        if peak > 0.05 && peak > prev * 1.8 && quiet >= 3 {
            onsets += 1;
            quiet = 0;
        } else {
            quiet += 1;
        }
        prev = peak;
    }
    eprintln!("HW0b: 200 fired, {late} late, {onsets} presses in the tap");
    assert!((190..=210).contains(&onsets), "the tap holds {onsets} presses of 200");
}
