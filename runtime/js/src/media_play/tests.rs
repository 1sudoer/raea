//! The decoder's tests (media_play.rs): play, seek, loop and pause; a seek while playing; halvings.

use crate::media_html::stream::{self, Cmd, Msg, Stream};
use std::time::{Duration, Instant};

/// The next news within `within`, polled as the page host polls it (each frame).
fn next(st: &Stream, within: Duration) -> Option<Msg> {
    let t = Instant::now();
    while t.elapsed() < within {
        if let Some(m) = st.next() {
            return Some(m);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

/// A second of 30 fps picture and a tone (ffmpeg's test sources), or None without ffmpeg.
fn clip() -> Option<std::path::PathBuf> {
    clip_of("1", "one")
}

/// `seconds` of 30 fps picture and a tone, named `name` among this run's clips, or None without ffmpeg.
fn clip_of(seconds: &str, name: &str) -> Option<std::path::PathBuf> {
    let p = std::env::temp_dir().join(format!("raea-media-play-{}-{name}.mp4", std::process::id()));
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=160x90:rate=30", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", seconds, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
        .arg(&p)
        .status()
        .is_ok_and(|s| s.success());
    ok.then_some(p)
}

/// A seek while playing (raea feed's timeline, 11-16: a stretch played twice): the decoder answers with the frame
/// at the time asked before any other news from its new place, nothing it decoded for the old place comes after
/// that frame, and play goes on from the frame after it, due on a clock started once it was sent.
#[test]
fn a_seek_while_playing_answers_first_and_plays_on_from_there() {
    // SAFETY: set before any stream reads it, and nothing else in this test binary reads it concurrently
    unsafe { std::env::set_var("RAEA_AUDIO_MUTE", "1") };
    let Some(path) = clip_of("3", "seek") else { return eprintln!("no ffmpeg: skipped") };
    let mut st = stream::open(path.to_str().unwrap(), true).expect("a stream");
    assert!(matches!(next(&st, Duration::from_secs(10)), Some(Msg::Meta { .. })));
    assert!(matches!(next(&st, Duration::from_secs(5)), Some(Msg::Frame(t, ..)) if t == 0.0));
    st.ask(Cmd::Play { at: 0.0, looping: false });
    // play until frames from past a second are coming
    let until = Instant::now() + Duration::from_secs(3);
    let mut furthest = 0.0;
    while Instant::now() < until && furthest < 1.2 {
        if let Some(Msg::Due(t, ..)) = next(&st, Duration::from_millis(200)) {
            furthest = f64::max(furthest, t);
        }
    }
    assert!(furthest >= 1.2, "played to {furthest}");
    // back to 0.5 while playing: what was on its way may come first, then the seek's frame
    let asked = Instant::now();
    st.ask(Cmd::Seek(0.5));
    let mut answered = None;
    let (mut after, mut last) = (Vec::new(), 0.5);
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until && after.len() < 10 {
        match next(&st, Duration::from_millis(300)) {
            Some(Msg::Frame(t, ..)) => {
                assert_eq!(t, 0.5, "the seek's frame");
                answered = Some(Instant::now());
            }
            Some(Msg::Due(t, _, due, _)) if answered.is_some() => {
                assert!(t > last, "after the seek, frames in order from it: {t} after {last}");
                last = t;
                after.push((t, due));
            }
            _ => {}
        }
    }
    let answered = answered.expect("the seek answered with its frame");
    assert!(answered.duration_since(asked) < Duration::from_secs(1));
    // the frame after the seek's (index 15 at 30 fps is 0.5 itself): the seek's frame isn't sent twice
    let first = after.first().expect("play went on after the seek").0;
    assert!((first - 16.0 / 30.0).abs() < 1e-9, "the first frame after the seek is at {first}");
    // each due a frame on from the seek's moment, on a clock started when the seek's frame went (not before its
    // decode), so none had to be skipped: ten frames in a row
    for (i, (t, _)) in after.iter().enumerate() {
        assert!((t - (16 + i) as f64 / 30.0).abs() < 1e-9, "frame {i} after the seek is at {t}");
    }
    let (t1, due1) = after[0];
    let on_clock = due1.saturating_duration_since(answered).as_secs_f64();
    assert!(on_clock <= t1 - 0.5 + 0.03, "the first frame after the seek is due {on_clock} s after its answer");
    st.ask(Cmd::Pause);
    let _ = std::fs::remove_file(path);
}

/// A halving's pixel is the mean of the four it stands for; an odd last row or column is left out; a picture a
/// pixel wide has none; and a frame is sent whole when none are asked for.
#[test]
fn halvings_are_means_of_four() {
    use crate::media_html::stream::{halvings, sent};
    let px: Vec<u8> = (0..5 * 3 * 4).map(|i| (i * 3) as u8).collect();
    let h = halvings(5, 3, &px, 3);
    assert_eq!(h.iter().map(|(w, h, p)| (*w, *h, p.len())).collect::<Vec<_>>(), vec![(2, 1, 8)]);
    let mean = |x: usize, c: usize| ((px[x * 4 + c] as u16 + px[(x + 1) * 4 + c] as u16 + px[(5 + x) * 4 + c] as u16 + px[(5 + x + 1) * 4 + c] as u16 + 2) / 4) as u8;
    assert_eq!(h[0].2, vec![mean(0, 0), mean(0, 1), mean(0, 2), mean(0, 3), mean(2, 0), mean(2, 1), mean(2, 2), mean(2, 3)]);
    let f = raea_io_media::Frame { pts: 0.0, width: 5, height: 3, rgba: px.clone() };
    let (whole, none) = sent(f.clone(), 0);
    assert!(whole.rgba == px && none.is_empty());
    let (size, one) = sent(f, 1);
    assert!(size.rgba.is_empty() && (size.width, size.height) == (5, 3) && one == vec![h[0].clone()]);
}

/// Played on the wall's clock (RAEA_AUDIO_MUTE=1: no device), a video's frames come as they fall due and it ends
/// at its end; a seek answers with the frame at exactly its time; looping, it goes round; paused, it stops.
#[test]
fn plays_seeks_loops_and_pauses() {
    // SAFETY: set before any stream reads it, and nothing else in this test binary reads it concurrently
    unsafe { std::env::set_var("RAEA_AUDIO_MUTE", "1") };
    let Some(path) = clip() else { return eprintln!("no ffmpeg: skipped") };
    let mut st = stream::open(path.to_str().unwrap(), true).expect("a stream");
    assert!(matches!(next(&st, Duration::from_secs(10)), Some(Msg::Meta { w: 160, h: 90, .. })));
    assert!(matches!(next(&st, Duration::from_secs(5)), Some(Msg::Frame(t, ..)) if t == 0.0));

    let began = Instant::now();
    st.ask(Cmd::Play { at: 0.0, looping: false });
    assert!(matches!(next(&st, Duration::from_secs(1)), Some(Msg::Playing(t)) if t == 0.0));
    let (mut frames, mut last, mut ended) = (0, -1.0, None);
    while let Some(m) = next(&st, Duration::from_secs(3)) {
        match m {
            Msg::Due(t, f, due, _) => {
                assert!(t > last, "frames in order: {t} after {last}");
                assert_eq!((f.width, f.height), (160, 90));
                // each is due when its time comes on the clock (the wall's, here), sent before then
                let on_clock = due.duration_since(began).as_secs_f64();
                assert!((on_clock - t).abs() < 0.06, "the frame at {t} is due at {on_clock}");
                (frames, last) = (frames + 1, t);
            }
            Msg::Ended(t) => {
                ended = Some((t, began.elapsed().as_secs_f64()));
                break;
            }
            _ => {}
        }
    }
    let (end, took) = ended.expect("it ended");
    assert!((end - 1.0).abs() < 0.05, "ended at its duration: {end}");
    assert!((0.85..1.4).contains(&took), "a second's clip took {took} s");
    assert!(frames >= 25, "{frames} frames of 30");

    st.ask(Cmd::Seek(0.5));
    assert!(matches!(next(&st, Duration::from_secs(2)), Some(Msg::Frame(t, ..)) if t == 0.5));

    let looped = Instant::now();
    st.ask(Cmd::Play { at: 0.5, looping: true });
    let (mut wrapped, mut last, mut laps, until) = (false, 0.5, 0.0, Instant::now() + Duration::from_millis(2300));
    while Instant::now() < until {
        match next(&st, Duration::from_millis(200)) {
            Some(Msg::Due(t, _, due, _)) => {
                // the first frame of the next lap is sent before the last frame of this one has gone
                if t < last {
                    wrapped = true;
                    laps += 1.0;
                    assert!(due > Instant::now(), "the loop's first frame came after it was due");
                }
                // round the loop a frame is still due when the clock is at its time: half a second of play
                // to the end, then a second a lap
                let on_clock = due.duration_since(looped).as_secs_f64();
                assert!((on_clock - (t - 0.5 + laps)).abs() < 0.06, "lap {laps}: the frame at {t} is due {on_clock} s after play");
                last = t;
            }
            Some(Msg::Ended(_)) => panic!("a looping source doesn't end"),
            _ => {}
        }
    }
    assert!(wrapped, "looping went round");

    st.ask(Cmd::Pause);
    // paused, asked for two halvings: the picture showing comes again as its size and its second halving
    while next(&st, Duration::from_millis(300)).is_some() {}
    st.ask(Cmd::Halvings(2));
    match next(&st, Duration::from_secs(2)) {
        Some(Msg::Frame(_, f, halves)) => {
            assert_eq!((f.width, f.height), (160, 90));
            assert!(f.rgba.is_empty(), "the frame's own pixels stay behind");
            assert_eq!(halves.iter().map(|(w, h, px)| (*w, *h, px.len())).collect::<Vec<_>>(), vec![(40, 22, 40 * 22 * 4)]);
        }
        _ => panic!("no frame with its halvings"),
    }
    // what was on its way comes; then nothing more
    while next(&st, Duration::from_millis(300)).is_some() {}
    assert!(next(&st, Duration::from_millis(300)).is_none(), "frames after pausing");
    let _ = std::fs::remove_file(path);
}
