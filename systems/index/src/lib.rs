//! Index (docs/index.feature): every picture and video a person has, searchable in minutes. This crate is the fast
//! pass: sources are listed (walk.rs), each picture's small copy decoded and embedded by SigLIP 2 B/16-256 on the
//! Neural Engine (apple.rs) while the next ones decode, its row appended as its batch finishes (store.rs), and rows
//! searched by words through SigLIP 2's text tower (text.rs, search.rs). Libraries' deep pass (EmbeddingGemma 2 per
//! shot, sound and words) follows in the feed's host.

#[cfg(target_vendor = "apple")]
pub mod apple;
pub mod cli;
pub mod export;
#[cfg(target_vendor = "apple")]
pub mod cloud;
#[cfg(target_vendor = "apple")]
pub mod frames;
#[cfg(target_vendor = "apple")]
pub mod photos;
pub mod search;
pub mod serve;
pub mod store;
pub mod text;
pub mod walk;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Instant;
use store::{MODEL, Store};

/// A picture or a video a source holds, before the fast pass reads it
#[derive(Debug, Clone, Default)]
pub struct Found {
    /// where it lives in its source: a path, a Photos asset id, a Drive file id
    pub locator: String,
    /// "picture" or "video"
    pub kind: String,
    pub name: String,
    pub size: u64,
    /// seconds since 1970
    pub mtime: i64,
    pub taken: Option<i64>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub dur: Option<f64>,
}

/// How a run is going, said as it goes
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Progress {
    pub source: String,
    pub found: u64,
    pub done: u64,
    pub failed: u64,
    /// pictures embedded a second since the fast pass started
    pub per_sec: f64,
    pub seconds: f64,
    /// the items finished since the last report, newest last, for the wall of thumbnails
    pub newest: Vec<i64>,
    /// pictures and videos the listing left out because their bytes are only in a cloud other than iCloud (walk.rs
    /// cloud_only; Google Drive's or Dropbox's online-only files): reading one would download its original. iCloud's go
    /// in from Quick Look (cloud.rs)
    pub cloud: u64,
    pub stage: &'static str,
}

/// What an add names for Apple Photos (photos.rs), and the root its source keeps
pub const PHOTOS: &str = "photos:";

/// What an add names: Apple Photos (`photos:`), or a folder or a drive, made absolute
pub fn root_of(path: &str) -> Result<PathBuf> {
    if path == PHOTOS {
        return Ok(PathBuf::from(PHOTOS));
    }
    let root = dunce::canonicalize(path).map_err(|e| anyhow::anyhow!("{path}: {e}"))?;
    anyhow::ensure!(root.is_dir(), "{path} isn't a folder");
    Ok(root)
}

/// Lists the source an add named (root_of)
pub fn list_root(store: &mut Store, root: &Path, on: &mut dyn FnMut(&Progress)) -> Result<Listed> {
    #[cfg(target_vendor = "apple")]
    if root == Path::new(PHOTOS) {
        return list_photos(store, on);
    }
    list_folder(store, root, on)
}

pub fn normalise(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    v.iter_mut().for_each(|x| *x /= n);
}

/// SigLIP 2 B/16-256's folder in the models folder ($RAEA_MODELS, else ~/.local/share/raea-models, as perception's):
/// the image tower's Core ML package, the text tower's ONNX and the tokenizer
pub fn siglip_dir() -> PathBuf {
    raea_io_home::env_path("RAEA_MODELS", ".local/share/raea-models").join("siglip2-base-p16-256")
}

pub fn image_package() -> PathBuf {
    siglip_dir().join("SigLIP2Image.mlpackage")
}

/// What listing a source found
#[derive(Debug, Clone, Default)]
pub struct Listed {
    pub source: String,
    pub found: u64,
    pub added: u64,
    /// items whose file changed since the last listing: their old rows are gone
    pub changed: u64,
    /// items left out as only in a cloud other than iCloud (Progress::cloud)
    pub cloud: u64,
}

