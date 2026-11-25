//! Searching the fast pass (docs/index.feature IX3): a query vector against every row of the model's file, brute
//! force over f16 on every core, best `k` kept, one hit per item (its best moment). Rows of removed or changed items
//! are left out. A long-lived searcher grows its rows from the file's tail as batches land, and reads them all again
//! only after something was removed or changed.

use crate::store::{DIM, Store};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Hit {
    pub item: i64,
    pub score: f32,
    /// the moment in a video, seconds; 0 for a picture
    pub t: f64,
    pub source: String,
    pub kind: String,
    pub name: String,
    pub locator: String,
    pub thumb: String,
}

/// The rows' vectors as f16 bits, and which item, moment and source each row is (item -1: out of search)
#[derive(Default)]
pub struct Rows {
    pub bits: Vec<u16>,
    pub item: Vec<i64>,
    pub t: Vec<f64>,
    src: Vec<u16>,
    sources: Vec<String>,
}

impl Rows {
    pub fn load(store: &Store, model: &str) -> Result<Rows> {
        let mut r = Rows::default();
        r.grow(store, model)?;
        Ok(r)
    }

    pub fn len(&self) -> usize {
        self.item.len()
    }

    pub fn is_empty(&self) -> bool {
        self.item.is_empty()
    }

    /// Reads the rows written since the last read: the file's tail, up to the last row the list already knows (a
    /// batch's vectors land in the file just before its rows land in the list). Returns how many it added.
    pub fn grow(&mut self, store: &Store, model: &str) -> Result<usize> {
        let have = self.len();
        let known: Option<i64> = store.db.query_row("SELECT MAX(row) FROM rows WHERE model = ?1", [model], |r| r.get(0))?;
        let file = store.vectors(model);
        let on_disk = std::fs::metadata(&file).map(|m| m.len() as usize / (DIM * 2)).unwrap_or(0);
        let to = on_disk.min(known.map(|k| k as usize + 1).unwrap_or(0));
        if to <= have {
            return Ok(0);
        }
        let mut raw = vec![0u8; (to - have) * DIM * 2];
        let mut f = std::fs::File::open(&file)?;
        f.seek(SeekFrom::Start((have * DIM * 2) as u64))?;
        f.read_exact(&mut raw)?;
        self.bits.extend(raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])));
        self.item.resize(to, -1);
        self.t.resize(to, 0.0);
        self.src.resize(to, 0);
        let mut q = store.db.prepare("SELECT r.row, r.item, r.t, i.source FROM rows r JOIN items i ON i.id = r.item WHERE r.model = ?1 AND r.row >= ?2 AND r.row < ?3 AND i.state = 1")?;
        let mut it = q.query(rusqlite::params![model, have as i64, to as i64])?;
        while let Some(r) = it.next()? {
            let row = r.get::<_, i64>(0)? as usize;
            self.item[row] = r.get(1)?;
            self.t[row] = r.get(2)?;
            let s: String = r.get(3)?;
            self.src[row] = match self.sources.iter().position(|x| *x == s) {
                Some(i) => i as u16,
                None => {
                    self.sources.push(s);
                    (self.sources.len() - 1) as u16
                }
            };
        }
        Ok(to - have)
    }

    /// An item's vector (its first row: a picture's only one), to find what looks like it
    pub fn vector(&self, item: i64) -> Option<Vec<f32>> {
        let i = self.item.iter().position(|&x| x == item)?;
        Some(self.bits[i * DIM..(i + 1) * DIM].iter().map(|&b| half::f16::from_bits(b).to_f32()).collect())
    }

    /// How many rows are in search
    pub fn live(&self) -> usize {
        self.item.iter().filter(|&&i| i >= 0).count()
    }
}

fn dot(bits: &[u16], q: &[f32]) -> f32 {
    let mut acc = [0f32; 8];
    for (c, qc) in bits.chunks_exact(8).zip(q.chunks_exact(8)) {
        for j in 0..8 {
            acc[j] += half::f16::from_bits(c[j]).to_f32() * qc[j];
        }
    }
    acc.iter().sum()
}

/// The `k` best items for the query, best first; `source` keeps one source's
pub fn search(store: &Store, rows: &Rows, q: &[f32], k: usize, source: Option<&str>) -> Result<Vec<Hit>> {
    let only = match source {
        Some(s) => match rows.sources.iter().position(|x| x == s) {
            Some(i) => Some(i as u16),
            None => return Ok(vec![]),
        },
        None => None,
    };
    let scored: Vec<(usize, f32)> = rows
        .bits
        .par_chunks_exact(DIM)
        .enumerate()
        .filter(|(i, _)| rows.item[*i] >= 0 && only.is_none_or(|s| rows.src[*i] == s))
        .map(|(i, v)| (i, dot(v, q)))
        .collect();
    // best row per item
    let mut best: HashMap<i64, (usize, f32)> = HashMap::new();
    for (i, s) in scored {
        let e = best.entry(rows.item[i]).or_insert((i, f32::MIN));
        if s > e.1 {
            *e = (i, s);
        }
    }
    let mut top: Vec<(i64, usize, f32)> = best.into_iter().map(|(item, (i, s))| (item, i, s)).collect();
    top.sort_by(|a, b| b.2.total_cmp(&a.2));
    top.truncate(k);
    let mut q = store.db.prepare_cached("SELECT kind, name, locator FROM items WHERE id = ?1")?;
    let mut hits = Vec::with_capacity(top.len());
    for (item, i, score) in top {
        // removed since the rows were read
        let Some((kind, name, locator)) = rusqlite::OptionalExtension::optional(q.query_row([item], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))? else { continue };
        let source = rows.sources[rows.src[i] as usize].clone();
        hits.push(Hit { item, score, t: rows.t[i], source, kind, name, locator, thumb: store.thumb(item).to_string_lossy().into_owned() });
    }
    Ok(hits)
}
