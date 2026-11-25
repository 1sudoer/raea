//! raea/wear's bake as a native call and a verb (docs/wear.feature WR4): `wear.bake` from a composition and
//! `raea wear bake REQUEST.json [-o OUT.json]` from a shell both run raea-sys-weathering's bake of a .glb found from
//! the composition's folder, and keep it in that folder's asset store under the request's content key: made once,
//! and the same request gives the same files. The runtime hands in where the composition is and how a media path
//! (staticFile's /media/...) resolves, so this crate needs no engine.

use std::path::{Path, PathBuf};

use serde_json::Value;

type R<T> = Result<T, String>;

/// Where the composition is, and how its media paths resolve (the runtime's own).
#[derive(Clone, Copy)]
pub struct Place {
    pub dir: fn() -> Option<PathBuf>,
    pub media: fn(&Path, &str) -> PathBuf,
}

pub const TAKES: &str = "{ src: string; env?: { up?: [number, number, number]; north?: [number, number, number]; rain?: { dir?: [number, number, number]; spread?: number }; sun?: { lat?: number }; snow?: { dir?: [number, number, number]; spread?: number }; walk?: string[] | { y: number }; density?: number | Record<string, number>; max_side?: number; rays?: { sky?: number; cavity?: number } }; classes?: Record<string, string | { alpha: Record<string, string>; default?: string }> }";
pub const GIVES: &str = "{ key: string; file: string; maps: { causes0: string; causes1: string; causes2: string; causes3: string }; spots: string; size: [number, number]; density: number; charts: number; tris: number; ms: Record<string, number> }";
const ABOUT: &str = "a .glb's weathering causes baked once and kept (raea/wear): the model with a second UV set, and four maps over it (sky, sun, cavity, edge; run-off, deposit, touch; height over the ground, element, open sky; December's sun, driven snow, litter)";

pub const USAGE: &str = "raea wear bake REQUEST.json [-o OUT.json]";

pub fn register(place: Place) {
    // the registry takes a plain function, so the place is kept beside it (the first runtime's)
    let _ = PLACE.set(place);
    raea_native::register("wear.bake", TAKES, GIVES, ABOUT, |v| {
        let p = *PLACE.get().ok_or("wear.bake: the runtime didn't say where the composition is")?;
        bake_in(&(p.dir)().unwrap_or_else(|| PathBuf::from(".")), p.media, v)
    });
}

static PLACE: std::sync::OnceLock<Place> = std::sync::OnceLock::new();

/// The bake of `v.src` (resolved from `dir`) under `v`'s env and classes, kept in `dir`'s asset store.
pub fn bake_in(dir: &Path, media: fn(&Path, &str) -> PathBuf, v: &Value) -> R<Value> {
    let src = v.get("src").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or("wear.bake: src is the .glb to bake (staticFile(\"model.glb\") or a path)")?;
    let file = media(dir, src);
    if !file.is_file() {
        return Err(format!("wear.bake: no file at {} (src {src:?}, from {})", file.display(), dir.display()));
    }
    let store = raea_io_assets::Store::beside(&dir.join("edit.tsx")).dir;
    raea_sys_bake::making(|| raea_sys_weathering::bake(&store, &file, v))
}

/// `raea wear bake REQUEST.json [-o OUT.json]`: the bake from a shell, in the current folder (the composition's).
/// Prints the model's path, or writes the whole answer to OUT.json and prints its path.
pub fn cli(args: &[String], media: fn(&Path, &str) -> PathBuf) -> R<String> {
    if args.get(1).map(String::as_str) != Some("bake") {
        return Err(USAGE.into());
    }
    let req = args.get(2).filter(|a| !a.starts_with('-')).ok_or(USAGE)?;
    let v: Value = serde_json::from_slice(&std::fs::read(req).map_err(|e| format!("{req}: {e}"))?).map_err(|e| format!("{req}: {e}"))?;
    let out = bake_in(Path::new("."), media, &v)?;
    let o = match args.iter().position(|a| a == "-o") {
        Some(i) => Some(args.get(i + 1).ok_or(USAGE)?),
        None => None,
    };
    match o {
        Some(o) => {
            std::fs::write(o, serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?).map_err(|e| format!("{o}: {e}"))?;
            Ok(o.clone())
        }
        None => Ok(out.get("file").and_then(Value::as_str).map(String::from).unwrap_or_else(|| out.to_string())),
    }
}

#[cfg(test)]
mod tests;
