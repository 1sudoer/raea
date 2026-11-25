//! Opening what the stage shows: a composition opened to play or to be drawn as its render draws it, a written file
//! taken in place of opening it when only its defaults' values changed (docs/realtime.feature RT4f), a tweak's next
//! version, and an element's box in the stage's pixels.

use super::*;

/// The composition at `p` opened to play, laid out at the stage's box, its sound on (RAEA_AUDIO_MUTE=1: off). With
/// `render`, drawn as `raea render` draws it (Live::open_render: its own size, seed, clock and length; RT4d): what an
/// editor shows, where a game (the default) has a session of its own and the person's input.
pub(super) fn open(p: &std::path::Path, size: (u32, u32), render: bool) -> Result<raea_runtime_live::Live, String> {
    let l = if render { raea_runtime_live::Live::open_render(p) } else { raea_runtime_live::Live::open_with(p, size, true) };
    let mut l = l.map_err(|e| format!("{}: {e}", p.display()))?;
    sound(&mut l);
    Ok(l)
}

/// What the stage shows, for taking a file written from it in place of opening that (`adopt`): the file, its text as
/// opened, its size and whether it is drawn as its render.
pub(super) struct Shown {
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) size: (u32, u32),
    pub(super) render: bool,
}

/// `src` taken in place of opening it, when it sits beside what `shown` holds and differs from it only in its
/// `<Composition defaultProps>` values (raea_sys_tweak::values_only), shown the same way: its values laid over the
/// running world (Live::adopt), the frame drawn again. Some(the file's text) when taken; None to open it, as when the
/// values change something the 3D keeps from its first frame (the answer's held parts), which only an open shows.
pub(super) fn adopt(l: &mut raea_runtime_live::Live, shown: &Shown, src: &std::path::Path, size: (u32, u32), render: bool) -> Option<String> {
    if (size, render) != (shown.size, shown.render) || src.parent() != shown.path.parent() {
        return None;
    }
    let text = std::fs::read_to_string(src).ok()?;
    let values = raea_sys_tweak::values_only(&shown.text, &text)?;
    match l.adopt(values) {
        Ok(_) if l.held().is_empty() => Some(text),
        Ok(_) => None,
        Err(e) => {
            eprintln!("raea-page-host: stage: {} not taken in place ({e}): opened", src.display());
            None
        }
    }
}

fn sound(l: &mut raea_runtime_live::Live) {
    if std::env::var("RAEA_AUDIO_MUTE").as_deref() != Ok("1") {
        l.open_mixer();
    }
}

/// The changes written into the next version beside `at` (raea-sys-tweak), opened and stepped through the frames played so
/// far with the input each had, so it stands where the old one did. The old one plays on when any of it fails.
pub(super) fn next_version(at: &std::path::Path, changes: &[Change], size: (u32, u32), log: &[(Vec<String>, Option<(f64, f64, bool)>)]) -> Result<(PathBuf, raea_runtime_live::Live), String> {
    let src = std::fs::read_to_string(at).map_err(|e| format!("{}: {e}", at.display()))?;
    let tweaks: Vec<raea_sys_tweak::Tweak> = changes.iter().map(|c| raea_sys_tweak::Tweak { owner: &c.owner, kind: &c.kind, prop: &c.prop, value: &c.value }).collect();
    let out = raea_sys_tweak::tweak(&src, &tweaks)?;
    let next = raea_sys_tweak::free_name(at);
    std::fs::write(&next, out).map_err(|e| format!("{}: {e}", next.display()))?;
    // its sound only from here on: the replay is silent
    let mut l = raea_runtime_live::Live::open_with(&next, size, true).map_err(|e| format!("{}: {e}", next.display()))?;
    for (f, (keys, pointer)) in log.iter().enumerate() {
        l.step(f as i64, keys, *pointer).map_err(|e| format!("{} at frame {f}: {e}", next.display()))?;
    }
    l.draw()?;
    sound(&mut l);
    eprintln!("raea-page-host: stage: wrote {}, replayed {} frames", next.display(), log.len());
    Ok((next, l))
}

/// An element's box in the stage's pixels (the page's scaled to the stage), beside its own in page px.
pub(super) fn on_stage(mut e: Value, l: &raea_runtime_live::Live, size: (u32, u32)) -> Value {
    let (sx, sy) = (size.0 as f64 / l.w.max(1) as f64, size.1 as f64 / l.h.max(1) as f64);
    if let Some(r) = e["rect"].as_array().map(|r| r.iter().filter_map(Value::as_f64).collect::<Vec<_>>()).filter(|r| r.len() == 4) {
        e["stage"] = json!([r[0] * sx, r[1] * sy, r[2] * sx, r[3] * sy]);
    }
    if e.get("under").is_some_and(Value::is_object) {
        e["under"] = on_stage(e["under"].take(), l, size);
    }
    e
}