/// Lists a folder or a drive into the index: new files added, changed ones put back in line
pub fn list_folder(store: &mut Store, root: &Path, on: &mut dyn FnMut(&Progress)) -> Result<Listed> {
    let label = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string());
    let each = |f: &mut dyn FnMut(Vec<Found>)| Ok(walk::walk(root, 2000, f).1);
    list_source(store, ("folder", &root.to_string_lossy(), &label), each, on)
}

/// Lists Apple Photos into the index (photos.rs): its root is `photos:`, each item an asset
#[cfg(target_vendor = "apple")]
pub fn list_photos(store: &mut Store, on: &mut dyn FnMut(&Progress)) -> Result<Listed> {
    list_source(store, ("photos", PHOTOS, "Apple Photos"), |f| photos::list(2000, f).map(|_| 0).map_err(anyhow::Error::msg), on)
}

/// Lists a source (its kind, root and label) as `each` hands its items over a batch at a time and then says how many
/// it left out as only in the cloud
fn list_source(store: &mut Store, (kind, root, label): (&str, &str, &str), each: impl FnOnce(&mut dyn FnMut(Vec<Found>)) -> Result<u64>, on: &mut dyn FnMut(&Progress)) -> Result<Listed> {
    let source = store.source(kind, root, label)?;
    let started = Instant::now();
    let mut l = Listed { source: source.clone(), ..Listed::default() };
    let mut err = None;
    l.cloud = each(&mut |batch| {
        l.found += batch.len() as u64;
        match store.list(&source, &batch) {
            Ok((a, c)) => {
                l.added += a as u64;
                l.changed += c as u64;
            }
            Err(e) => {
                err.get_or_insert(e);
            }
        }
        on(&Progress { source: source.clone(), found: l.found, stage: "listing", seconds: started.elapsed().as_secs_f64(), ..Progress::default() });
    })?;
    if let Some(e) = err {
        return Err(e);
    }
    if l.cloud > 0 {
        on(&Progress { source: source.clone(), found: l.found, cloud: l.cloud, stage: "listing", seconds: started.elapsed().as_secs_f64(), ..Progress::default() });
    }
    Ok(l)
}

/// An item's frames read for the fast pass, or the item and why it couldn't be read
#[cfg(target_vendor = "apple")]
type Decoded = Result<Vec<apple::Ready>, (i64, String)>;

/// One item's frames: a Photos asset, a picture or a file kept only in iCloud gives one small copy (photos.rs,
/// apple.rs, cloud.rs), a video file its keyframes (frames.rs)
#[cfg(target_vendor = "apple")]
fn frames_of(p: &store::Pending, thumb: &Path) -> Decoded {
    let file = Path::new(&p.locator);
    let one = match p.locator.strip_prefix(PHOTOS) {
        Some(id) => Some(photos::picture(id, Some(thumb))),
        None if cloud::wants(file) => Some(cloud::picture(file, Some(thumb))),
        None if p.kind == "video" => None,
        None => Some(apple::picture(file, Some(thumb))),
    };
    let r = match one {
        Some(r) => r.map(|(px, w, h)| vec![apple::Ready { item: p.id, t: 0.0, px, w, h }]),
        None => frames::video(file, Some(thumb)).map(|f| f.px.into_iter().map(|(px, t)| apple::Ready { item: p.id, t, px, w: f.w, h: f.h }).collect()),
    };
    r.map_err(|e| (p.id, e))
}

