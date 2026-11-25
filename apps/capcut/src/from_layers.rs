//! layers.json as a CapCut timeline (docs/layers.feature LA4b): its tracks bottom first, as CapCut lists them (the
//! first video track is CapCut's main track); footage placed in CapCut's terms (plan::place); a footage clip's own
//! sound, which layers.json links to it, as the clip's volume (CapCut's Extract audio separates it); every layer's
//! movie a full-canvas clip. Pure: the export (lib.rs) makes the layers and writes the draft around it.

use std::path::Path;

use raea_app_layers::model::{FileKind, Layer, Layers};
use raea_io_capcut::{Clip, Kind, Media, MediaKind, Place, Timeline, Track};

use crate::plan;

/// The track names CapCut shows for raea's layers (the overlay's, as the export has always named it).
pub fn track_name(layer: Layer, name: &str) -> String {
    match layer {
        Layer::Captions => format!("raea {name}"),
        Layer::Graphics => name.replacen("graphics", "raea overlay", 1),
        Layer::Under | Layer::Backing => format!("raea {name}"),
        Layer::Footage | Layer::Sound => name.to_string(),
    }
}

/// The timeline, and what it can't carry. `dir` is where layers.json is: its movies' paths are relative to it.
pub fn timeline(l: &Layers, dir: &Path) -> (Timeline, Vec<String>) {
    let mut notes = Vec::new();
    let media: Vec<Media> = l
        .files
        .iter()
        .map(|f| {
            let p = Path::new(&f.path);
            Media { path: if p.is_absolute() { p.to_path_buf() } else { dir.join(p) }, kind: if f.kind == FileKind::Audio { MediaKind::Audio } else { MediaKind::Video }, seconds: f.seconds, width: f.width, height: f.height, has_audio: f.has_audio }
        })
        .collect();
    let all = || l.tracks.iter().flat_map(|t| t.clips.iter());
    // the sound clips a footage clip carries as its own (their ids)
    let carried: Vec<&str> = all().filter(|c| c.r#box.is_some()).filter_map(|c| c.link.as_deref()).collect();
    let mut video: Vec<Track> = Vec::new();
    let mut audio: Vec<Track> = Vec::new();
    for t in &l.tracks {
        let clips: Vec<Clip> = t
            .clips
            .iter()
            .filter(|c| !carried.contains(&c.id.as_str()))
            .map(|c| {
                let base = Clip { media: c.file, start: c.start, frames: c.frames, source_in: c.source_in, speed: c.speed.max(0.01), ..Clip::default() };
                match t.layer {
                    Layer::Sound => Clip { volume: 10f64.powf(c.gain_db / 20.0), fade_in: c.fade_in, fade_out: c.fade_out, ..base },
                    Layer::Footage => {
                        let m = &media[c.file];
                        // (a picture stretched to its box is noted by the layers export already, for every editor)
                        let (media, canvas) = (((m.width as f64).max(1.0), (m.height as f64).max(1.0)), (l.width as f64, l.height as f64));
                        let bx = c.r#box.unwrap_or([0.0, 0.0, canvas.0, canvas.1]);
                        let fit = c.fit.as_deref().unwrap_or("fill");
                        let (picture, _) = raea_app_layers::plan::picture(&bx, &c.clip, fit, &c.position, media, canvas);
                        let place = plan::place(&picture, media, canvas, c.opacity);
                        let (keys, cut_changes) = plan::keys(c, fit, picture.kept, media, canvas);
                        if cut_changes {
                            let name = Path::new(&l.files[c.file].path).file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            notes.push(format!("{name} (frames {}-{}): what of it is cut changes over its run, and CapCut's crop holds: cut as its first frame shows it throughout, its moves and fades keyed", c.start, c.end() - 1));
                        }
                        // its own sound at its gain, or silent: the composition doesn't play its sound there
                        let own = c.link.as_deref().and_then(|id| all().find(|a| a.id == id));
                        Clip { volume: own.map_or(0.0, |a| 10f64.powf(a.gain_db / 20.0)), place, keys, ..base }
                    }
                    Layer::Graphics | Layer::Captions | Layer::Under | Layer::Backing => Clip { volume: 0.0, place: Place::centred(l.width, l.height), ..base },
                }
            })
            .collect();
        if clips.is_empty() {
            continue;
        }
        let track = Track { kind: if t.layer == Layer::Sound { Kind::Audio } else { Kind::Video }, name: track_name(t.layer, &t.name), clips };
        if t.layer == Layer::Sound { audio.push(track) } else { video.push(track) }
    }
    // bottom first; CapCut's main track (the first) keeps no name of its own when it's footage
    video.reverse();
    if let Some(main) = video.first_mut().filter(|t| t.name.starts_with("footage")) {
        main.name.clear();
    }
    video.extend(audio);
    (Timeline { width: l.width, height: l.height, fps: l.fps, media, tracks: video }, notes)
}
