//! CC2a without a render: where a picture sits in CapCut's terms, and layers.json as CapCut's tracks (LA4b). The
//! manifest's runs, sounds and spans are tested where they're read, in raea-app-layers.

use std::path::Path;

use raea_app_layers::model::{self, Clip as LClip, File, FileKind, Layer, Layers, Track as LTrack};

use crate::from_layers;
use crate::plan::*;

#[test]
fn pictures_sit_as_their_fit_shows_them() {
    let at = |bx: [f64; 4], fit: &str, media: (f64, f64)| {
        let (p, note) = raea_app_layers::plan::picture(&bx, &None, fit, &None, media, (1080.0, 1920.0));
        (place(&p, media, (1080.0, 1920.0), 1.0), note)
    };
    // a landscape take covering a portrait canvas: scaled past the fitted width until it fills the height, uncut (the
    // canvas cuts it)
    let (p, w) = at([0.0, 0.0, 1080.0, 1920.0], "cover", (1920.0, 1080.0));
    assert!(w.is_none() && (p.scale - (1920.0 / 1080.0) / 0.5625).abs() < 1e-9 && p.crop == [0.0, 0.0, 1.0, 1.0]);
    assert_eq!((p.x, p.y), (540.0, 960.0));
    // a square in a corner, contained: a third of the canvas's width
    let (p, _) = at([660.0, 200.0, 360.0, 360.0], "contain", (640.0, 640.0));
    assert!((p.scale - 1.0 / 3.0).abs() < 1e-9 && (p.x, p.y) == (840.0, 380.0));
    // a landscape take covering a square box inside the canvas: cropped to its middle square, which then fits as a square
    let (p, _) = at([100.0, 100.0, 540.0, 540.0], "cover", (1920.0, 1080.0));
    assert!((p.crop[0] - 0.21875).abs() < 1e-9 && (p.crop[2] - 0.78125).abs() < 1e-9 && p.crop[1] == 0.0);
    assert!((p.scale - 0.5).abs() < 1e-9 && ((p.x - 370.0).abs(), (p.y - 370.0).abs()) < (1e-9, 1e-9));
    // the same box in the canvas's corner: cut only on its right, the canvas cutting its left; the part kept sits where
    // raea drew it, its left past the canvas
    let (p, _) = at([0.0, 0.0, 540.0, 540.0], "cover", (1920.0, 1080.0));
    assert!(p.crop[0] == 0.0 && (p.crop[2] - 0.78125).abs() < 1e-9);
    assert!((p.x - (-210.0 + 750.0 / 2.0)).abs() < 1e-9 && (p.y - 270.0).abs() < 1e-9);
    // stretched in raea: said
    assert!(at([0.0, 0.0, 500.0, 100.0], "fill", (640.0, 640.0)).1.is_some());
}

