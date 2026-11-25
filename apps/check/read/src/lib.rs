//! What `raea check` reads of a video: through FFmpeg where it's on the PATH (the reference its rules were tuned on),
//! else through raea's own decoders (raea-app-media), so `raea check` works wherever `raea media probe` and `raea media
//! loudness` do, with nothing to install. 0.1.7 refused any video without FFmpeg, which Windows never has (Shane 11-18:
//! "can we make it so ffmpeg does work the way we need it?"). RAEA_CHECK_NATIVE=1 reads natively with FFmpeg there too;
//! tests/parity.rs holds the two to the same answers. Each reader names the FFmpeg command it stands in for.
pub mod once;
pub use once::{log_level, showinfo_base, showinfo_frame};
use raea_io_media::VideoDecoder;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub type R<T> = Result<T, String>;

/// Whether to read natively: FFmpeg's ffmpeg or ffprobe isn't on the PATH, or RAEA_CHECK_NATIVE is on
pub fn native() -> bool {
    raea_io_home::env_on("RAEA_CHECK_NATIVE") || !(on_path("ffmpeg") && on_path("ffprobe"))
}

fn on_path(tool: &str) -> bool {
    let exe = if cfg!(windows) { format!("{tool}.exe") } else { tool.to_string() };
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(&exe).is_file()))
}

fn open(video: &str) -> R<(Box<dyn VideoDecoder>, raea_io_media::Info)> {
    let d = raea_app_media::open_video(Path::new(video))?;
    let info = d.info().clone();
    Ok((d, info))
}

/// A luma byte as FFmpeg's format=gray gives it from a limited-range picture: full range, 16-235 spread to 0-255 and
/// rounded (FFmpeg 9: 16 is 0, 126 is 128, 235 is 255)
fn gray(y: u8) -> u8 {
    (((y.clamp(16, 235) as u32 - 16) * 255 + 109) / 219) as u8
}

/// What `raea check` needs to know of a video before it reads it
pub struct Probe {
    pub w: f64,
    pub h: f64,
    pub duration: f64,
    /// when the picture ends, on the clock FFmpeg's filters read: it starts at the file's first stream, sound can run on
    /// past the picture, and the container's duration counts that
    pub picture: f64,
    pub audio: bool,
    /// the picture's frames a second (0 when it doesn't say)
    pub fps: f64,
}

/// The picture's size, the file's duration, where the picture ends and whether there's sound: ffprobe's stream and
/// format entries, or raea's own probe without FFmpeg
pub fn probe(video: &str) -> R<Probe> {
    if native() {
        let p = raea_app_media::probe_of(Path::new(video))?;
        let v = p.video.as_ref().ok_or("no video stream")?;
        let picture = p.track.as_ref().map_or(v.duration, |t| t.duration);
        return Ok(Probe { w: v.width as f64, h: v.height as f64, duration: p.duration.max(picture), picture, audio: p.audio.is_some(), fps: v.fps });
    }
    let entries = "stream=codec_type,width,height,r_frame_rate,duration,start_time:format=duration,start_time";
    let out = Command::new("ffprobe").args(["-v", "error", "-show_entries", entries, "-of", "json", video]).output().map_err(|e| format!("ffprobe: {e}"))?;
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe: {e}"))?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let vid = streams.iter().find(|s| s["codec_type"] == "video").ok_or("no video stream")?;
    let seconds = |field: &Value| -> Option<f64> { field.as_str()?.parse().ok() };
    let duration = seconds(&v["format"]["duration"]).unwrap_or(0.0);
    let origin = seconds(&v["format"]["start_time"]).unwrap_or(0.0);
    let picture = match seconds(&vid["duration"]) {
        Some(length) => seconds(&vid["start_time"]).unwrap_or(origin) - origin + length,
        None => last_packet_end(video).map_or(duration, |end| end - origin),
    };
    let audio = streams.iter().any(|s| s["codec_type"] == "audio");
    let fps = vid["r_frame_rate"].as_str().and_then(|r| {
        let (n, d) = r.split_once('/').unwrap_or((r, "1"));
        Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok().filter(|d| *d > 0.0)?)
    });
    Ok(Probe { w: vid["width"].as_f64().unwrap_or(1920.0), h: vid["height"].as_f64().unwrap_or(1080.0), duration, picture, audio, fps: fps.unwrap_or(0.0) })
}

