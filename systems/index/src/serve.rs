//! `raea index serve` (docs/index.feature IX13): the index kept open for the feed, one JSON object a line each way.
//! Each ask on stdin carries an `id` and an `op` and gets one answer with the same id on stdout; the fast pass's
//! progress comes between them as `{"ev": ...}` lines. Indexing runs on a worker with its own connection and the
//! Neural Engine model, searches on a searcher that keeps the text tower and the rows loaded, so a search answers
//! while a drive is still going in.
//!   {"id":1,"op":"add","path":"/Volumes/Photos"}               a folder or a drive: listed, then embedded
//!   {"id":2,"op":"search","q":"a red car","k":60,"source":ID}  `source` optional: one source's
//!   {"id":3,"op":"similar","item":42,"k":60}                   what looks like an item
//!   {"id":4,"op":"status"}                                     the sources, what's running and waiting
//!   {"id":5,"op":"remove","source":ID}                         (or the folder's path)
//!   {"id":6,"op":"stop"}                                       the run going now, and the ones waiting
//!   {"id":7,"op":"warm"}                                       the text tower loaded before the first search
//!   {"id":8,"op":"rows","after":-1,"limit":500}                searchable rows after a row, with their vectors (IX10)
//! A search with "vector": true also answers its query's vector (export.rs: base64 of f16), to ask raea's cloud too.
//! Events: ready {home, model}, progress (lib.rs Progress: stage listing|embedding|done|stopped, newest item ids),
//! error {root, message}. A thumbnail is home/thumbs/<id % 256>/<id>.jpg.

use crate::search::{self, Rows};
use crate::store::{MODEL, Store};
use crate::text::Text;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

struct Shared {
    out: Mutex<std::io::Stdout>,
    /// bumped when rows leave search (a source removed, a file changed), so the searcher reads them all again
    epoch: AtomicU64,
    stop: Arc<AtomicBool>,
    /// the folder going in now, and its source once it's listed
    running: Mutex<Option<(String, Option<String>)>>,
    /// folders waiting their turn; a stopped or removed one leaves this list and the worker skips it
    waiting: Mutex<Vec<String>>,
}

impl Shared {
    fn send(&self, v: &Value) {
        let mut o = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    }

    fn progress(&self, p: &crate::Progress) {
        let mut v = serde_json::to_value(p).unwrap_or_default();
        v["ev"] = "progress".into();
        self.send(&v);
    }
}

enum Find {
    Words { id: Value, q: String, k: usize, source: Option<String>, vector: bool },
    Like { id: Value, item: i64, k: usize, source: Option<String> },
    Warm { id: Value },
}

pub fn serve(dir: &Path) -> Result<()> {
    let sh = Arc::new(Shared { out: Mutex::new(std::io::stdout()), epoch: AtomicU64::new(0), stop: Arc::new(AtomicBool::new(false)), running: Mutex::new(None), waiting: Mutex::new(vec![]) });
    let mut store = Store::open(dir)?;
    let (jobs, jrx) = mpsc::channel::<PathBuf>();
    let (finds, frx) = mpsc::channel::<Find>();
    let worker = {
        let (sh, dir) = (sh.clone(), dir.to_path_buf());
        std::thread::spawn(move || work(&sh, &dir, jrx))
    };
    let finder = {
        let (sh, dir) = (sh.clone(), dir.to_path_buf());
        std::thread::spawn(move || find(&sh, &dir, frx))
    };
    // what was there before: each folder (and Photos) listed again (new files in, changed ones redone) and its waiting
    // pictures embedded
    for s in store.sources()? {
        if (s.kind == "folder" && Path::new(&s.root).is_dir()) || s.kind == "photos" {
            sh.waiting.lock().unwrap().push(s.root.clone());
            let _ = jobs.send(PathBuf::from(&s.root));
        }
    }
    sh.send(&json!({ "ev": "ready", "home": dir, "model": MODEL }));
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let ask: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                sh.send(&json!({ "ev": "error", "message": format!("not JSON: {e}") }));
                continue;
            }
        };
        let id = ask.get("id").cloned().unwrap_or(Value::Null);
        match answer(&sh, &mut store, &jobs, &finds, &ask, id.clone()) {
            Ok(Some(mut v)) => {
                v["id"] = id;
                sh.send(&v);
            }
            Ok(None) => {}
            Err(e) => sh.send(&json!({ "id": id, "error": format!("{e:#}") })),
        }
    }
    // the feed went away: the searches asked are answered, the batch in hand is written, then out
    drop(finds);
    let _ = finder.join();
    sh.stop.store(true, Ordering::Relaxed);
    sh.waiting.lock().unwrap().clear();
    drop(jobs);
    let _ = worker.join();
    Ok(())
}

