//! The render's manifest read as what an export places (docs/layers.feature LA4a, docs/capcut.feature CC2): each run of
//! frames showing one file, each sound clip, the runs of frames a layer draws, and where a file the manifest names is.
//! Pure: the export (lib.rs) renders and probes around it.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use crate::model::{Key, key_at};

/// One file shown over a run of frames, as the manifest's frames say: its first source time and rate, its box, what of
/// it a box it's in leaves showing (its clip), its fit and object-position at its first frame, its place in the page's
/// order (lower draws first), and its keys when it moves, is cut differently or fades.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub src: String,
    pub key: String,
    pub start: i64,
    pub end: i64,
    pub at: f64,
    pub last_at: f64,
    pub rate: Option<f64>,
    pub bx: [f64; 4],
    pub clip: Option<[f64; 4]>,
    pub fit: String,
    /// object-position when not centred: [x px, x share of the free space, y px, y share]
    pub position: Option<[f64; 4]>,
    pub opacity: f64,
    pub z: usize,
    /// its box, clip, object-position and opacity where they change, frames from the timeline's start: each frame that
    /// differs from the one before and each that differs from the one after (where a hold ends), its first and its
    /// last, so a straight line between two keys is what each frame between them shows; none when nothing changes
    pub keys: Vec<Key>,
}

