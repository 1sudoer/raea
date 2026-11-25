//! The picture read once for four of raea check's rules (11-19: flashing, freezes, flash frames and blotching each
//! decoded the whole file, 3.7 s apiece on a 56 s export, and decoding is the machine's whole CPU, so reading them at
//! the same time saved nothing). One FFmpeg decodes it and splits the frames into the four filter chains the rules
//! were tuned on, each to its own output with its own options as when it ran alone, so each rule reads the same bytes:
//!
//! - the flashing screen's area- and point-sampled frames (showinfo, rgb24, passthrough) to stdout, their times on stderr;
//! - freezedetect into a null output, its findings on stderr;
//! - each frame's 32x18 grey (the flash-frame rule) to a named pipe;
//! - every 2 s the grey crop blotching scores, to another.
//!
//! Unix only (named pipes); elsewhere, and where FFmpeg isn't installed, the rules read on their own as before.
#[cfg(unix)]
use std::io::{BufRead, Read};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Stdio};

use crate::R;

/// What the one read gave the rules beside the frames it handed on
pub struct Once {
    /// the flashing screen's frames read, and whether one came without its time (the read was stopped)
    pub frames: usize,
    pub lost: bool,
    /// FFmpeg ran to its end
    pub ok: bool,
    /// errors decoding the picture, and the first of them
    pub errors: usize,
    pub first: String,
    /// freezedetect's lines
    pub freezes: String,
    /// each frame's mean brightness, 0 to 1 (defects.rs lumas)
    pub lumas: Vec<f32>,
}

/// A frame's number and time from showinfo's log, exact: its `pts` in ticks of the time base. `pts_time` is rounded to six
/// significant digits, which is enough to push a legal third flash across the one-second line.
pub fn showinfo_frame(line: &str, base: Option<(i64, i64)>) -> Option<(usize, f64)> {
    let rest = line.split(" n:").nth(1)?;
    let n = rest.split_whitespace().next()?.parse().ok()?;
    let pts: i64 = rest.split("pts:").nth(1)?.split_whitespace().next()?.parse().ok()?;
    let (num, den) = base?;
    Some((n, pts as f64 * num as f64 / den as f64))
}

pub fn showinfo_base(line: &str) -> Option<(i64, i64)> {
    let (num, den) = line.split("config in time_base:").nth(1)?.split(',').next()?.trim().split_once('/')?;
    Some((num.parse().ok()?, den.parse().ok()?))
}

/// A line's severity under `-loglevel level+`: the first bracket after any `[context @ address]`. Searching the whole line
/// would take a file's own metadata for an error when its title happens to say one.
pub fn log_level(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if rest.starts_with('[') && rest[..rest.find(']')?].contains(" @ ") {
        rest = rest[rest.find(']')? + 1..].trim_start();
    }
    let rest = rest.strip_prefix('[')?;
    Some(&rest[..rest.find(']')?])
}

/// Each 32x18 grey frame's mean brightness, 0 to 1 (`scale=32:18:flags=area,format=gray`, defects.rs lumas)
pub fn lumas_of(grey: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(grey.len() / (32 * 18));
    for f in grey.as_chunks::<{ 32 * 18 }>().0 {
        let mut sum = 0f32;
        for &p in f {
            sum += p as f32;
        }
        out.push(sum / (32.0 * 18.0 * 255.0));
    }
    out
}

/// Whether the four rules can read in one: FFmpeg is the reader, and the system has named pipes
pub fn can() -> bool {
    cfg!(unix) && !crate::native()
}

/// The one read: the flashing screen's frames (`flash`, w x h each side) to `screen` with their times, the grey crop
/// (`crop`, w x h; None when blotching doesn't read) every 2 s to `crops`, freezedetect at `still` seconds
#[cfg(not(unix))]
pub fn picture(_: &str, _: (usize, usize), _: Option<(usize, usize)>, _: f64, _: impl FnMut(f64, &[u8]) + Send, _: impl FnMut(&[u8]) + Send) -> R<Once> {
    Err("one read needs named pipes".into())
}

