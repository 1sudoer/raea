//! raea's mix, before its master: every line of dialogue at one loudness, so the master brings the whole piece to its
//! target without a line louder or quieter than the next (11-14: a phone walkaround's lines ran from -13.8 to -26.9
//! LUFS; the agent measured each and set its gainDb, and the limiter still flattened the loud ones).
//!
//! Dialogue is a clip on the voice bus, or footage (a video file's sound) on no bus. It's measured over the span of its
//! source it plays (BS.1770 integrated loudness, gated, so its pauses don't count) and brought to DIALOGUE, by at most
//! REACH either way. A clip quieter than ROOM is room tone or a silent take: it keeps its level. Everything else (music,
//! effects, ambience, anything on another bus) keeps its own level and its bus's settings, which now sit under dialogue
//! that's always at the same place. A clip's gainDb is an offset on top of the level, never undone by it.
//!
//! Live play and the render call the same function on the same span, so they hear the same mix; each span is measured
//! once a process.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// Where dialogue sits before the master: EBU R128's programme level, the master brings it up to the delivery's
pub const DIALOGUE: f64 = -23.0;
/// The most a clip is moved, up or down (dB)
pub const REACH: f64 = 12.0;
/// Under this a clip is room tone or silence, not a line (LUFS)
pub const ROOM: f64 = -45.0;
/// A span with no end is measured this far
const OPEN: f64 = 30.0;

/// Each file's measured spans, (from ms, to ms or -1, gain): looked up by the path as it is, without making a key,
/// since live play asks every frame
static LEVELS: Mutex<Option<HashMap<PathBuf, Vec<(i64, i64, f32)>>>> = Mutex::new(None);

/// A video file's extension (its sound is the footage's own)
fn footage(ext: &str) -> bool {
    for v in ["mov", "mp4", "m4v", "webm", "mkv"] {
        if ext.eq_ignore_ascii_case(v) {
            return true;
        }
    }
    false
}

/// Whether a clip is dialogue: on the voice bus, or a video file's sound on no bus
pub fn is_dialogue(file: &Path, bus: Option<&str>) -> bool {
    match bus {
        Some(b) => b == "voice",
        None => file.extension().and_then(|x| x.to_str()).is_some_and(footage),
    }
}

/// The gain (dB) that brings a clip of `loudness` LUFS to DIALOGUE: 0 for room tone or an unmeasurable span
pub fn to_dialogue(loudness: f64) -> f64 {
    if !loudness.is_finite() || loudness < ROOM { 0.0 } else { (DIALOGUE - loudness).clamp(-REACH, REACH) }
}

fn millis(s: f64) -> i64 {
    (s * 1000.0).round() as i64
}

/// A span of `file` measured before: its gain
fn known(file: &Path, a: i64, b: i64) -> Option<f32> {
    let levels = LEVELS.lock().unwrap_or_else(PoisonError::into_inner);
    for s in levels.as_ref()?.get(file)? {
        if s.0 == a && s.1 == b {
            return Some(s.2);
        }
    }
    None
}

/// The linear gain raea's mix puts on a clip playing `file` from `from` to `to` seconds of it (None: to its end, or
/// OPEN on), on `bus`: dialogue's level, else 1
pub fn gain(file: &Path, from: f64, to: Option<f64>, bus: Option<&str>) -> f32 {
    if !is_dialogue(file, bus) {
        return 1.0;
    }
    let (a, b) = (millis(from), to.map_or(-1, millis));
    if let Some(g) = known(file, a, b) {
        return g;
    }
    let secs = to.map_or(OPEN, |t| (t - from).max(0.4));
    let loud = raea_io_media::decode_audio_window(file, from.max(0.0), secs).map_or(f64::NAN, |a| raea_io_media::audio::lufs(&a));
    let g = 10f64.powf(to_dialogue(loud) / 20.0) as f32;
    LEVELS.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).entry(file.to_path_buf()).or_default().push((a, b, g));
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The walkaround's lines, measured 11-14, all come to -23; room tone and silence keep their level; nothing moves
    /// further than REACH
    #[test]
    fn dialogue_comes_to_one_level() {
        for (line, loud) in [("intro", -22.1), ("ext5", -26.9), ("rear2", -16.4), ("trunk1", -13.8)] {
            let g = to_dialogue(loud);
            assert!((loud + g - DIALOGUE).abs() < 1e-9, "{line}: {loud} + {g}");
        }
        assert_eq!(to_dialogue(-50.0), 0.0);
        assert_eq!(to_dialogue(f64::NEG_INFINITY), 0.0);
        assert_eq!(to_dialogue(-40.0), REACH);
        assert_eq!(to_dialogue(0.0), -REACH);
    }

    /// Dialogue is the voice bus, or footage on no bus; music, effects and ambience keep their own level
    #[test]
    fn what_is_dialogue() {
        assert!(is_dialogue(Path::new("take.MOV"), None));
        assert!(is_dialogue(Path::new("vo.wav"), Some("voice")));
        assert!(!is_dialogue(Path::new("song.mp3"), None));
        assert!(!is_dialogue(Path::new("take.mov"), Some("music")));
        assert!(!is_dialogue(Path::new("hit.wav"), Some("sfx")));
        assert_eq!(gain(Path::new("song.mp3"), 0.0, None, None), 1.0);
    }
}