/// Whether two keys show the footage alike: their boxes and clips to a twentieth of a pixel, their object-positions'
/// px to the same and their shares of the free space to a hundred-thousandth (a 4K picture's free space is a few
/// thousand px, a share's step a frame of a slow pan a few hundredths), and their opacity to half a percent.
fn same(a: &Key, b: &Key) -> bool {
    let (p, q) = (placed(&a.position), placed(&b.position));
    let most = [apart(&a.r#box, &b.r#box), apart(&shown(&a.r#box, &a.clip), &shown(&b.r#box, &b.clip)), (p[0] - q[0]).abs(), (p[2] - q[2]).abs()];
    most.into_iter().fold(0.0, f64::max) <= 0.05 && (p[1] - q[1]).abs().max((p[3] - q[3]).abs()) <= 1e-5 && (a.opacity - b.opacity).abs() <= 0.005
}

/// How far apart two fours are: their farthest pair.
pub fn apart(p: &[f64; 4], q: &[f64; 4]) -> f64 {
    let mut most = 0.0f64;
    for (x, y) in p.iter().zip(q) {
        most = most.max((x - y).abs());
    }
    most
}

impl Run {
    /// Its box, clip, object-position and opacity at `frame` (from the timeline's start): its keys' straight line
    /// there, or its first frame's when it has none.
    pub fn at(&self, frame: i64) -> Key {
        key_at(&self.keys, frame).unwrap_or(Key { frame, r#box: self.bx, clip: self.clip, position: self.position, opacity: self.opacity })
    }
}

/// Only the keys where something changes: the first, the last, and those on either side of a change (none when nothing
/// does).
fn changes(all: Vec<Key>) -> Vec<Key> {
    if all.windows(2).all(|p| same(&p[0], &p[1])) {
        return Vec::new();
    }
    let n = all.len();
    (0..n).filter(|&i| i == 0 || i + 1 == n || !same(&all[i], &all[i - 1]) || !same(&all[i], &all[i + 1])).map(|i| all[i].clone()).collect()
}

/// One sound clip of the manifest: its file, frames, source time, gain, rate and fades.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub src: String,
    pub from: i64,
    pub to: i64,
    pub at: f64,
    pub gain_db: f64,
    pub rate: f64,
    pub fade_in: f64,
    pub fade_out: f64,
}

/// The footage's runs, frame by frame: a run goes on while the same element shows the same file with its source time
/// advancing at one rate; a jump in source time (a cut within one take) starts another.
pub fn runs(manifest: &Value, fps: f64) -> Vec<Run> {
    let mut done: Vec<Run> = Vec::new();
    let mut open: Vec<Run> = Vec::new();
    let frames = manifest["frames"].as_array().cloned().unwrap_or_default();
    for (i, fr) in frames.iter().enumerate() {
        let f = fr["frame"].as_i64().unwrap_or(i as i64);
        let mut next = Vec::new();
        for (z, v) in fr["videos"].as_array().into_iter().flatten().enumerate() {
            let src = v["src"].as_str().unwrap_or_default().to_string();
            let id = format!("{src}|{}|{}|{}", v["loc"], v["use"], v["index"]);
            let at = v["at"].as_f64().unwrap_or(0.0);
            let bx: [f64; 4] = std::array::from_fn(|k| v["box"][k].as_f64().unwrap_or(0.0));
            let opacity = v["opacity"].as_f64().unwrap_or(1.0);
            let four = |field: &str| -> Option<[f64; 4]> { v[field].is_array().then(|| std::array::from_fn(|k| v[field][k].as_f64().unwrap_or(0.0))) };
            let (clip, position) = (four("clip"), four("position"));
            let key = Key { frame: f, r#box: bx, clip, position, opacity };
            let continues = |r: &Run| {
                let step = at - r.last_at;
                r.key == id && r.end == f && step > 0.0 && match r.rate {
                    Some(rate) => (step - rate / fps).abs() <= 0.6 / fps,
                    None => step * fps <= 8.0,
                }
            };
            if let Some(k) = open.iter().position(continues) {
                let mut r = open.remove(k);
                r.rate.get_or_insert((at - r.last_at) * fps);
                r.keys.push(key);
                (r.end, r.last_at) = (f + 1, at);
                next.push(r);
            } else {
                let fit = v["fit"].as_str().unwrap_or("fill").to_string();
                next.push(Run { src, key: id, start: f, end: f + 1, at, last_at: at, rate: None, bx, clip, fit, position, opacity, z, keys: vec![key] });
            }
        }
        done.append(&mut open);
        open = next;
    }
    done.append(&mut open);
    // the rate over the whole run (source times are whole milliseconds, so one step's can be 1% off), to hundredths
    for r in &mut done {
        r.keys = changes(std::mem::take(&mut r.keys));
        if r.end - r.start > 1 {
            r.rate = Some(((r.last_at - r.at) * fps / (r.end - r.start - 1) as f64 * 100.0).round() / 100.0);
        }
    }
    done.sort_by_key(|r| (r.start, r.z));
    done
}

/// The manifest's sound clips.
pub fn sounds(manifest: &Value) -> Vec<Sound> {
    let s = &manifest["sounds"]["clips"];
    s.as_array()
        .into_iter()
        .flatten()
        .map(|c| Sound {
            src: c["src"].as_str().unwrap_or_default().to_string(),
            from: c["from"].as_i64().unwrap_or(0),
            to: c["to"].as_i64().unwrap_or(0),
            at: c["at"].as_f64().unwrap_or(0.0),
            gain_db: c["gainDb"].as_f64().unwrap_or(0.0),
            rate: c["rate"].as_f64().unwrap_or(1.0),
            fade_in: c["fadeIn"].as_f64().unwrap_or(0.0),
            fade_out: c["fadeOut"].as_f64().unwrap_or(0.0),
        })
        .filter(|c| c.to > c.from)
        .collect()
}

/// How many of a sound's frames it's heard for: its span on the timeline, or fewer when its file ends first (raea's
/// <Audio> stops at its file's end, so a one-second tick in a longer <Sequence> is a one-second clip, not one that
/// runs past its media in an editor).
pub fn heard_frames(s: &Sound, seconds: f64, fps: f64) -> i64 {
    let left = ((seconds - s.at) / s.rate.max(0.01) * fps + 1e-6).floor() as i64;
    (s.to - s.from).min(left.max(0))
}

/// Runs of frames that draw something.
pub fn spans(drawn: &[bool]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut f = 0;
    while f < drawn.len() {
        if !drawn[f] {
            f += 1;
            continue;
        }
        let a = f;
        while f < drawn.len() && drawn[f] {
            f += 1;
        }
        out.push((a as i64, f as i64));
    }
    out
}

/// The keyframes a straight line between their neighbours can't stand for: each (when, values) whose values lie off
/// the line from the last kept to the next by more than `tol` (each value's own), kept with the first and the last. A
/// steady move or push-in is its two ends; a raea easing keeps a key a frame.
pub fn thin<const N: usize>(all: &[(i64, [f64; N])], tol: &[f64; N]) -> Vec<(i64, [f64; N])> {
    let mut kept = vec![all[0]];
    let mut from = 0;
    for to in 2..all.len() {
        let (a, b) = (&all[from], &all[to]);
        let off = (from + 1..to).any(|i| {
            let t = (all[i].0 - a.0) as f64 / (b.0 - a.0).max(1) as f64;
            (0..N).any(|k| (a.1[k] + (b.1[k] - a.1[k]) * t - all[i].1[k]).abs() > tol[k])
        });
        if off {
            from = to - 1;
            kept.push(all[from]);
        }
    }
    if all.len() > 1 {
        kept.push(all[all.len() - 1]);
    }
    kept
}

/// What of a box shows: its clip, else all of it.
pub fn shown(bx: &[f64; 4], clip: &Option<[f64; 4]>) -> [f64; 4] {
    clip.unwrap_or(*bx)
}

/// An object-position as [x px, x share, y px, y share]: its own, or centred (CSS's default) when none.
pub fn placed(position: &Option<[f64; 4]>) -> [f64; 4] {
    position.unwrap_or([0.0, 0.5, 0.0, 0.5])
}

/// A picture as raea draws it: where the whole of it is on the canvas, and what of it is kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    /// the whole picture, [x, y, width, height] on the canvas (it can run past its box and the canvas)
    pub drawn: [f64; 4],
    /// what of it is kept, as shares of it: [left, top, right, bottom] edges, [0, 0, 1, 1] all of it
    pub kept: [f64; 4],
}

/// Where a picture of `mw` x `mh` (upright) is drawn in `bx` on a `w` x `h` canvas, as blitz-paint draws it: sized by
/// CSS's object-fit `fit`, placed by `position` (object-position; centred when none), and cut where its box or `clip`
/// (what a box it's in leaves showing) cut it inside the canvas. The canvas's own edges cut nothing: an editor shows no
/// more than its frame anyway, and someone reframing the clip there finds the rest of the picture, not a crop. A note
/// when an editor can't show it so (object-fit: fill stretches it; an editor fits it inside its box instead).
pub fn picture(bx: &[f64; 4], clip: &Option<[f64; 4]>, fit: &str, position: &Option<[f64; 4]>, (mw, mh): (f64, f64), (w, h): (f64, f64)) -> (Picture, Option<String>) {
    let [x, y, bw, bh] = *bx;
    let (sx, sy) = (bw / mw, bh / mh);
    let mut note = None;
    let s = match fit {
        "cover" => sx.max(sy),
        "none" => 1.0,
        "scale-down" => sx.min(sy).min(1.0),
        "contain" => sx.min(sy),
        _ => {
            if (sx - sy).abs() > 0.01 * sx.max(sy) {
                note = Some(format!("stretched to its {bw:.0}x{bh:.0} box (object-fit: fill); fitted inside it"));
            }
            sx.min(sy)
        }
    };
    let (dw, dh) = (mw * s, mh * s);
    let [xp, xs, yp, ys] = placed(position);
    let (dx, dy) = (x + xp + xs * (bw - dw), y + yp + ys * (bh - dh));
    // the edges that cut it: its box's and its clip's, those inside the canvas
    let [kx, ky, kw, kh] = shown(bx, clip);
    let near = |e: f64| e > 0.5;
    let far = |e: f64, side: f64| e < side - 0.5;
    let left = [x, kx].into_iter().filter(|&e| near(e)).fold(f64::NEG_INFINITY, f64::max);
    let top = [y, ky].into_iter().filter(|&e| near(e)).fold(f64::NEG_INFINITY, f64::max);
    let right = [x + bw, kx + kw].into_iter().filter(|&e| far(e, w)).fold(f64::INFINITY, f64::min);
    let bottom = [y + bh, ky + kh].into_iter().filter(|&e| far(e, h)).fold(f64::INFINITY, f64::min);
    // those edges on the picture (none past it), as shares of it
    let (l, r) = (left.max(dx).min(dx + dw), right.min(dx + dw).max(dx));
    let (t, b) = (top.max(dy).min(dy + dh), bottom.min(dy + dh).max(dy));
    let kept = [(l - dx) / dw, (t - dy) / dh, (r - dx) / dw, (b - dy) / dh];
    (Picture { drawn: [dx, dy, dw, dh], kept }, note)
}

/// Whether a box covers the whole canvas.
pub fn full(bx: &[f64; 4], w: f64, h: f64) -> bool {
    bx[0] <= 1.0 && bx[1] <= 1.0 && bx[0] + bx[2] >= w - 1.0 && bx[1] + bx[3] >= h - 1.0
}

/// A file the manifest names, as a path, found as the renderer finds it: a staticFile path (/media/x) in the edit's
/// media/, then the project's media/ and assets/; else absolute; else relative to the composition's folder.
pub fn resolve(dir: &Path, src: &str) -> PathBuf {
    let p = raea_core_scene::media_path(dir, src);
    dunce::canonicalize(&p).unwrap_or(p)
}