#[cfg(unix)]
pub fn picture(video: &str, flash: (usize, usize), crop: Option<(usize, usize)>, still: f64, mut screen: impl FnMut(f64, &[u8]) + Send, mut crops: impl FnMut(&[u8]) + Send) -> R<Once> {
    let dir = std::env::temp_dir().join(format!("raea-once-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (lumas_at, crops_at) = (dir.join("lumas"), dir.join("crops"));
    let read = (|| {
        fifo(&lumas_at)?;
        if crop.is_some() {
            fifo(&crops_at)?;
        }
        let (w, h) = flash;
        let mut graph = format!(
            "[0:v:0]split={}[f][z][l]{};[f]showinfo,split=2[area][point];[area]scale={w}:{h}:flags=area[a];[point]scale={w}:{h}:flags=neighbor[p];[a][p]hstack=inputs=2,format=rgb24[fo];[z]freezedetect=n=-60dB:d={still}[zo];[l]scale=32:18:flags=area,format=gray[lo]",
            if crop.is_some() { 4 } else { 3 },
            if crop.is_some() { "[b]" } else { "" }
        );
        if let Some((cw, ch)) = crop {
            graph.push_str(&format!(";[b]fps=0.5,crop={cw}:{ch}:0:0,format=gray[bo]"));
        }
        let sync = raea_io_ffmpeg::passthrough();
        let mut args: Vec<String> = ["-hide_banner", "-nostats", "-nostdin", "-y", "-loglevel", "level+info", "-i", video, "-filter_complex", &graph].map(String::from).to_vec();
        args.extend(["-map", "[fo]", sync[0], sync[1], "-f", "rawvideo", "pipe:1", "-map", "[zo]", "-f", "null", "-", "-map", "[lo]", "-f", "rawvideo"].map(String::from));
        args.push(lumas_at.display().to_string());
        if crop.is_some() {
            args.extend(["-map", "[bo]", "-f", "rawvideo"].map(String::from));
            args.push(crops_at.display().to_string());
        }
        let mut child = Command::new("ffmpeg").args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("ffmpeg: {e}"))?;
        let (mut out, err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
        let (times, clock) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            // each output read apart from the others, so that none can fill while another is being waited on
            let log = s.spawn(move || {
                let (mut err, mut line) = (std::io::BufReader::new(err), Vec::new());
                let (mut base, mut errors, mut first, mut freezes) = (None, 0usize, String::new(), String::new());
                while err.read_until(b'\n', &mut line).is_ok_and(|n| n > 0) {
                    let text = String::from_utf8_lossy(&line);
                    if matches!(log_level(&text), Some("error" | "fatal" | "panic")) {
                        errors += 1;
                        if first.is_empty() {
                            first = text.trim().to_string();
                        }
                    } else if text.trim_start().starts_with("[Parsed_showinfo") {
                        base = showinfo_base(&text).or(base);
                        if let Some(frame) = showinfo_frame(&text, base) {
                            let _ = times.send(frame);
                        }
                    } else if text.contains("freezedetect") {
                        freezes.push_str(&text);
                    }
                    line.clear();
                }
                (errors, first, freezes)
            });
            let lumas = s.spawn(|| {
                let mut b = Vec::new();
                let _ = std::fs::File::open(&lumas_at).and_then(|mut f| f.read_to_end(&mut b));
                lumas_of(&b)
            });
            let crop_reader = crop.map(|(cw, ch)| {
                let at = &crops_at;
                let crops = &mut crops;
                s.spawn(move || {
                    if let Ok(mut f) = std::fs::File::open(at) {
                        let mut buf = vec![0u8; cw * ch];
                        while f.read_exact(&mut buf).is_ok() {
                            crops(&buf);
                        }
                    }
                })
            });
            let mut buf = vec![0u8; w * 2 * h * 3];
            let (mut frames, mut lost) = (0usize, false);
            while out.read_exact(&mut buf).is_ok() {
                // the frame's own line is written before its pixels, so one that doesn't come isn't going to
                match clock.recv_timeout(std::time::Duration::from_secs(60)) {
                    Ok((n, t)) if n == frames => screen(t, &buf),
                    _ => {
                        lost = true;
                        let _ = child.kill();
                        break;
                    }
                }
                frames += 1;
            }
            drop(out);
            let ok = child.wait().is_ok_and(|s| s.success());
            // a pipe FFmpeg never opened (it stopped first) would hold its reader in open(): opening it to write lets go
            for p in [&lumas_at, &crops_at] {
                let _ = unblock(p);
            }
            let (errors, first, freezes) = log.join().unwrap_or_default();
            let lumas = lumas.join().unwrap_or_default();
            if let Some(c) = crop_reader {
                let _ = c.join();
            }
            Ok(Once { frames, lost, ok, errors, first, freezes, lumas })
        })
    })();
    let _ = std::fs::remove_dir_all(&dir);
    read
}

#[cfg(unix)]
fn fifo(p: &Path) -> R<()> {
    let c = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    // SAFETY: a NUL-terminated path, and mkfifo reads nothing else
    if unsafe { libc::mkfifo(c.as_ptr(), 0o600) } != 0 {
        return Err(format!("{}: {}", p.display(), std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(unix)]
fn unblock(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(p).map(drop)
}