/// Where the picture's last packet ends, for a container that doesn't state the video stream's own length (Matroska)
fn last_packet_end(video: &str) -> Option<f64> {
    let out = Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0", "-show_entries", "packet=pts_time,duration_time", "-of", "csv=p=0", video]).output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut parts = l.split(',');
            let t: f64 = parts.next()?.trim().parse().ok()?;
            Some(t + parts.next().and_then(|d| d.trim().parse::<f64>().ok()).unwrap_or(0.0))
        })
        .max_by(f64::total_cmp)
}

/// `ffmpeg -i V -vf scale=32:18:flags=area,format=gray -f rawvideo -` averaged a frame, and the rate `ffmpeg -i` says
/// (defects.rs lumas): each frame's mean light, 0 to 1, and the frames a second
pub fn lumas(video: &str) -> R<(Vec<f32>, f64)> {
    let (mut d, info) = open(video)?;
    let mut out = Vec::new();
    for (_, at) in raea_app_media::frames_of(Path::new(video), &info, None, None) {
        let (y, ..) = raea_app_media::yuv(d.frame_nv12_at(at)?);
        out.push(y.iter().map(|&p| gray(p) as f32).sum::<f32>() / (y.len().max(1) as f32 * 255.0));
    }
    Ok((out, raea_app_media::standard_fps(info.fps)))
}

/// `ffmpeg -i V -vf fps=0.5,crop=W:H:0:0,format=gray -f rawvideo -` (picture.rs blotching): every 2 s, the picture's
/// top-left W x H in grey
pub fn crops(video: &str, w: usize, h: usize, mut each: impl FnMut(&[u8])) -> R<()> {
    let (mut d, info) = open(video)?;
    let frames = raea_app_media::frames_of(Path::new(video), &info, None, None);
    for (tick, _) in raea_app_media::times(raea_app_media::video_span(Path::new(video), &info), None, None, 0.5) {
        // the last frame before the tick's middle, as the fps filter picks it (by the frames' own times: a read at the
        // middle less a microsecond came back a frame late)
        let Some(&(_, at)) = frames.iter().take_while(|(pts, _)| *pts < tick + 1.0).last() else { continue };
        let f = d.frame_nv12_at(at)?;
        let fw = f.width as usize;
        let (y, ..) = raea_app_media::yuv(f);
        let crop: Vec<u8> = (0..h.min(y.len() / fw.max(1))).flat_map(|r| y[r * fw..r * fw + w.min(fw)].iter().map(|&p| gray(p))).collect();
        each(&crop);
    }
    Ok(())
}

/// flash.rs's graph (showinfo, the picture at W x H area-sampled beside it point-sampled, rgb24): every frame with
/// its time, until `each` says stop. How many frames were read.
pub fn flash_frames(video: &str, w: usize, h: usize, mut each: impl FnMut(f64, &[u8])) -> R<usize> {
    let (mut d, info) = open(video)?;
    let mut buf = vec![0u8; w * 2 * h * 3];
    let mut n = 0;
    for (pts, at) in raea_app_media::frames_of(Path::new(video), &info, None, None) {
        let f = d.frame_at(at)?;
        let (sw, sh) = (f.width as usize, f.height as usize);
        for y in 0..h {
            let (y0, y1) = (y * sh / h, ((y + 1) * sh / h).max(y * sh / h + 1));
            for x in 0..w {
                let (x0, x1) = (x * sw / w, ((x + 1) * sw / w).max(x * sw / w + 1));
                let mut sum = [0u32; 3];
                for sy in y0..y1 {
                    for p in f.rgba[(sy * sw + x0) * 4..(sy * sw + x1) * 4].chunks_exact(4) {
                        (0..3).for_each(|c| sum[c] += p[c] as u32);
                    }
                }
                let count = ((y1 - y0) * (x1 - x0)) as u32;
                let near = ((y * sh + sh / 2) / h * sw + (x * sw + sw / 2) / w) * 4;
                for c in 0..3 {
                    buf[(y * w * 2 + x) * 3 + c] = ((sum[c] + count / 2) / count) as u8;
                    buf[(y * w * 2 + w + x) * 3 + c] = f.rgba[near + c];
                }
            }
        }
        each(pts, &buf);
        n += 1;
    }
    Ok(n)
}