/// The fast pass over a source's pictures and videos that wait for it: decoded on every core but two (a video as a few
/// keyframes, frames.rs), embedded about 32 frames at a time on the Neural Engine, each batch's rows written as it
/// finishes. An item's frames go in one batch, so an item counts as searchable once all of its rows are written. Files
/// kept only in iCloud follow the local ones, from Quick Look's thumbnail (cloud.rs), a few asked for at once.
#[cfg(target_vendor = "apple")]
pub fn embed_pictures(store: &mut Store, model: &apple::Model, source: &str, stop: &std::sync::Arc<std::sync::atomic::AtomicBool>, on: &mut dyn FnMut(&Progress)) -> Result<Progress> {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    use rayon::prelude::*;
    const BATCH: usize = 32;
    // how many of iCloud's thumbnails are asked for at once
    const ICLOUD: usize = 8;
    let (in_cloud, all): (Vec<store::Pending>, Vec<store::Pending>) = store.pending(source, usize::MAX)?.into_iter().partition(|p| !p.locator.starts_with(PHOTOS) && cloud::wants(Path::new(&p.locator)));
    let base = store.counts(Some(source))?;
    let (tx, rx) = std::sync::mpsc::sync_channel::<Decoded>(BATCH * 8);
    let thumbs: Vec<PathBuf> = all.iter().map(|p| store.thumb(p.id)).collect();
    let cloud_thumbs: Vec<PathBuf> = in_cloud.iter().map(|p| store.thumb(p.id)).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get().saturating_sub(2).max(2)).unwrap_or(4);
    let halt = stop.clone();
    let decoder = std::thread::spawn(move || {
        let each = |tx: &mut std::sync::mpsc::SyncSender<Decoded>, (p, thumb): (&store::Pending, &PathBuf)| {
            if !halt.load(Ordering::Relaxed) {
                let _ = tx.send(frames_of(p, thumb));
            }
        };
        // the local files on every core but two, then iCloud's: each of those waits on the network, so a few at a time
        let stages = [(threads, all, thumbs, tx.clone()), (ICLOUD, in_cloud, cloud_thumbs, tx)];
        for (n, items, thumbs, tx) in stages {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().expect("a decoding pool");
            pool.install(|| items.par_iter().zip(thumbs.par_iter()).for_each_with(tx, each));
        }
    });
    let started = Instant::now();
    let mut last = Instant::now();
    let mut prog = Progress { source: source.into(), found: base.found, done: base.done, failed: base.failed, stage: "embedding", ..Progress::default() };
    let mut batch: Vec<apple::Ready> = Vec::with_capacity(BATCH + frames::FRAMES);
    let flush = |batch: &mut Vec<apple::Ready>, prog: &mut Progress, store: &mut Store| -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let pxs: Vec<&objc2_core_video::CVPixelBuffer> = batch.iter().map(|r| &*r.px).collect();
        let vs = model.embed(&pxs).map_err(|e| anyhow::anyhow!(e))?;
        // each frame's row and its item's size, in one pass
        let (rows, dims): (Vec<(i64, f64, Vec<f32>)>, Vec<(i64, u32, u32)>) = batch.iter().zip(vs).map(|(r, v)| ((r.item, r.t, v), (r.item, r.w, r.h))).unzip();
        store.append(MODEL, &rows, &dims)?;
        // an item's frames sit together: one count and one tile each
        let mut items: Vec<i64> = batch.iter().map(|r| r.item).collect();
        items.dedup();
        prog.done += items.len() as u64;
        prog.newest.extend(items);
        batch.clear();
        Ok(())
    };
    // when the batch's first frame came: a slow source (iCloud's thumbnails) still lands its pictures within a second
    let mut waiting = Instant::now();
    loop {
        let r = match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(r) => Some(r),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if stop.load(Ordering::Relaxed) {
            prog.stage = "stopped";
            break;
        }
        match r {
            Some(Ok(item)) => {
                if batch.is_empty() {
                    waiting = Instant::now();
                }
                batch.extend(item);
                if batch.len() >= BATCH {
                    flush(&mut batch, &mut prog, store)?;
                }
            }
            Some(Err((item, why))) => {
                store.fail(item, &why)?;
                prog.failed += 1;
            }
            None => {}
        }
        if !batch.is_empty() && waiting.elapsed() >= Duration::from_secs(1) {
            flush(&mut batch, &mut prog, store)?;
        }
        if last.elapsed().as_millis() >= 250 {
            prog.seconds = started.elapsed().as_secs_f64();
            prog.per_sec = (prog.done - base.done) as f64 / prog.seconds.max(1e-3);
            on(&prog);
            prog.newest.clear();
            last = Instant::now();
        }
    }
    flush(&mut batch, &mut prog, store)?;
    let _ = decoder.join();
    prog.seconds = started.elapsed().as_secs_f64();
    prog.per_sec = (prog.done - base.done) as f64 / prog.seconds.max(1e-3);
    if prog.stage != "stopped" {
        prog.stage = "done";
    }
    on(&prog);
    prog.newest.clear();
    Ok(prog)
}
