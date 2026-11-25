//! A video made as segments and joined by stream copy (docs/rerender.feature RR3), on the portable encoder (openh264):
//! every frame there once and in order, a keyframe where each segment starts and nowhere a cut was passed over, the
//! sound laid under once, and the pictures the segments' own.

use std::path::{Path, PathBuf};

use raea_io_media::{Audio, EncodeOpts};
use raea_runtime_segments::{Cuts, encoder, join};

const W: u32 = 64;
const H: u32 = 64;
const FPS: f64 = 30.0;

fn opts() -> EncodeOpts {
    EncodeOpts { width: W, height: H, fps: FPS, kbps: 800, quality: None, nv12: false, realtime: false }
}

/// Frame i: a flat grey that steps every frame, with a bar whose place says which frame it is.
fn frame(i: usize) -> Vec<u8> {
    let mut px = vec![0u8; (W * H * 4) as usize];
    for (k, p) in px.chunks_exact_mut(4).enumerate() {
        let x = k % W as usize;
        let v = if x / 4 == i % 16 { 250 } else { (40 + i * 3 % 120) as u8 };
        p.copy_from_slice(&[v, v, v, 255]);
    }
    px
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("raea-segments-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The video track.
fn video(m: &re_mp4::Mp4) -> &re_mp4::Track {
    m.tracks().values().find(|t| t.kind == Some(re_mp4::TrackKind::Video)).unwrap()
}

/// The video track's samples' key flags, and whether there's a sound track.
fn keys(path: &Path) -> (Vec<bool>, bool) {
    let b = std::fs::read(path).unwrap();
    let m = re_mp4::Mp4::read_bytes(&b).unwrap();
    let video = video(&m);
    // a track besides the video: re_mp4 names only AAC (mp4a) a sound track; Opus (soft, av1) reads as kind None
    let sound = m.tracks().len() > 1;
    (video.samples.iter().map(|s| s.is_sync).collect(), sound)
}

/// The video track's samples as stored.
fn samples(path: &Path) -> Vec<Vec<u8>> {
    let b = std::fs::read(path).unwrap();
    let m = re_mp4::Mp4::read_bytes(&b).unwrap();
    let video = video(&m);
    video.samples.iter().map(|s| b[s.byte_range()].to_vec()).collect()
}

fn silence(frames: usize) -> Audio {
    Audio { rate: 48000, channels: vec![vec![0.0; 48000 * frames / FPS as usize]; 2] }
}

#[test]
fn segments_joined_are_one_video_with_one_sound() {
    let d = scratch("join");
    let soft = raea_io_media::backend_unsigned("soft").unwrap();
    let mut paths = Vec::new();
    for (k, (from, n)) in [(0usize, 10usize), (10, 7)].into_iter().enumerate() {
        let p = d.join(format!("s{k}.mp4"));
        let mut e = soft.encoder(&p, &opts()).unwrap();
        (from..from + n).for_each(|i| e.push(&frame(i)).unwrap());
        e.finish(None).unwrap();
        paths.push(p);
    }
    let out = d.join("joined.mp4");
    let sound = soft.sound(&silence(17)).unwrap();
    assert_eq!(join(&paths, Some(&sound), FPS, &out).unwrap(), 17);
    let (k, has_sound) = keys(&out);
    assert_eq!(k.len(), 17);
    assert!(k[0] && k[10], "each segment starts on a keyframe: {k:?}");
    assert!(has_sound);
    // the joined video's samples are its segments' own bytes: nothing was encoded again
    let parts: Vec<Vec<u8>> = paths.iter().flat_map(|p| samples(p)).collect();
    assert!(samples(&out) == parts, "the joined samples aren't the segments'");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_encoder_splits_at_cuts_but_not_short_shots() {
    let d = scratch("cuts");
    let out = d.join("film.mp4");
    let cuts = Cuts::default();
    // 30 fps: a segment holds at least 30 frames, so the cut at 45 (15 frames after 30) is passed over
    for f in [30, 45, 80] {
        cuts.mark(f);
    }
    // SAFETY: the only test in this binary that reads the environment
    unsafe { std::env::set_var("RAEA_SEGMENTS", "1") };
    let mut e = encoder("soft", &out, &opts(), false, &cuts).unwrap();
    (0..100).for_each(|i| e.push(&frame(i)).unwrap());
    let st = e.finish(Some(&silence(100))).unwrap();
    assert_eq!(st.frames, 100);
    assert!(st.video_codec.ends_with("in 3 segments"), "{}", st.video_codec);
    let (k, has_sound) = keys(&out);
    let at: Vec<usize> = k.iter().enumerate().filter(|(_, s)| **s).map(|(i, _)| i).collect();
    assert_eq!(at, vec![0, 30, 80]);
    assert!(has_sound);
    let _ = std::fs::remove_dir_all(&d);
}
