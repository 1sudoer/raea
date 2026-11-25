//! Who can fix what `raea check` finds, so a round is spent only on what another render can change (11-19: two
//! Telluride projects took nine renders, five of them for findings no render could move). Three things, for any rule:
//!
//! - **Introduced or there already.** A rule an encode can cause is measured again on the composition's own frames,
//!   rendered without encoding (`unencoded`), and the export is set against them (`against`): what the picture already
//!   held is the footage's (a note), what the export adds is the encode's (the error, with a fix raea has).
//! - **Carried.** A finding an earlier export of the same composition had at the same moment (`earlier`, `carried`)
//!   says since when: the change between them didn't move it, so doing that again won't.
//! - **A source isn't a delivery.** A take or a download checked on its own (`is_source`) isn't held to a delivery's
//!   loudness target.
//!
//! None of it changes what a rule measures or where it draws the line.
use std::path::{Path, PathBuf};
use std::process::Command;

use raea_core_facts::{Finding, record};

pub type R<T> = Result<T, String>;

/// Whether a record entry is the render that wrote `out` (a path as the record names it)
fn rendered(e: &record::Entry, out: &str) -> bool {
    e.by == "raea render" && e.out.as_deref() == Some(out)
}

/// The composition a video was rendered from, and its project: the newest `raea render` writing it in the project's
/// record (DR3). None outside a project, or for a video no render there wrote (a take, a download).
pub fn composition_of(video: &Path) -> Option<(PathBuf, PathBuf)> {
    let project = record::up(video, ".raea/project")?;
    let want = record::rel(&project, video);
    let made = record::read(&project).into_iter().rev().find(|e| e.kind == "video" && rendered(e, &want))?;
    let of = project.join(&made.of);
    of.is_file().then_some((project, of))
}

/// Whether a file is one of a project's sources (a take or a download under assets/ or media/ that no render wrote):
/// it isn't a delivery, so a delivery's rules (its loudness target) don't hold it
pub fn is_source(video: &Path) -> bool {
    let Some(project) = record::up(video, ".raea/project") else { return false };
    let me = record::rel(&project, video);
    (me.starts_with("assets/") || me.starts_with("media/")) && !record::read(&project).iter().any(|e| rendered(e, &me))
}

/// Luma as a decoder of raea's export gives it to the check (BT.709, full range: FFmpeg's format=gray of the
/// encoder's limited-range picture), from 8-bit RGBA `width` wide, its top-left `w` x `h`
pub fn luma(rgba: &[u8], width: usize, w: usize, h: usize) -> Vec<u8> {
    let mut y = Vec::with_capacity(w * h);
    for r in 0..h {
        for p in rgba[r * width * 4..][..w * 4].chunks_exact(4) {
            y.push((0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32).round().clamp(0.0, 255.0) as u8);
        }
    }
    y
}

