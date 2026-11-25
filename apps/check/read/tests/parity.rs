//! Each native reader against the FFmpeg read it stands in for (lib.rs names the command), on a clip FFmpeg makes: a
//! moving test picture that freezes for its last 4 s, over stereo sound whose level swells. `raea check` without FFmpeg
//! must reach the verdicts it reaches with it, so each answer is held to FFmpeg's within what a rule could notice.
//! Skipped where FFmpeg isn't installed (there is nothing to compare against).

use std::path::Path;
use std::process::Command;

fn ffmpeg(args: &[&str]) -> (Vec<u8>, String) {
    let out = Command::new("ffmpeg").args(["-hide_banner", "-nostats", "-nostdin"]).args(args).output().expect("ffmpeg runs");
    assert!(out.status.success(), "ffmpeg {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    (out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
}

fn clip(dir: &Path) -> String {
    let file = dir.join("clip.mp4").to_string_lossy().into_owned();
    let picture = "testsrc2=size=320x180:rate=30:duration=5,tpad=stop_mode=clone:stop_duration=4";
    let sound = "aevalsrc=0.4*sin(2*PI*440*t)*(0.55+0.45*sin(2*PI*0.25*t))|0.25*sin(2*PI*660*t):s=48000:d=9";
    ffmpeg(&["-y", "-f", "lavfi", "-i", picture, "-f", "lavfi", "-i", sound, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-shortest", &file]);
    file
}

/// The mean absolute difference of two byte runs, in levels
fn apart(a: &[u8], b: &[u8]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y) as u64).sum::<u64>() as f64 / a.len().max(1) as f64
}

/// Root-mean-square level in dB of each 100 ms of `x` at `rate`
fn levels(x: &[f32], rate: usize) -> Vec<f64> {
    x.chunks_exact(rate / 10).map(|c| 10.0 * (c.iter().fold(0.0, |s, v| s + v * v) as f64 / c.len() as f64 + 1e-12).log10()).collect()
}

/// Each measure's name, how far the native read is from FFmpeg's, and how far it may be: all printed, then held
struct Off(Vec<(String, f64, f64)>);

impl Off {
    fn by(&mut self, what: impl Into<String>, off: f64, most: f64) {
        self.0.push((what.into(), off, most));
    }
}

#[test]
fn the_native_reads_match_ffmpeg_s() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("skipped: no FFmpeg to compare against");
        return;
    }
    let dir = std::env::temp_dir().join(format!("raea-check-read-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let v = &clip(&dir);
    let mut off = Off(vec![]);
    let by_ffmpeg = raea_app_check_read::probe(v).unwrap();
    // SAFETY: set before any reader runs, on this test's only thread
    unsafe { std::env::set_var("RAEA_CHECK_NATIVE", "1") };
    assert!(raea_app_check_read::native());
    let p = raea_app_check_read::probe(v).unwrap();
    assert_eq!((p.w, p.h, p.audio), (by_ffmpeg.w, by_ffmpeg.h, by_ffmpeg.audio));
    assert!((p.duration - by_ffmpeg.duration).abs() < 0.05 && (p.picture - by_ffmpeg.picture).abs() < 0.05, "{} {} vs {} {}", p.duration, p.picture, by_ffmpeg.duration, by_ffmpeg.picture);

    // flash-frame's light per frame
    let (gray, _) = ffmpeg(&["-v", "error", "-i", v, "-map", "0:v:0", "-vf", "scale=32:18:flags=area,format=gray", "-f", "rawvideo", "-"]);
    let theirs: Vec<f64> = gray.chunks_exact(32 * 18).map(|f| f.iter().map(|&b| b as f64).sum::<f64>() / (32.0 * 18.0 * 255.0)).collect();
    let (ours, fps) = raea_app_check_read::lumas(v).unwrap();
    assert_eq!((ours.len(), fps), (theirs.len(), 30.0));
    off.by("a frame's light, of 1", ours.iter().zip(&theirs).map(|(a, b)| (*a as f64 - b).abs()).fold(0.0, f64::max), 0.01);

    // blotching's dark crops every 2 s
    let (w, h) = (320, 176);
    let (gray, _) = ffmpeg(&["-v", "error", "-i", v, "-vf", &format!("fps=0.5,crop={w}:{h}:0:0,format=gray"), "-f", "rawvideo", "-"]);
    let mut crops = vec![];
    raea_app_check_read::crops(v, w, h, |c| crops.push(c.to_vec())).unwrap();
    assert_eq!(crops.len(), gray.len() / (w * h));
    for (i, (a, b)) in crops.iter().zip(gray.chunks_exact(w * h)).enumerate() {
        let bias = a.iter().zip(b).map(|(x, y)| *x as f64 - *y as f64).sum::<f64>() / a.len() as f64;
        off.by(format!("blotching crop {i}, levels (bias {bias:+.2})"), apart(a, b), 3.0);
    }

    // the flashing screen's frames: area-sampled beside point-sampled RGB
    let (fw, fh) = (192, 108);
    let graph = format!("split=2[area][point];[area]scale={fw}:{fh}:flags=area[a];[point]scale={fw}:{fh}:flags=neighbor[b];[a][b]hstack=inputs=2,format=rgb24");
    let (rgb, _) = ffmpeg(&["-v", "error", "-i", v, "-map", "0:v:0", "-vf", &graph, "-f", "rawvideo", "-"]);
    let (mut frames, mut times) = (vec![], vec![]);
    let n = raea_app_check_read::flash_frames(v, fw, fh, |t, buf| {
        times.push(t);
        frames.push(buf.to_vec());
    })
    .unwrap();
    assert_eq!(n, rgb.len() / (fw * 2 * fh * 3));
    assert!(times.windows(2).all(|t| (t[1] - t[0] - 1.0 / 30.0).abs() < 1e-3), "frame times step by a frame");
    off.by("a flash frame, levels", frames.iter().zip(rgb.chunks_exact(fw * 2 * fh * 3)).map(|(a, b)| apart(a, b)).fold(0.0, f64::max), 6.0);

    // the sound folded to mono, 100 ms at a time
    let (pcm, _) = ffmpeg(&["-v", "error", "-i", v, "-vn", "-ac", "1", "-ar", "24000", "-f", "f32le", "-"]);
    let theirs = levels(&pcm.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect::<Vec<_>>(), 24000);
    let ours = levels(&raea_app_check_read::mono(v, 24000).unwrap(), 24000);
    assert!(ours.len().abs_diff(theirs.len()) <= 1, "{} windows vs {}", ours.len(), theirs.len());
    off.by("100 ms of mono sound, dB", ours.iter().zip(&theirs).skip(1).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max), 0.5);

    // momentary loudness every 100 ms
    let (log, _) = ffmpeg(&["-i", v, "-vn", "-af", "ebur128=metadata=1,ametadata=print:key=lavfi.r128.M:file=-", "-f", "null", "-"]);
    let theirs: Vec<f64> = String::from_utf8_lossy(&log).lines().filter_map(|l| l.strip_prefix("lavfi.r128.M=")?.trim().parse().ok()).collect();
    let (at, ours) = raea_app_check_read::meter(v).unwrap();
    assert!(ours.len().abs_diff(theirs.len()) <= 2 && at.len() == ours.len(), "{} readings vs {}", ours.len(), theirs.len());
    off.by("a momentary loudness, LU", ours.iter().zip(&theirs).skip(4).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max), 0.5);

    // the freeze: 5 s in, to the end
    let (_, log) = ffmpeg(&["-i", v, "-an", "-vf", "freezedetect=n=-60dB:d=3", "-f", "null", "-"]);
    let starts = |log: &str| log.lines().filter_map(|l| l.split("freeze_start: ").nth(1)?.trim().parse::<f64>().ok()).collect::<Vec<_>>();
    let (theirs, ours) = (starts(&log), starts(&raea_app_check_read::freezes(v, -60.0, 3.0).unwrap()));
    assert!(ours.len() == 1 && theirs.len() == 1, "freezes from {ours:?} vs {theirs:?}");
    off.by("the freeze's start, s", (ours[0] - theirs[0]).abs(), 0.1);

    // the mix's loudness and a caption's contrast, against raea-sys-measure's FFmpeg reads
    let (i, tp) = raea_app_check_read::loudness(v).unwrap();
    let (fi, ftp) = raea_sys_measure::loudness(Path::new(v)).unwrap();
    off.by("integrated loudness, LU", (i - fi).abs(), 0.3);
    off.by("true peak, dB", (tp.unwrap() - ftp.unwrap()).abs(), 0.5);
    let ours = raea_app_check_read::contrast(v, 1.0, [40, 60, 120, 60], "rgb(255, 255, 255)").unwrap();
    let theirs = raea_sys_measure::contrast(Path::new(v), 1.0, [40, 60, 120, 60], "rgb(255, 255, 255)").unwrap();
    off.by("a caption's contrast, share", (ours / theirs - 1.0).abs(), 0.05);
    let _ = std::fs::remove_dir_all(&dir);
    for (what, by, most) in &off.0 {
        eprintln!("{} {what}: {by:.4} (at most {most})", if by < most { "ok  " } else { "OVER" });
    }
    assert!(off.0.iter().all(|(_, by, most)| by < most), "a native read is further from FFmpeg's than a rule allows");
}

