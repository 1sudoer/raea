//! layers.json (docs/layers.feature LA4a): a composition's layers and the timeline that places them, in no editor's
//! terms. Each editor's export (CapCut's draft, the FCP7 XML) is written from it. Tracks are listed top to bottom;
//! frames are the composition's, whole; a clip's source time is in seconds.

use serde::{Deserialize, Serialize};

/// The layers.json format's version, which a reader checks before it reads the rest.
pub const VERSION: u32 = 1;

/// A composition's layers and their timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layers {
    pub version: u32,
    /// the export's name (the composition's file stem, unless --name says)
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// the composition's length in frames
    pub frames: i64,
    /// the reference render, beside layers.json: the composition as raea draws it, on no track
    pub reference: String,
    /// the captions as SubRip, beside layers.json, when there are captions (LA2b); on no track
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub srt: Option<String>,
    /// every file a clip shows or plays: the footage and sounds where they are, the layers' movies beside layers.json
    pub files: Vec<File>,
    /// top to bottom
    pub tracks: Vec<Track>,
    /// what doesn't carry into an editor, said once each (CC2e)
    pub notes: Vec<String>,
}

/// A file a clip shows or plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct File {
    /// absolute for the footage and sounds (never copied, LA5a); relative to layers.json for a layer's movie
    pub path: String,
    pub kind: FileKind,
    pub seconds: f64,
    /// a picture's upright size; 0 for a sound
    pub width: u32,
    pub height: u32,
    /// whether it has sound of its own (footage)
    pub has_audio: bool,
    /// its sound's channels and sample rate, when it has sound (an editor places each channel; Premiere a track each)
    #[serde(default = "two")]
    pub channels: u32,
    #[serde(default = "rate_48k")]
    pub sample_rate: u32,
    /// how far its pictures are turned to stand upright, as its container says (a phone's take is stored on its side):
    /// counter-clockwise degrees, 0, 90, 180 or 270; its width and height are upright already. Resolve crops such a
    /// file mirrored (xml::Editor)
    #[serde(default, skip_serializing_if = "is_zero_u")]
    pub rotation: u32,
}

fn is_zero_u(v: &u32) -> bool {
    *v == 0
}

fn two() -> u32 {
    2
}

fn rate_48k() -> u32 {
    48_000
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Video,
    Audio,
}

/// Which layer a track holds (the layers table in docs/layers.feature).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Captions,
    Graphics,
    Footage,
    /// what's drawn behind a picture in picture, between it and the footage under it (its card, LA2c)
    Backing,
    Under,
    Sound,
}

/// A track: clips of one layer that never overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub layer: Layer,
    /// what an editor calls it: "captions", "graphics", "picture in picture", "behind picture in picture", "footage",
    /// "footage 2", "footage over", "under", "sound"
    pub name: String,
    pub clips: Vec<Clip>,
}

/// A span of a file on a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    /// its own id, unique in the timeline
    pub id: String,
    /// its file, an index into `files`
    pub file: usize,
    /// its first frame on the timeline, and how many it lasts
    pub start: i64,
    pub frames: i64,
    /// the source time it starts from, in seconds
    pub source_in: f64,
    /// how fast the source plays (1 as recorded)
    pub speed: f64,
    /// footage: its box on the canvas at its first frame [x, y, width, height] and how the picture fills it (CSS's
    /// object-fit); a layer's movie fills the canvas and has none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#box: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<String>,
    /// footage: what of its box shows at its first frame, when a box it's in cuts it (an `overflow: hidden` parent):
    /// [x, y, width, height] on the canvas, inside its box; none when all of the box shows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<[f64; 4]>,
    /// footage: where the picture sits in what its box leaves free (CSS's object-position) when not centred, each axis
    /// as px plus a share of the free space: [x px, x share, y px, y share]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 4]>,
    /// footage: its opacity at its first frame
    pub opacity: f64,
    /// footage that moves, scales, is cut, pans in its box or fades over its run: its box, clip, object-position and
    /// opacity at the frames where that
    /// changes, a straight line between two keys being what each frame between them shows (the first key is its first
    /// frame, the last its last); none when it holds as its first frame shows it
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Key>,
    /// a sound: its gain in decibels and its fades in seconds
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gain_db: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade_in: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade_out: f64,
    /// footage and its own sound: the other's id, so an editor moves them together until it's told not to. Footage
    /// whose file has sound and no link plays silent: the composition doesn't play its sound there
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// Footage's box, clip, object-position and opacity at one frame of its clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    /// from the clip's first frame
    pub frame: i64,
    pub r#box: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 4]>,
    pub opacity: f64,
}

/// The key at `frame` (keys' frames counted alike): the straight line between the keys either side of it, the first's
/// before them and the last's after; none when there are no keys. A clip or object-position one key has and the other
/// hasn't goes from the whole box, or the centre, to it.
pub fn key_at(keys: &[Key], frame: i64) -> Option<Key> {
    let i = keys.iter().position(|k| k.frame >= frame).unwrap_or(keys.len().checked_sub(1)?);
    let b = &keys[i];
    if i == 0 || b.frame <= frame {
        return Some(Key { frame, ..b.clone() });
    }
    let a = &keys[i - 1];
    let t = (frame - a.frame) as f64 / (b.frame - a.frame) as f64;
    let clip = (a.clip.is_some() || b.clip.is_some()).then(|| toward(crate::plan::shown(&a.r#box, &a.clip), &crate::plan::shown(&b.r#box, &b.clip), t));
    let position = (a.position.is_some() || b.position.is_some()).then(|| toward(crate::plan::placed(&a.position), &crate::plan::placed(&b.position), t));
    Some(Key { frame, r#box: toward(a.r#box, &b.r#box, t), clip, position, opacity: a.opacity + (b.opacity - a.opacity) * t })
}

/// `from` moved `t` of the way to `to`, each of its four alike.
fn toward(mut from: [f64; 4], to: &[f64; 4], t: f64) -> [f64; 4] {
    for (v, end) in from.iter_mut().zip(to) {
        *v += (end - *v) * t;
    }
    from
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Clip {
    /// A clip of `file` from `start` for `frames`, from its source's `source_in` at speed 1, fully opaque.
    pub fn new(id: String, file: usize, start: i64, frames: i64, source_in: f64) -> Clip {
        Clip { id, file, start, frames, source_in, speed: 1.0, r#box: None, fit: None, clip: None, position: None, opacity: 1.0, keys: Vec::new(), gain_db: 0.0, fade_in: 0.0, fade_out: 0.0, link: None }
    }

    pub fn end(&self) -> i64 {
        self.start + self.frames
    }
}

/// Clips laid on as few tracks as hold them without overlap, first come first: `layer` tracks named `name`,
/// `name 2`... (the second and later are above the first, so they're listed before it: tracks go top to bottom).
pub fn lay(layer: Layer, name: &str, clips: Vec<Clip>) -> Vec<Track> {
    let mut tracks: Vec<Track> = Vec::new();
    for c in clips {
        match tracks.iter_mut().find(|t| t.clips.last().map_or(i64::MIN, Clip::end) <= c.start) {
            Some(t) => t.clips.push(c),
            None => {
                let n = tracks.len();
                tracks.push(Track { layer, name: if n == 0 { name.to_string() } else { format!("{name} {}", n + 1) }, clips: vec![c] });
            }
        }
    }
    tracks.reverse();
    tracks
}
