use super::*;
use raea_app_check_flash::FlashScreen;

/// The width each frame is decoded at for the flashing screen (raea-app-check-flash: its rule and its windows), twice over:
/// the area sample and the point sample side by side
const FLASH_W: usize = 192;

pub(crate) use crate::read::{log_level, showinfo_base, showinfo_frame};

/// The flashing screen's frame height for the picture's shape
fn flash_h(probe: &Probe) -> usize {
    (((FLASH_W as f64 * probe.h / probe.w.max(1.0)) / 2.0).round() as usize * 2).max(6)
}

/// Freezes, flashing, flash frames and blotching, in that order, from one read of the picture (raea-app-check-read
/// once.rs: one decode where there were four); each reads on its own where one read can't be had
pub(crate) fn picture_rules(video: &str, probe: &Probe, taste: Option<&Taste>, issues: &mut Vec<Issue>, passed: &mut Vec<String>) -> R<()> {
    if !crate::read::once::can() {
        freezes(video, probe, taste, issues, passed)?;
        flashing(video, probe, issues, passed)?;
        flash_frame_rule(video, issues, passed)?;
        return blotching(video, probe, issues, passed);
    }
    let (w, h) = (FLASH_W, flash_h(probe));
    let kept = x264_kept(video);
    let (cw, ch) = (probe.w as usize / 8 * 8, probe.h as usize / 8 * 8);
    let mut screen = FlashScreen::new(w, h);
    let mut scores = Vec::new();
    let once = crate::read::once::picture(video, (w, h), kept.is_none().then_some((cw, ch)), taste.map_or(3.0, |t| t.value), |t, buf| screen.push(buf, t), scorer(&mut scores, cw, ch))?;
    freezes_from(&once.freezes, probe, taste, issues, passed);
    if once.lost {
        return Err(format!("{video}: flashing can't be screened, frame {} has no timestamp of its own", once.frames));
    }
    if !once.ok || once.frames == 0 {
        return Err(format!("{video}: flashing can't be screened, FFmpeg decoded {} frames ({})", once.frames, once.first));
    }
    screened(screen, once.errors, &once.first, issues, passed);
    flash_frame_from(&once.lumas, ffmpeg_rate(video)?, issues, passed);
    match kept {
        Some(crf) => passed.push(format!("blotching: encoded by x264 at crf {crf}, which keeps the darks")),
        None => blotching_from(video, probe, &scores, issues, passed),
    }
    Ok(())
}

pub(crate) fn flashing(video: &str, probe: &Probe, issues: &mut Vec<Issue>, passed: &mut Vec<String>) -> R<()> {
    let (w, h) = (FLASH_W, flash_h(probe));
    if crate::read::native() {
        let mut screen = FlashScreen::new(w, h);
        if crate::read::flash_frames(video, w, h, |t, buf| screen.push(buf, t))? == 0 {
            return Err(format!("{video}: flashing can't be screened, no frame decoded"));
        }
        screened(screen, 0, "", issues, passed);
        return Ok(());
    }
    let graph = format!("showinfo,split=2[area][point];[area]scale={w}:{h}:flags=area[a];[point]scale={w}:{h}:flags=neighbor[b];[a][b]hstack=inputs=2,format=rgb24");
    let sync = raea_io_ffmpeg::passthrough();
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-nostdin", "-loglevel", "level+info", "-i", video, "-map", "0:v:0", "-vf", &graph, sync[0], sync[1], "-f", "rawvideo", "-"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let mut out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let (times, clock) = std::sync::mpsc::channel();
    // read apart from the frames, so that neither pipe can fill while the other is being waited on
    let log = std::thread::spawn(move || {
        let mut err = std::io::BufReader::new(err);
        let (mut line, mut base, mut errors, mut first) = (Vec::new(), None, 0usize, String::new());
        while std::io::BufRead::read_until(&mut err, b'\n', &mut line).is_ok_and(|n| n > 0) {
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
            }
            line.clear();
        }
        (errors, first)
    });
    let mut screen = FlashScreen::new(w, h);
    let mut buf = vec![0u8; w * 2 * h * 3];
    let mut frames = 0;
    let mut lost = false;
    while std::io::Read::read_exact(&mut out, &mut buf).is_ok() {
        // the frame's own line is written before its pixels, so one that doesn't come isn't going to
        match clock.recv_timeout(std::time::Duration::from_secs(60)) {
            Ok((n, t)) if n == frames => screen.push(&buf, t),
            _ => {
                lost = true;
                let _ = child.kill();
                break;
            }
        }
        frames += 1;
    }
    drop(out);
    let status = child.wait().map_err(|e| format!("ffmpeg: {e}"))?;
    let (errors, first) = log.join().unwrap_or_default();
    if lost {
        return Err(format!("{video}: flashing can't be screened, frame {frames} has no timestamp of its own"));
    }
    if !status.success() || frames == 0 {
        return Err(format!("{video}: flashing can't be screened, FFmpeg decoded {frames} frames ({first})"));
    }
    screened(screen, errors, &first, issues, passed);
    Ok(())
}

/// What the screen found, after `errors` errors decoding the picture (`first` the first of them)
fn screened(screen: FlashScreen, errors: usize, first: &str, issues: &mut Vec<Issue>, passed: &mut Vec<String>) {
    if errors > 0 {
        issues.push(Issue {
            rule: "decode",
            p: 1,
            at: None,
            says: format!("FFmpeg hit {errors} errors decoding the picture, so it may be damaged and flashing was only screened on what decoded: {first}"),
        });
    }
    match screen.finish() {
        Some((swings, at, kind)) => issues.push(Issue {
            rule: "flash",
            p: 1,
            at: Some(at),
            says: format!("{swings} opposing {kind} changes in one second, past WCAG 2.3.1's limit of 3 flashes (6 changes); flashing like this can cause seizures: slow it, shrink it or lower its contrast"),
        }),
        None if errors == 0 => passed.push("flashing: no more than 3 flashes a second found (a screen, not a certification)".to_string()),
        None => {}
    }
}
