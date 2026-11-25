//! `raea index` (docs/index.feature IX13): the fast pass from a terminal, and the server the feed keeps open.

use crate::search::{self, Rows};
use crate::store::{MODEL, Store};
use crate::text::Text;
use anyhow::{Result, anyhow};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub const USAGE: &str = "raea index add FOLDER|photos:... [--json] | raea index search WORDS... [--k N] [--source ID|FOLDER] [--json] | raea index status [--json] | raea index remove ID|FOLDER | raea index serve   [--home DIR]   (every picture and video on a folder, a drive or Apple Photos (photos:) made searchable by words on this Mac, fast: add lists it and embeds each picture's small copy with SigLIP 2 on the Neural Engine, a counter as it goes, JSON lines with --json; search gives the best for the words; status lists the sources and what each holds; remove takes one out; serve keeps the index open for the raea app, one JSON ask a line on stdin; the index lives in ~/.raea/library/fast, or --home / RAEA_INDEX_HOME)";

/// The exit code: 0, or 1 with the reason said
pub fn cli(args: &[String]) -> i32 {
    let Err(e) = run(args) else { return 0 };
    eprintln!("raea index: {e:#}");
    1
}

struct Flags {
    json: bool,
    k: usize,
    source: Option<String>,
    rest: Vec<String>,
}

fn flags(args: &[String]) -> Result<(Flags, PathBuf)> {
    let mut f = Flags { json: false, k: 10, source: None, rest: vec![] };
    let mut home = Store::default_dir();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| anyhow!("{name} needs a value"));
        match a.as_str() {
            "--json" => f.json = true,
            "--k" => f.k = value("--k")?.parse().map_err(|_| anyhow!("--k takes a number"))?,
            "--source" => f.source = Some(value("--source")?),
            "--home" => home = PathBuf::from(value("--home")?),
            _ => f.rest.push(a.clone()),
        }
    }
    Ok((f, home))
}

fn run(args: &[String]) -> Result<()> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    let (f, home) = flags(args.get(1..).unwrap_or(&[]))?;
    match verb {
        "serve" => crate::serve::serve(&home),
        "add" => add(&f, &home),
        "search" => find(&f, &home),
        "status" => status(&f, &home),
        "remove" => {
            let which = f.rest.first().ok_or_else(|| anyhow!("remove takes a source id or the folder"))?;
            let mut store = Store::open(&home)?;
            let canon = dunce::canonicalize(which).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| which.clone());
            let source = store.find(which)?.or(store.find(&canon)?).ok_or_else(|| anyhow!("no source {which}"))?;
            let n = store.remove(&source)?;
            println!("removed {source}: {n} items");
            Ok(())
        }
        _ => Err(anyhow!("usage: {USAGE}")),
    }
}

fn add(f: &Flags, home: &std::path::Path) -> Result<()> {
    if f.rest.is_empty() {
        anyhow::bail!("add takes a folder or a drive");
    }
    let mut store = Store::open(home)?;
    let tty = std::io::stderr().is_terminal() && !f.json;
    let say = |p: &crate::Progress| {
        if f.json {
            println!("{}", serde_json::to_string(p).unwrap_or_default());
        } else if tty {
            match p.stage {
                "listing" => eprint!("\r\x1b[2Kfound {}", p.found),
                _ => eprint!("\r\x1b[2K{} of {} searchable · {:.0} a second", p.done, p.found, p.per_sec),
            }
        }
    };
    #[cfg(target_vendor = "apple")]
    let mut model: Option<crate::apple::Model> = None;
    for path in &f.rest {
        let root = crate::root_of(path)?;
        let l = crate::list_root(&mut store, &root, &mut |p| say(p))?;
        #[cfg(target_vendor = "apple")]
        {
            if model.is_none() {
                let pkg = crate::image_package();
                model = Some(crate::apple::Model::load(&pkg).map_err(|e| anyhow!("SigLIP 2's picture model ({e})"))?);
            }
            let stop = Arc::new(AtomicBool::new(false));
            let p = crate::embed_pictures(&mut store, model.as_ref().expect("loaded"), &l.source, &stop, &mut |p| say(p))?;
            if tty {
                eprint!("\r\x1b[2K");
            }
            if !f.json {
                println!("{}: {} found ({} new), {} searchable, {} failed; {:.0} a second over {:.1} s", root.display(), l.found, l.added, p.done, p.failed, p.per_sec, p.seconds);
                if l.cloud > 0 {
                    println!("  {} only in the cloud (not iCloud), left out: reading them would download each original", l.cloud);
                }
            }
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            let _ = (&l, Arc::new(AtomicBool::new(false)));
            anyhow::bail!("listed {} in {}; embedding runs on a Mac for now", l.found, root.display());
        }
    }
    Ok(())
}

fn find(f: &Flags, home: &std::path::Path) -> Result<()> {
    let words = f.rest.join(" ");
    if words.trim().is_empty() {
        anyhow::bail!("search takes words");
    }
    let store = Store::open(home)?;
    let source = match &f.source {
        Some(s) => {
            let canon = dunce::canonicalize(s).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| s.clone());
            Some(store.find(s)?.or(store.find(&canon)?).ok_or_else(|| anyhow!("no source {s}"))?)
        }
        None => None,
    };
    let mut text = Text::load(&crate::siglip_dir())?;
    let rows = Rows::load(&store, MODEL)?;
    let q = text.embed(&words)?;
    let hits = search::search(&store, &rows, &q, f.k, source.as_deref())?;
    if f.json {
        println!("{}", serde_json::to_string(&hits)?);
        return Ok(());
    }
    if hits.is_empty() {
        eprintln!("nothing searchable yet: raea index add FOLDER");
    }
    for h in hits {
        let at = if h.kind == "video" { format!(" at {:.1} s", h.t) } else { String::new() };
        println!("{:.3}  {}{at}", h.score, h.locator);
    }
    Ok(())
}

fn status(f: &Flags, home: &std::path::Path) -> Result<()> {
    let store = Store::open(home)?;
    let sources = store.sources()?;
    let rows = store.counts(None)?.rows;
    if f.json {
        println!("{}", serde_json::json!({ "home": home, "model": MODEL, "rows": rows, "sources": sources }));
        return Ok(());
    }
    println!("{} ({MODEL}, {rows} rows)", home.display());
    if sources.is_empty() {
        println!("  nothing yet: raea index add FOLDER");
    }
    for s in sources {
        println!("  {}  {}  {} found, {} searchable, {} failed", s.id, s.root, s.found, s.done, s.failed);
    }
    Ok(())
}