fn answer(sh: &Shared, store: &mut Store, jobs: &mpsc::Sender<PathBuf>, finds: &mpsc::Sender<Find>, ask: &Value, id: Value) -> Result<Option<Value>> {
    let s = |k: &str| ask[k].as_str().map(String::from);
    let k = ask.get("k").and_then(Value::as_u64).unwrap_or(60).clamp(1, 2000) as usize;
    match ask.get("op").and_then(Value::as_str).unwrap_or("") {
        "add" => {
            let path = s("path").ok_or_else(|| anyhow!("add needs a path"))?;
            let root = crate::root_of(&path)?;
            let r = root.to_string_lossy().into_owned();
            let going = sh.running.lock().unwrap().as_ref().is_some_and(|(x, _)| *x == r);
            let mut w = sh.waiting.lock().unwrap();
            if !going && !w.contains(&r) {
                w.push(r.clone());
                jobs.send(root).map_err(|_| anyhow!("the indexer stopped"))?;
            }
            Ok(Some(json!({ "ok": true, "root": r })))
        }
        "search" => {
            let q = s("q").filter(|q| !q.trim().is_empty()).ok_or_else(|| anyhow!("search needs words (q)"))?;
            let vector = ask.get("vector").and_then(Value::as_bool).unwrap_or(false);
            finds.send(Find::Words { id, q, k, source: s("source"), vector }).map_err(|_| anyhow!("the searcher stopped"))?;
            Ok(None)
        }
        "rows" => {
            let after = ask.get("after").and_then(Value::as_i64).unwrap_or(-1);
            let limit = ask.get("limit").and_then(Value::as_u64).unwrap_or(500).clamp(1, 5000) as usize;
            Ok(Some(crate::export::rows_after(store, MODEL, after, limit)?))
        }
        "similar" => {
            let item = ask.get("item").and_then(Value::as_i64).ok_or_else(|| anyhow!("similar needs an item"))?;
            finds.send(Find::Like { id, item, k, source: s("source") }).map_err(|_| anyhow!("the searcher stopped"))?;
            Ok(None)
        }
        "warm" => {
            finds.send(Find::Warm { id }).map_err(|_| anyhow!("the searcher stopped"))?;
            Ok(None)
        }
        "status" => {
            let running = sh.running.lock().unwrap().clone().map(|(root, source)| json!({ "root": root, "source": source }));
            let rows = store.counts(None)?.rows;
            Ok(Some(json!({ "sources": store.sources()?, "running": running, "waiting": *sh.waiting.lock().unwrap(), "rows": rows, "home": store.dir, "model": MODEL })))
        }
        "remove" => {
            let which = s("source").or_else(|| s("path")).ok_or_else(|| anyhow!("remove needs a source"))?;
            let canon = dunce::canonicalize(&which).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| which.clone());
            let Some(source) = store.find(&which)?.or(store.find(&canon)?) else { anyhow::bail!("no source {which}") };
            let root: String = store.db.query_row("SELECT root FROM sources WHERE id = ?1", [&source], |r| r.get(0))?;
            sh.waiting.lock().unwrap().retain(|r| *r != root);
            if sh.running.lock().unwrap().as_ref().is_some_and(|(r, s)| *r == root || s.as_deref() == Some(source.as_str())) {
                sh.stop.store(true, Ordering::Relaxed);
            }
            let n = store.remove(&source)?;
            sh.epoch.fetch_add(1, Ordering::Relaxed);
            Ok(Some(json!({ "ok": true, "source": source, "removed": n })))
        }
        "stop" => {
            sh.waiting.lock().unwrap().clear();
            sh.stop.store(true, Ordering::Relaxed);
            Ok(Some(json!({ "ok": true })))
        }
        op => anyhow::bail!("no op {op:?}: add, search, similar, warm, status, remove, stop, rows"),
    }
}