#[test]
fn layers_json_goes_bottom_first_with_footage_carrying_its_own_sound() {
    let file = |path: &str, kind, w, h| File { path: path.into(), kind, seconds: 3.0, width: w, height: h, has_audio: kind == FileKind::Video, channels: 2, sample_rate: 48_000, rotation: 0 };
    let clip = |id: &str, f, start, frames| LClip::new(id.into(), f, start, frames, 0.0);
    let t = |layer, name: &str, clips| LTrack { layer, name: name.into(), clips };
    let v1 = LClip { r#box: Some([0.0, 0.0, 1080.0, 1920.0]), fit: Some("cover".into()), link: Some("a1".into()), ..clip("v1", 0, 0, 90) };
    let a1 = LClip { gain_db: -6.0, link: Some("v1".into()), ..clip("a1", 0, 0, 90) };
    let l = Layers {
        version: model::VERSION,
        name: "edit".into(),
        width: 1080,
        height: 1920,
        fps: 30.0,
        frames: 90,
        reference: "reference.mp4".into(),
        srt: Some("edit.srt".into()),
        files: vec![file("/m/a.mp4", FileKind::Video, 1920, 1080), file("captions.prores.mov", FileKind::Video, 1080, 1920), file("graphics.prores.mov", FileKind::Video, 1080, 1920), file("/m/music.wav", FileKind::Audio, 0, 0)],
        tracks: vec![
            t(Layer::Captions, "captions", vec![clip("c1", 1, 10, 30)]),
            t(Layer::Graphics, "graphics", vec![clip("g1", 2, 0, 90)]),
            t(Layer::Footage, "footage", vec![v1]),
            t(Layer::Sound, "sound", vec![a1, LClip { gain_db: -12.0, fade_out: 0.5, ..clip("a2", 3, 0, 90) }]),
        ],
        notes: vec![],
    };
    let (tl, _) = from_layers::timeline(&l, Path::new("/out/edit"));
    // bottom first: the footage is CapCut's main track (no name), then the overlay, then the captions; sound after
    assert_eq!(tl.tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["", "raea overlay", "raea captions", "sound"]);
    // a layer's movie is found beside layers.json; the footage where it is
    assert_eq!(tl.media[1].path, Path::new("/out/edit/captions.prores.mov"));
    assert_eq!(tl.media[0].path, Path::new("/m/a.mp4"));
    // the footage carries its own sound at its gain (-6 dB), so that sound isn't a clip of its own; the music is
    let foot = &tl.tracks[0].clips[0];
    assert!((foot.volume - 10f64.powf(-6.0 / 20.0)).abs() < 1e-9);
    assert_eq!(tl.tracks[3].clips.len(), 1);
    assert!((tl.tracks[3].clips[0].volume - 10f64.powf(-12.0 / 20.0)).abs() < 1e-9 && tl.tracks[3].clips[0].fade_out == 0.5);
    // the layers' movies fill the canvas, silent
    let cap = &tl.tracks[2].clips[0];
    assert_eq!((cap.start, cap.frames, cap.volume, cap.place.scale, cap.place.x, cap.place.y), (10, 30, 0.0, 1.0, 540.0, 960.0));
    // footage with no linked sound plays silent
    let mut quiet = l.clone();
    quiet.tracks[2].clips[0].link = None;
    quiet.tracks[3].clips.remove(0);
    assert_eq!(from_layers::timeline(&quiet, Path::new("/out/edit")).0.tracks[0].clips[0].volume, 0.0);
}

/// CC2f: a footage clip's keys in CapCut's terms, around the cut its first frame shows: a steady move is its two ends,
/// a move inside a card keeps the card's cut, and a cut that changes (the split close) is said
#[test]
fn keyed_footage_is_keyed_around_its_first_cut() {
    use raea_app_layers::model::Key;
    let canvas = (1080.0, 1920.0);
    let key = |frame: i64, bx: [f64; 4], clip: Option<[f64; 4]>| Key { frame, r#box: bx, clip, position: None, opacity: 1.0 };
    let first = |c: &LClip| raea_app_layers::plan::picture(&c.r#box.unwrap(), &c.clip, "cover", &None, (1080.0, 1920.0), canvas).0.kept;
    // a picture in picture sliding right 10 px a frame: its two ends
    let slide = LClip { r#box: Some([100.0, 500.0, 400.0, 400.0]), keys: (0..24).map(|f| key(f, [100.0 + 10.0 * f as f64, 500.0, 400.0, 400.0], None)).collect(), ..LClip::new("v".into(), 0, 0, 24, 0.0) };
    let (got, changes) = keys(&slide, "cover", first(&slide), (1080.0, 1920.0), canvas);
    assert!(!changes && got.len() == 2 && got[1].0 == 23 && (got[1].1.x - got[0].1.x - 230.0).abs() < 1e-6);
    // inside a card that moves with it, cutting its sides: the same cut throughout, keyed as it moves
    let card = |f: i64| key(f, [80.0 + 4.0 * f as f64, 300.0, 600.0, 1066.7], Some([140.0 + 4.0 * f as f64, 300.0, 480.0, 1066.7]));
    let carded = LClip { r#box: Some(card(0).r#box), clip: card(0).clip, keys: (0..24).map(card).collect(), ..LClip::new("v".into(), 0, 0, 24, 0.0) };
    let (got, changes) = keys(&carded, "cover", first(&carded), (1080.0, 1920.0), canvas);
    assert!(!changes && got.len() == 2 && got.iter().all(|(_, p)| p.crop[0] > 0.09 && p.crop[2] < 0.91));
    // the split close: its box shrinking to the seam changes what's cut, which CapCut's crop can't follow
    let split = |f: i64, h: f64| key(f, [0.0, -260.0 * (1920.0 - h) / 1060.0, 1080.0, 1920.0], (h < 1920.0).then_some([0.0, 0.0, 1080.0, h]));
    let closing = LClip { r#box: Some(split(0, 1920.0).r#box), keys: vec![split(0, 1920.0), split(12, 860.0)], ..LClip::new("v".into(), 0, 0, 24, 0.0) };
    let (got, changes) = keys(&closing, "cover", first(&closing), (1080.0, 1920.0), canvas);
    assert!(changes && got.len() == 2 && got[1].1.crop == [0.0, 0.0, 1.0, 1.0]);
}
