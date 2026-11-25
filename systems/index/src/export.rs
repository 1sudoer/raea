//! The fast pass's rows handed out (docs/index.feature IX10): on Pro and Team the feed copies each row (its vector,
//! the item it is and its thumbnail) to raea's cloud so the account's other Macs search it, and a search hands back its
//! query's vector so the same words can be asked of the cloud's rows. Vectors travel as base64 of their f16 bits, little
//! endian, as the rows' file keeps them (2 KB a row).

use crate::store::{DIM, Store};
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding
pub fn b64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { B64[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// A query's vector as the rows' file would keep it
pub fn vector_b64(v: &[f32]) -> String {
    let bytes: Vec<u8> = v.iter().flat_map(|&x| half::f16::from_f32(x).to_bits().to_le_bytes()).collect();
    b64(&bytes)
}

/// The searchable rows after row `after`, oldest first, at most `limit`: each row's number, its item (kind, name,
/// where it lives, its source, when it was taken, its size in pixels), its moment and its vector. `next` is the last
/// row handed out (or `after` when there were none), the place to ask from next. A changed item's new rows come after
/// its old ones, so a reader that keys rows by item and moment replaces them.
pub fn rows_after(store: &Store, model: &str, after: i64, limit: usize) -> Result<Value> {
    let mut q = store.db.prepare_cached(
        "SELECT r.row, r.item, r.t, i.source, i.kind, i.name, i.locator, i.taken, i.w, i.h FROM rows r JOIN items i ON i.id = r.item
         WHERE r.model = ?1 AND r.row > ?2 AND i.state = 1 ORDER BY r.row LIMIT ?3",
    )?;
    let found: Vec<(i64, i64, f64, String, String, String, String, Option<i64>, Option<i64>, Option<i64>)> = q
        .query_map(rusqlite::params![model, after, limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut file = if found.is_empty() { None } else { Some(std::fs::File::open(store.vectors(model))?) };
    let mut raw = vec![0u8; DIM * 2];
    let mut rows = Vec::with_capacity(found.len());
    for (row, item, t, source, kind, name, locator, taken, w, h) in &found {
        let f = file.as_mut().expect("opened when rows were found");
        f.seek(SeekFrom::Start(*row as u64 * (DIM * 2) as u64))?;
        f.read_exact(&mut raw)?;
        rows.push(json!({ "row": row, "item": item, "t": t, "source": source, "kind": kind, "name": name, "locator": locator,
            "taken": taken, "w": w, "h": h, "thumb": store.thumb(*item), "vector": b64(&raw) }));
    }
    let next = found.last().map(|r| r.0).unwrap_or(after);
    Ok(json!({ "rows": rows, "next": next, "model": model }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_as_the_standard_writes_it() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn a_query_vector_goes_out_as_f16_bits() {
        // 1.0 is f16 0x3C00, little endian 00 3C; -2.0 is 0xC000
        assert_eq!(vector_b64(&[1.0, -2.0]), b64(&[0x00, 0x3c, 0x00, 0xc0]));
    }

    #[test]
    fn rows_come_out_with_their_items_and_vectors_from_where_they_left_off() {
        let dir = std::env::temp_dir().join(format!("raea-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(&dir).unwrap();
        let src = store.source("folder", "/pics", "pics").unwrap();
        store.db.execute("INSERT INTO items (id, source, locator, kind, name, size, mtime, state) VALUES (7, ?1, '/pics/a.jpg', 'picture', 'a.jpg', 2000, 1, 1), (8, ?1, '/pics/b.mov', 'video', 'b.mov', 9000, 1, 1), (9, ?1, '/pics/c.jpg', 'picture', 'c.jpg', 2000, 1, 0)", [&src]).unwrap();
        store.db.execute("INSERT INTO rows (model, row, item, t) VALUES ('m', 0, 7, 0), ('m', 1, 8, 0), ('m', 2, 8, 4), ('m', 3, 9, 0)", []).unwrap();
        let mut file = Vec::new();
        for r in 0..4u16 {
            file.extend((0..DIM).flat_map(|_| (0x3c00 + r).to_le_bytes()));
        }
        std::fs::write(store.vectors("m"), &file).unwrap();
        let first = rows_after(&store, "m", -1, 2).unwrap();
        assert_eq!(first["next"], 1);
        assert_eq!(first["rows"][0]["name"], "a.jpg");
        assert_eq!(first["rows"][1]["kind"], "video");
        assert_eq!(first["rows"][0]["vector"].as_str().unwrap(), b64(&file[..DIM * 2]));
        // the rest: row 2 (b.mov at 4 s); row 3's item isn't searchable yet, so it waits
        let rest = rows_after(&store, "m", 1, 10).unwrap();
        assert_eq!(rest["rows"].as_array().unwrap().len(), 1);
        assert_eq!(rest["rows"][0]["t"], 4.0);
        assert_eq!(rest["rows"][0]["vector"].as_str().unwrap(), b64(&file[2 * DIM * 2..3 * DIM * 2]));
        assert_eq!(rest["next"], 2);
        assert_eq!(rows_after(&store, "m", 2, 10).unwrap()["next"], 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