/// The worker: one folder at a time, listed then embedded, the Neural Engine model loaded on its first picture
fn work(sh: &Shared, dir: &Path, jobs: mpsc::Receiver<PathBuf>) {
    let mut store = match Store::open(dir) {
        Ok(s) => s,
        Err(e) => return sh.send(&json!({ "ev": "error", "message": format!("the index at {}: {e:#}", dir.display()) })),
    };
    #[cfg(target_vendor = "apple")]
    let mut model: Option<crate::apple::Model> = None;
    for root in jobs {
        let r = root.to_string_lossy().into_owned();
        {
            let mut w = sh.waiting.lock().unwrap();
            let Some(i) = w.iter().position(|x| *x == r) else { continue };
            w.remove(i);
            *sh.running.lock().unwrap() = Some((r.clone(), None));
            sh.stop.store(false, Ordering::Relaxed);
        }
        let done = (|| -> Result<()> {
            let l = crate::list_root(&mut store, &root, &mut |p| sh.progress(p))?;
            if l.changed > 0 {
                sh.epoch.fetch_add(1, Ordering::Relaxed);
            }
            *sh.running.lock().unwrap() = Some((r.clone(), Some(l.source.clone())));
            if sh.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            #[cfg(target_vendor = "apple")]
            {
                if model.is_none() {
                    model = Some(crate::apple::Model::load(&crate::image_package()).map_err(anyhow::Error::msg)?);
                }
                crate::embed_pictures(&mut store, model.as_ref().expect("loaded"), &l.source, &sh.stop, &mut |p| sh.progress(p))?;
                Ok(())
            }
            #[cfg(not(target_vendor = "apple"))]
            anyhow::bail!("the fast pass runs on a Mac for now")
        })();
        if let Err(e) = done {
            sh.send(&json!({ "ev": "error", "root": r, "message": format!("{e:#}") }));
        }
        *sh.running.lock().unwrap() = None;
    }
}

/// The searcher: the text tower loaded once, the rows grown from the file's tail before each search
fn find(sh: &Shared, dir: &Path, finds: mpsc::Receiver<Find>) {
    let store = match Store::open(dir) {
        Ok(s) => s,
        Err(e) => return sh.send(&json!({ "ev": "error", "message": format!("the index at {}: {e:#}", dir.display()) })),
    };
    let mut text: Option<Text> = None;
    let mut rows = Rows::default();
    let mut seen = u64::MAX;
    let tower = |text: &mut Option<Text>| -> Result<f64> {
        if text.is_some() {
            return Ok(0.0);
        }
        let t = Instant::now();
        *text = Some(Text::load(&crate::siglip_dir())?);
        Ok(t.elapsed().as_secs_f64() * 1e3)
    };
    for f in finds {
        let id = match &f {
            Find::Words { id, .. } | Find::Like { id, .. } | Find::Warm { id } => id.clone(),
        };
        let r = (|| -> Result<Value> {
            let e = sh.epoch.load(Ordering::Relaxed);
            if e != seen {
                rows = Rows::load(&store, MODEL)?;
                seen = e;
            } else {
                rows.grow(&store, MODEL)?;
            }
            match f {
                Find::Warm { .. } => Ok(json!({ "ok": true, "load_ms": tower(&mut text)?, "rows": rows.live() })),
                Find::Words { q, k, source, vector, .. } => {
                    let load = tower(&mut text)?;
                    let t = Instant::now();
                    let v = text.as_mut().expect("loaded").embed(&q)?;
                    let words = t.elapsed().as_secs_f64() * 1e3;
                    let hits = search::search(&store, &rows, &v, k, source.as_deref())?;
                    let mut out = json!({ "hits": hits, "rows": rows.live(), "ms": { "load": load, "words": words, "search": t.elapsed().as_secs_f64() * 1e3 - words } });
                    if vector {
                        out["vector"] = crate::export::vector_b64(&v).into();
                    }
                    Ok(out)
                }
                Find::Like { item, k, source, .. } => {
                    let v = rows.vector(item).ok_or_else(|| anyhow!("item {item} isn't searchable yet"))?;
                    let t = Instant::now();
                    let hits: Vec<search::Hit> = search::search(&store, &rows, &v, k + 1, source.as_deref())?.into_iter().filter(|h| h.item != item).take(k).collect();
                    Ok(json!({ "hits": hits, "rows": rows.live(), "ms": { "search": t.elapsed().as_secs_f64() * 1e3 } }))
                }
            }
        })();
        match r {
            Ok(mut v) => {
                v["id"] = id;
                sh.send(&v);
            }
            Err(e) => sh.send(&json!({ "id": id, "error": format!("{e:#}") })),
        }
    }
}