/// The one read (once.rs) against the four reads it stands in for, each run alone as raea check ran it: each rule gets
/// the same bytes, frames and log lines. Away from Unix the rules read alone (no named pipes), and this is skipped.
#[test]
fn one_read_gives_each_rule_what_its_own_read_gave() {
    if !raea_app_check_read::once::can() {
        eprintln!("skipped: the rules read alone here");
        return;
    }
    let dir = std::env::temp_dir().join(format!("raea-once-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("clip.mp4").to_string_lossy().into_owned();
    ffmpeg(&["-y", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30:duration=5,tpad=stop_mode=clone:stop_duration=4", "-c:v", "libx264", "-pix_fmt", "yuv420p", &file]);
    let (w, h, cw, ch) = (192usize, 108usize, 320usize, 176usize);
    let (mut frames, mut crops) = (Vec::new(), Vec::new());
    let once = raea_app_check_read::once::picture(&file, (w, h), Some((cw, ch)), 3.0, |t, b| frames.push((t, b.to_vec())), |b| crops.extend_from_slice(b)).unwrap();
    assert!(once.ok && !once.lost && once.errors == 0);
    // the flashing screen's frames, as flash.rs read them alone
    let sync = raea_io_ffmpeg::passthrough();
    let graph = format!("showinfo,split=2[area][point];[area]scale={w}:{h}:flags=area[a];[point]scale={w}:{h}:flags=neighbor[b];[a][b]hstack=inputs=2,format=rgb24");
    let (alone, log) = ffmpeg(&["-loglevel", "level+info", "-i", &file, "-map", "0:v:0", "-vf", &graph, sync[0], sync[1], "-f", "rawvideo", "-"]);
    assert_eq!(frames.len(), alone.len() / (w * 2 * h * 3));
    assert_eq!(frames.iter().flat_map(|f| f.1.iter().copied()).collect::<Vec<u8>>(), alone);
    let base = log.lines().find_map(raea_app_check_read::showinfo_base);
    let times: Vec<f64> = log.lines().filter(|l| l.trim_start().starts_with("[Parsed_showinfo")).filter_map(|l| raea_app_check_read::showinfo_frame(l, base)).map(|f| f.1).collect();
    assert_eq!(frames.iter().map(|f| f.0).collect::<Vec<f64>>(), times);
    // each frame's brightness, as defects.rs read it
    let (grey, _) = ffmpeg(&["-v", "error", "-i", &file, "-map", "0:v:0", "-vf", "scale=32:18:flags=area,format=gray", "-f", "rawvideo", "-"]);
    assert_eq!(once.lumas, raea_app_check_read::once::lumas_of(&grey));
    // blotching's crops, as picture.rs read them
    let (alone, _) = ffmpeg(&["-v", "error", "-i", &file, "-vf", &format!("fps=0.5,crop={cw}:{ch}:0:0,format=gray"), "-f", "rawvideo", "-"]);
    assert_eq!(crops, alone);
    // and freezedetect's freeze, as sound.rs read it
    let (_, log) = ffmpeg(&["-i", &file, "-an", "-vf", "freezedetect=n=-60dB:d=3", "-f", "null", "-"]);
    let starts = |s: &str| s.lines().filter_map(|l| l.split("freeze_start: ").nth(1).map(|x| x.trim().to_string())).collect::<Vec<_>>();
    assert_eq!(starts(&once.freezes), starts(&log));
    assert!(!starts(&log).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
