//! Segments joined into one MP4 by stream copy: each segment's samples as they were encoded, one after another (H.264's
//! and HEVC's as Annex B with their parameter sets before each keyframe, AV1's OBUs as they are), and the sound laid
//! under them once, from the render's own mix. Nothing is decoded or encoded again.

use std::path::Path;

use muxide::api::{AacProfile, AudioCodec, MuxerBuilder, VideoCodec};
use raea_io_codec::remux::{avcc_params, avcc_to_annexb, hvcc_params};
use raea_io_media::{R, SoundTrack};

/// A segment's video: its codec and size, and each sample with its key flag, as the muxer takes them.
pub(crate) struct Track {
    pub codec: VideoCodec,
    pub w: u32,
    pub h: u32,
    pub samples: Vec<(Vec<u8>, bool)>,
}

/// Whether a length-prefixed sample carries its own parameter sets (H.264's SPS or PPS, HEVC's VPS, SPS or PPS).
fn has_params(sample: &[u8], hevc: bool) -> bool {
    let mut i = 0;
    while i + 4 < sample.len() {
        let n = u32::from_be_bytes([sample[i], sample[i + 1], sample[i + 2], sample[i + 3]]) as usize;
        let t = sample[i + 4];
        if (!hevc && matches!(t & 0x1f, 7 | 8)) || (hevc && matches!((t >> 1) & 0x3f, 32..=34)) {
            return true;
        }
        i += 4 + n;
    }
    false
}

/// A segment's video read back from its MP4.
pub(crate) fn read(path: &Path) -> R<Track> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m = re_mp4::Mp4::read_bytes(&b).map_err(|e| format!("{}: {e}", path.display()))?;
    let (_, t) = m.tracks().iter().find(|(_, t)| t.kind == Some(re_mp4::TrackKind::Video)).ok_or_else(|| format!("{}: no video", path.display()))?;
    let config = t.raw_codec_config(&m).unwrap_or_default();
    let (codec, params) = match &t.trak(&m).mdia.minf.stbl.stsd.contents {
        re_mp4::StsdBoxContent::Avc1(_) => (VideoCodec::H264, Some(avcc_params(&config))),
        re_mp4::StsdBoxContent::Hvc1(_) | re_mp4::StsdBoxContent::Hev1(_) => (VideoCodec::H265, Some(hvcc_params(&config))),
        re_mp4::StsdBoxContent::Av01(_) => (VideoCodec::Av1, None),
        _ => return Err(format!("{}: a segment's video is H.264, HEVC or AV1 ({:?})", path.display(), t.codec_string(&m))),
    };
    let hevc = codec == VideoCodec::H265;
    let samples = t.samples.iter().map(|s| {
        let data = &b[s.byte_range()];
        let out = match &params {
            // the parameter sets before a keyframe that doesn't carry its own, as an encoder's own stream has them
            Some(p) => avcc_to_annexb(data, (s.is_sync && !has_params(data, hevc)).then_some(&p[..])),
            None => data.to_vec(),
        };
        (out, s.is_sync)
    });
    Ok(Track { codec, w: t.width as u32, h: t.height as u32, samples: samples.collect() })
}

/// The segments at `paths`, in order, as one MP4 at `out` at `fps`, with `sound` under them. Each segment starts on a
/// keyframe; they share a codec and a size or the join is refused (a segment made with other settings is drawn again).
pub fn join(paths: &[impl AsRef<Path>], sound: Option<&SoundTrack>, fps: f64, out: &Path) -> R<usize> {
    let tracks: Vec<Track> = paths.iter().map(|p| read(p.as_ref())).collect::<R<_>>()?;
    let first = tracks.first().ok_or("no segments to join")?;
    for (t, p) in tracks.iter().zip(paths) {
        if (t.codec, t.w, t.h) != (first.codec, first.w, first.h) {
            return Err(format!("{}: {:?} {}x{}, not the first segment's {:?} {}x{}", p.as_ref().display(), t.codec, t.w, t.h, first.codec, first.w, first.h));
        }
        if t.samples.first().is_some_and(|s| !s.1) {
            return Err(format!("{}: a segment starts on a keyframe", p.as_ref().display()));
        }
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut b = MuxerBuilder::new(std::io::BufWriter::new(file)).video(first.codec, first.w, first.h, fps).with_fast_start(true);
    if let Some(s) = sound {
        let codec = if s.aac { AudioCodec::Aac(AacProfile::Lc) } else { AudioCodec::Opus };
        b = b.audio(codec, s.rate, s.channels).audio_priming(s.priming);
    }
    let mut m = b.build().map_err(|e| format!("muxide: {e}"))?;
    let packets = sound.map_or(&[][..], |s| &s.packets[..]);
    let (mut k, mut ai) = (0usize, 0usize);
    for (data, key) in tracks.iter().flat_map(|t| t.samples.iter()) {
        let pts = k as f64 / fps;
        while ai < packets.len() && packets[ai].0 < pts {
            m.write_audio(packets[ai].0, &packets[ai].1).map_err(|e| format!("muxide audio: {e}"))?;
            ai += 1;
        }
        m.write_video(pts, data, *key).map_err(|e| format!("muxide video: {e}"))?;
        k += 1;
    }
    let end = k as f64 / fps;
    for (pts, d) in packets[ai..].iter().filter(|p| p.0 < end) {
        m.write_audio(*pts, d).map_err(|e| format!("muxide audio: {e}"))?;
    }
    m.finish().map_err(|e| format!("muxide: {e}"))?;
    Ok(k)
}
