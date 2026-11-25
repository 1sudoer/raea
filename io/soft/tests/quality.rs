//! What openh264 is asked for reaches what it writes (#28; Windows' and Linux's encoder):
//! - (3) `--quality`: it ignored EncodeOpts.quality, so draft and master gave byte-identical files; now quality scales
//!   its bitrate (quality_kbps).
//! - (2) a fixed quantiser: the mezz copy's all-intra QP (h264_encoder's `intra_qp`) and a GOP's (GopEncoder). With
//!   rate control off, as both were, openh264 wrote QP 26 whatever was asked: QP 12 and QP 40 gave the same bytes.
#![cfg(feature = "h264")]

use raea_io_codec::EncodeOpts;
use raea_io_soft::{GopEncoder, h264_encoder};

/// soft rings moving across, `w`x`h`, frame `k`: detailed enough that the bitrate or the quantiser decides the size,
/// not noise that every rate leaves at openh264's coarsest quantiser
fn rings(w: u32, h: u32, ring: i64, k: u32) -> Vec<u8> {
    (0..w * h).flat_map(|i| {
        let (x, y) = (i % w, i / w);
        let (dx, dy) = (x as i64 - w as i64 / 2 + k as i64 * 4, y as i64 - h as i64 / 2);
        let v = ((dx * dx + dy * dy) / ring % 256) as u8;
        [v, v.wrapping_mul(3), 255 - v, 255]
    }).collect()
}

/// The size of `n` frames of rings encoded to an MP4 by h264_encoder
fn mp4(o: &EncodeOpts, intra_qp: Option<u8>, ring: i64, n: u32) -> u64 {
    let out = std::env::temp_dir().join(format!("raea-soft-quality-{}-{:?}-{intra_qp:?}.mp4", std::process::id(), o.quality));
    let mut enc = h264_encoder(&out, o, intra_qp).unwrap();
    for k in 0..n {
        enc.push(&rings(o.width, o.height, ring, k)).unwrap();
    }
    enc.finish(None).unwrap();
    let size = std::fs::metadata(&out).unwrap().len();
    let _ = std::fs::remove_file(&out);
    size
}

/// 90 frames of 640x360: three seconds, so the rate outweighs the first frame
fn encode(quality: Option<f32>) -> u64 {
    mp4(&EncodeOpts { width: 640, height: 360, fps: 30.0, kbps: 2000, quality, nv12: false, realtime: false }, None, 23, 90)
}

#[test]
fn quality_changes_what_openh264_writes() {
    let (draft, master, rate) = (encode(Some(0.6)), encode(Some(0.95)), encode(Some(0.85)));
    // before, all three were the same file
    assert!(draft * 5 / 4 < rate && rate * 5 / 4 < master, "draft {draft}, high {rate} and master {master} bytes: a higher quality spends more");
}

const W: u32 = 320;
const H: u32 = 192;

fn intra(qp: u8) -> u64 {
    mp4(&EncodeOpts { width: W, height: H, fps: 30.0, kbps: 0, quality: None, nv12: false, realtime: false }, Some(qp), 13, 6)
}

fn gop(qp: u8) -> Vec<u8> {
    let mut enc = GopEncoder::new(W, H, 30.0, qp).unwrap();
    (0..6).flat_map(|k| enc.push(&rings(W, H, 13, k)).unwrap().0).collect()
}

#[test]
fn the_mezz_copys_quantiser_changes_what_openh264_writes() {
    let (fine, coarse) = (intra(16), intra(40));
    assert!(coarse * 2 < fine, "QP 16 {fine} bytes, QP 40 {coarse}: a finer quantiser spends more");
}

#[test]
fn a_gops_quantiser_changes_what_openh264_writes() {
    let (fine, coarse) = (gop(16).len(), gop(40).len());
    assert!(coarse * 2 < fine, "QP 16 {fine} bytes, QP 40 {coarse}: a finer quantiser spends more");
    // and a GOP is still the same bytes each time it's made (an export stitched from GOPs made at different times)
    assert!(gop(16) == gop(16));
}

#[test]
fn a_quantiser_below_openh264s_floor_is_its_floor() {
    // openh264 refuses a pinned QP under 14 with rate control on (Native:3); asked for less, it gets 14
    assert!(gop(4) == gop(14));
}

/// A dim glow drifting over near-black, frame `k` of a slow move: the flat darks of a night scene, which openh264's
/// background detection took for still ground and copied forward from earlier frames (#169, "the encoder starved the
/// near-blacks" at --quality master on Windows)
fn glow(w: u32, h: u32, k: u32) -> Vec<u8> {
    let (cx, cy, r) = (w as f32 * (0.3 + k as f32 * 0.004), h as f32 * 0.45, w as f32 * 0.8);
    (0..w * h).flat_map(|i| {
        let (dx, dy) = ((i % w) as f32 - cx, (i / w) as f32 - cy);
        let v = (2.0 + 36.0 * (1.0 - (dx * dx + dy * dy).sqrt() / r).max(0.0).powi(2)).round() as u8;
        [v, v, v, 255]
    }).collect()
}

/// The share of 8x8 blocks whose mean red is more than a level off between two RGBA pictures
fn blocks_off(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    let (bw, bh) = (w as usize / 8, h as usize / 8);
    let mean = |p: &[u8], bx: usize, by: usize| (0..64).map(|k| p[((by * 8 + k / 8) * w as usize + bx * 8 + k % 8) * 4] as f32).sum::<f32>() / 64.0;
    let off = (0..bw * bh).filter(|i| (mean(a, i % bw, i / bw) - mean(b, i % bw, i / bw)).abs() > 1.0).count();
    off as f64 / (bw * bh) as f64
}

#[test]
fn a_dark_glow_drifting_keeps_every_block_at_master() {
    use raea_io_codec::Backend;
    let (w, h, n) = (640, 360, 45);
    let o = EncodeOpts { width: w, height: h, fps: 30.0, kbps: 10_000, quality: Some(0.95), nv12: false, realtime: false };
    let out = std::env::temp_dir().join(format!("raea-soft-darks-{}.mp4", std::process::id()));
    let mut enc = h264_encoder(&out, &o, None).unwrap();
    for k in 0..n {
        enc.push(&glow(w, h, k)).unwrap();
    }
    enc.finish(None).unwrap();
    let mut dec = raea_io_soft::Soft.open_video(&out).unwrap();
    let last = dec.frame_at((n - 1) as f64 / 30.0).unwrap();
    let _ = std::fs::remove_file(&out);
    let off = blocks_off(&last.rgba, &glow(w, h, n - 1), w, h);
    eprintln!("blocks off by more than a level: {:.2}%", off * 100.0);
    // with background detection on, the ground lagged the glow: blocks stood where earlier frames had left them, 5.3% of
    // them more than a level off by frame 44; without it 1.6% (the rest is the quantiser's)
    assert!(off < 0.03, "{:.1}% of 8x8 blocks more than a level off the picture at frame {}", off * 100.0, n - 1);
}
