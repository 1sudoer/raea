//! A render's video as shot segments (docs/rerender.feature RR3): split at every cut the timeline knows (split.rs), each
//! segment its own encoder run starting on a keyframe (encode.rs), joined into one MP4 by stream copy with the sound
//! laid under once (join.rs). So a changed shot can be drawn again alone and the film joined around it (RR4), and
//! segments drawn by several workers at once (RR5).

use std::path::Path;

use raea_io_media::{EncodeOpts, R, VideoEncoder};

mod encode;
mod join;
mod split;
pub use join::join;
pub use split::{Cuts, segments};

/// The backends whose encoders write segments the join reads (muxide's MP4s: H.264, HEVC, AV1) and lay a sound under
/// a joined video (Backend::sound). The ffmpeg CLI, the baseline, encodes in one run.
pub fn segmentable(codec: &str) -> bool {
    matches!(codec, "apple" | "soft" | "av1")
}

/// Whether renders make their videos as segments: with RAEA_SEGMENTS=1, until RR3's gates are measured
pub fn on() -> bool {
    std::env::var("RAEA_SEGMENTS").as_deref() == Ok("1")
}

/// The encoder for a whole video (frame 0 first, every frame in order) to `out`: `codec`'s, a new segment at each of
/// `cuts` once the one before holds a second of frames, the whole wrapped by the signer when one is installed (one export, one
/// watermark id, the joined file signed). The ffmpeg baseline's, or with segments off: one run, as before.
pub fn encoder(codec: &str, out: &Path, opts: &EncodeOpts, ten: bool, cuts: &Cuts) -> R<Box<dyn VideoEncoder>> {
    let b = if segmentable(codec) && on() {
        raea_io_media::sign::wrap(Box::new(encode::Segs { name: codec.into(), cuts: cuts.clone(), min: (opts.fps.round() as usize).max(1) }))
    } else {
        raea_io_media::backend(codec)?
    };
    if ten { b.encoder_10bit(out, opts) } else { b.encoder(out, opts) }
}
