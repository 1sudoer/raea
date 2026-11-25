//! What a render is doing while it runs, for whatever shows it: the raea app's chat draws a meter from it (the
//! developer, 11-16: "in the chat ui having something that if its rendering we can see a progress meter"). One small
//! JSON file per render, `<renders>/<pid>-<n>.json`:
//!
//! `{"pid", "out", "cwd", "frames", "done", "started", "updated"}` (times in ms since the epoch)
//!
//! It's written when the render starts, as its frames finish (at most four times a second, and at the last frame),
//! and taken away when the render ends, however it ends; one left by a process that died is a reader's to pass over
//! (its pid is gone). `<renders>` is $RAEA_RENDERS, else ~/.raea/renders; `RAEA_RENDERS=off` writes none.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SEQ: AtomicUsize = AtomicUsize::new(0);
const EVERY: Duration = Duration::from_millis(250);

/// A render's progress, kept on disk while it lives.
pub struct Progress {
    file: Option<PathBuf>,
    out: String,
    cwd: String,
    frames: usize,
    done: usize,
    started: u64,
    last: Option<Instant>,
}

fn dir() -> Option<PathBuf> {
    let named = std::env::var_os("RAEA_RENDERS").filter(|v| !v.is_empty());
    if named.as_deref() == Some(std::ffi::OsStr::new("off")) {
        return None;
    }
    // unset: ~/.raea/renders, the home being USERPROFILE on Windows, which has no HOME
    match (named, raea_io_home::user_home()) {
        (Some(d), _) => Some(PathBuf::from(d)),
        (None, Some(home)) => Some(Path::new(&home).join(".raea/renders")),
        (None, None) => None,
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Progress {
    /// A render of `frames` frames to `out`, starting.
    pub fn start(out: &Path, frames: usize) -> Self {
        Self::start_in(dir(), out, frames)
    }

    fn start_in(dir: Option<PathBuf>, out: &Path, frames: usize) -> Self {
        let file = dir.and_then(|d| std::fs::create_dir_all(&d).ok().map(|_| d.join(format!("{}-{}.json", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)))));
        let out = std::path::absolute(out).unwrap_or_else(|_| out.to_path_buf()).display().to_string();
        let cwd = std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default();
        let mut p = Progress { file, out, cwd, frames, done: 0, started: now_ms(), last: None };
        p.write();
        p
    }

    /// One more frame done.
    pub fn frame(&mut self) {
        self.done += 1;
        if self.done >= self.frames || self.last.is_none_or(|t| t.elapsed() >= EVERY) {
            self.write();
        }
    }

    fn write(&mut self) {
        let Some(f) = &self.file else { return };
        self.last = Some(Instant::now());
        let body = serde_json::json!({"pid": std::process::id(), "out": self.out, "cwd": self.cwd, "frames": self.frames, "done": self.done, "started": self.started, "updated": now_ms()});
        // written whole beside it, then put in its place: a reader never sees half of one
        let part = f.with_extension("json.part");
        if std::fs::write(&part, body.to_string()).is_ok() {
            let _ = std::fs::rename(&part, f);
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if let Some(f) = &self.file {
            let _ = std::fs::remove_file(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_as_frames_finish_and_gone_when_the_render_ends() {
        let d = std::env::temp_dir().join(format!("raea-renders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let read = || -> serde_json::Value {
            let f = std::fs::read_dir(&d).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "json")).unwrap();
            serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap()
        };
        let mut p = Progress::start_in(Some(d.clone()), Path::new("film.mp4"), 3);
        let v = read();
        assert_eq!((v["frames"].as_u64(), v["done"].as_u64(), v["pid"].as_u64()), (Some(3), Some(0), Some(std::process::id() as u64)));
        assert!(v["out"].as_str().unwrap().ends_with("film.mp4") && Path::new(v["out"].as_str().unwrap()).is_absolute());
        p.frame();
        p.frame();
        p.frame();
        // the last frame is always written, however soon after the one before
        assert_eq!(read()["done"].as_u64(), Some(3));
        drop(p);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }
}