/// `ffmpeg -i V -vn -ac 1 -ar RATE -f f32le -` (sound.rs sound): the sound folded to mono at `rate`
pub fn mono(video: &str, rate: u32) -> R<Vec<f32>> {
    if native() {
        return Ok(raea_app_media::audio_folded(Path::new(video), rate, true, None, None, false)?.channels.swap_remove(0));
    }
    ffmpeg_f32(video, 1, rate)
}

/// `ffmpeg -i V -vn -ac N -ar R -f f32le -`: the sound as N interleaved channels of floats
fn ffmpeg_f32(video: &str, channels: u32, rate: u32) -> R<Vec<f32>> {
    let out = Command::new("ffmpeg").args(["-v", "error", "-i", video, "-vn", "-ac", &channels.to_string(), "-ar", &rate.to_string(), "-f", "f32le", "-"]).output().map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg couldn't read the sound of {video}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

/// `ffmpeg -i V -vn -ac 2 -ar R -f f32le -` (defects.rs clipping): each channel at its own level. Clipping is a
/// channel's: on the mono fold (L + R) * 0.707 a centred voice reads 3 dB hotter than either side, and a master at
/// -1.6 dBTP read as full scale (11-19, the walkaround's flat tops).
pub fn channels(video: &str, rate: u32) -> R<Vec<Vec<f32>>> {
    if native() {
        return Ok(raea_app_media::audio_folded(Path::new(video), rate, false, None, None, false)?.channels);
    }
    let x = ffmpeg_f32(video, 2, rate)?;
    Ok(vec![x.iter().step_by(2).copied().collect(), x.iter().skip(1).step_by(2).copied().collect()])
}

/// `ffmpeg -i V -vn -af ebur128=metadata=1,ametadata=print:key=lavfi.r128.M:file=- -f null -` (sound.rs sound): the
/// momentary loudness (400 ms) every 100 ms, each time and its LUFS
pub fn meter(video: &str) -> R<(Vec<f64>, Vec<f64>)> {
    let a = raea_app_media::audio_folded(Path::new(video), raea_io_media::audio::RATE, false, None, None, false)?;
    let n = (a.duration() * 10.0).floor() as usize;
    Ok(((0..n).map(|i| i as f64 / 10.0).collect(), raea_io_media::audio::momentary(&a, 10.0, n)))
}

/// `ffmpeg -i V -an -vf freezedetect=n=NOISE_DB:d=MIN -f null -`'s log (sound.rs freezes reads it either way)
pub fn freezes(video: &str, noise_db: f64, min: f64) -> R<String> {
    let found = raea_app_media::freezes(Path::new(video), 10f64.powf(noise_db / 20.0), min)?;
    Ok(found.into_iter().map(|(s, end)| match end {
        Some((d, e)) => format!("freeze_start: {s}\nfreeze_duration: {d}\nfreeze_end: {e}\n"),
        None => format!("freeze_start: {s}\n"),
    }).collect())
}

/// raea_sys_measure::contrast's read (`ffmpeg -ss AT -i V -frames:v 1 -vf crop=... -pix_fmt rgb24`): a text's contrast
/// against what's behind it, None when the crop falls outside the frame
pub fn contrast(video: &str, at: f64, crop: [u32; 4], color: &str) -> Option<f64> {
    let [x, y, w, h] = [crop[0] & !1, crop[1] & !1, crop[2].max(2) & !1, crop[3].max(2) & !1].map(|v| v as usize);
    let (mut d, _) = open(video).ok()?;
    let f = d.frame_at(at).ok()?;
    let fw = f.width as usize;
    if x + w > fw || y + h > f.height as usize {
        return None;
    }
    let rgb: Vec<u8> = (y..y + h).flat_map(|r| f.rgba[(r * fw + x) * 4..(r * fw + x + w) * 4].chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect::<Vec<_>>()).collect();
    raea_sys_measure::ratio_behind(&rgb, raea_sys_measure::lum(raea_sys_measure::rgb_of(color)))
}

/// raea_sys_measure::loudness's read (`ffmpeg -i V -vn -af ebur128=peak=true -f null -`): the integrated loudness and
/// true peak, None when there's no sound
pub fn loudness(video: &str) -> Option<(f64, Option<f64>)> {
    let a = raea_app_media::audio_folded(Path::new(video), raea_io_media::audio::RATE, false, None, None, false).ok().filter(|a| a.len() > 0)?;
    Some((raea_io_media::audio::lufs(&a), Some(raea_io_media::audio::true_peak_db(&a))))
}