/// `frames` as `comp` draws them, never encoded, as luma `w` x `h` (top-left): `raea render COMP --at F,... -o DIR/`
/// (PNG) run by `raea` from the project, kept out of its record as the quick check's stills are
pub fn unencoded(raea: &Path, project: &Path, comp: &Path, frames: &[i64], w: usize, h: usize) -> R<Vec<Vec<u8>>> {
    let mut wanted: Vec<i64> = frames.to_vec();
    wanted.sort_unstable();
    wanted.dedup();
    let tmp = std::env::temp_dir().join(format!("raea-unencoded-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let at = wanted.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
    let read = (|| {
        let run = Command::new(raea).arg("render").arg(comp).args(["--at", &at, "-o"]).arg(format!("{}/", tmp.display())).current_dir(project).env("RAEA_RECORD", "off").output().map_err(|e| e.to_string())?;
        let mut pngs: Vec<PathBuf> = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "png")).collect();
        pngs.sort();
        if pngs.len() != wanted.len() {
            let said = String::from_utf8_lossy(&run.stderr).lines().chain(String::from_utf8_lossy(&run.stdout).lines()).find(|l| l.contains("REFUSED") || l.contains("rror")).unwrap_or("").to_string();
            return Err(format!("its render gave {} of {} frames {said}", pngs.len(), wanted.len()).trim_end().to_string());
        }
        let mut by_frame = std::collections::HashMap::new();
        for (f, p) in wanted.iter().zip(&pngs) {
            let img = image::open(p).map_err(|e| format!("{}: {e}", p.display()))?.to_rgba8();
            let (iw, ih) = (img.width() as usize, img.height() as usize);
            if iw < w || ih < h {
                return Err(format!("it draws {iw}x{ih}, smaller than the export's {w}x{h}"));
            }
            by_frame.insert(*f, luma(img.as_raw(), iw, w, h));
        }
        Ok(frames.iter().map(|f| by_frame[f].clone()).collect())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    read
}

/// The frame a check reads for each sample taken every `every` seconds from 0: the last frame before the sample's
/// middle, as ffmpeg's fps filter picks it (and raea-app-check-read's readers), held to the picture's `last` frame
pub fn sampled(samples: usize, every: f64, fps: f64, last: i64) -> Vec<i64> {
    (0..samples).map(|k| (((k as f64 + 0.5) * every * fps).ceil() as i64 - 1).clamp(0, last.max(0))).collect()
}

/// A sampled measure of the export against the same measure of the unencoded frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Against {
    /// samples over the rule's line that the export made worse by more than its noise
    pub worse: usize,
    /// samples over the line before encoding: the picture's own
    pub before: usize,
    /// samples compared
    pub of: usize,
    /// the mean of the export's measure less the unencoded one's
    pub added: f64,
}

impl Against {
    /// Whether the export added to it, beyond what a clean encode moves: at least three samples (and a tenth) worse,
    /// and more than half the noise on average
    pub fn by_encode(&self, noise: f64) -> bool {
        self.worse >= 3usize.max(self.of / 10) && self.added > noise / 2.0
    }

    /// The first sample the export made worse
    pub fn first(out: &[Option<f64>], before: &[Option<f64>], line: f64, noise: f64) -> Option<usize> {
        out.iter().zip(before).position(|(o, b)| o.unwrap_or(0.0) > line && o.unwrap_or(0.0) - b.unwrap_or(0.0) > noise)
    }
}

/// Per sample, the export's measure against the unencoded frames' (None: nothing to measure, read as 0), over a
/// rule's `line`, where a clean encode moves it at most `noise`
pub fn against(out: &[Option<f64>], before: &[Option<f64>], line: f64, noise: f64) -> Against {
    let n = out.len().min(before.len());
    let at = |s: &[Option<f64>], i: usize| s[i].unwrap_or(0.0);
    let worse = (0..n).filter(|&i| at(out, i) > line && at(out, i) - at(before, i) > noise).count();
    let was = (0..n).filter(|&i| at(before, i) > line).count();
    let added = (0..n).map(|i| at(out, i) - at(before, i)).sum::<f64>() / n.max(1) as f64;
    Against { worse, before: was, of: n, added }
}

/// The newest check, in the project's record, of an earlier export of the composition `video` was rendered from
pub fn earlier(video: &Path) -> Option<record::Entry> {
    let project = record::up(video, ".raea/project")?;
    let me = record::rel(&project, video);
    let entries = record::read(&project);
    let made = entries.iter().rposition(|e| rendered(e, &me))?;
    // the composition's exports rendered before this one; a versioned copy (NAME-v2.tsx) is the same piece
    let stem = |of: &str| piece(&Path::new(of).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let comp = stem(&entries[made].of);
    let exports: Vec<String> = entries[..made].iter().filter(|e| e.by == "raea render" && e.kind == "video" && stem(&e.of) == comp).filter_map(|e| e.out.clone()).filter(|o| *o != me).collect();
    entries.into_iter().rev().find(|e| e.by == "raea check" && exports.contains(&e.of))
}

/// A composition's name without its version: walkaround-v2 is walkaround
pub fn piece(stem: &str) -> String {
    match stem.rfind("-v") {
        Some(i) if i + 2 < stem.len() && stem[i + 2..].bytes().all(|b| b.is_ascii_digit()) => stem[..i].to_string(),
        _ => stem.to_string(),
    }
}

/// The same finding in another export: the same rule, about the same thing, at the same moment (within a second; a
/// finding about the whole piece matches one about the whole piece). Its words may differ ("worst 13%", "worst 15%").
pub fn alike(a: &Finding, b: &Finding, fps: f64) -> bool {
    let about = |f: &Finding| f.about.iter().map(|a| a.key.clone().unwrap_or_else(|| a.name.clone())).collect::<Vec<_>>();
    a.check == b.check && about(a) == about(b) && match (a.frames, b.frames) {
        (Some([x, _]), Some([y, _])) => ((x - y).abs() as f64) <= fps.max(1.0),
        (None, None) => true,
        _ => false,
    }
}

/// Each finding an earlier check had at the same moment says so, and that its fix hasn't moved it: rank and words
/// otherwise as measured
pub fn carried(now: &mut [Finding], before: &record::Entry, fps: f64) {
    for f in now.iter_mut() {
        if before.findings.iter().any(|b| alike(b, f, fps)) {
            f.says.push_str(&format!(" [also in {} ({}): what changed between them didn't move it]", before.of, before.id));
        }
    }
}

#[cfg(test)]
mod tests;
