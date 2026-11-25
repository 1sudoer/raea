//! The .glb container: its JSON and binary chunks, accessors read as bytes, floats or indices (strides, component
//! types and normalized integers handled; sparse accessors refused), and the file written back with only the
//! accessors and buffer views still in use.

use serde_json::{Value, json};

/// An error is a line for the person.
type R<T> = Result<T, String>;

pub struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}
pub fn parse(b: &[u8]) -> R<Glb> {
    if b.len() < 20 || &b[0..4] != b"glTF" {
        return Err("not a .glb (a .gltf with outside files isn't taken: pack it to .glb first)".into());
    }
    let jl = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(b.get(20..20 + jl).ok_or("truncated JSON chunk")?).map_err(|e| e.to_string())?;
    let at = 20 + jl;
    let bin = if b.len() >= at + 8 {
        let bl = u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
        b.get(at + 8..at + 8 + bl).ok_or("truncated BIN chunk")?.to_vec()
    } else {
        Vec::new()
    };
    Ok(Glb { json, bin })
}

pub fn arr<'a>(j: &'a Value, k: &str) -> &'a [Value] {
    j.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// An accessor's elements as tight bytes, and its layout (components a vertex, bytes a component).
pub fn raw(g: &Glb, i: usize) -> R<(Vec<u8>, usize, usize, &Value)> {
    let a = arr(&g.json, "accessors").get(i).ok_or_else(|| format!("accessor {i} missing"))?;
    if a.get("sparse").is_some() {
        return Err(format!("accessor {i}: sparse accessors aren't taken"));
    }
    let comps = match a["type"].as_str().unwrap_or("") {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT4" => 16,
        t => return Err(format!("accessor {i}: type {t}")),
    };
    let csize = match a["componentType"].as_u64().unwrap_or(0) {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        c => return Err(format!("accessor {i}: component type {c}")),
    };
    let count = a["count"].as_u64().unwrap_or(0) as usize;
    let el = comps * csize;
    let Some(v) = a.get("bufferView").and_then(Value::as_u64) else { return Ok((vec![0; count * el], comps, csize, a)) };
    let bv = &arr(&g.json, "bufferViews")[v as usize];
    if bv["buffer"].as_u64().unwrap_or(0) != 0 {
        return Err(format!("accessor {i}: only the .glb's own buffer is read"));
    }
    let start = bv["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
    let stride = bv["byteStride"].as_u64().map(|s| s as usize).unwrap_or(el);
    let mut out = Vec::with_capacity(count * el);
    for k in 0..count {
        let s = start + k * stride;
        out.extend_from_slice(g.bin.get(s..s + el).ok_or_else(|| format!("accessor {i} runs past the buffer"))?);
    }
    Ok((out, comps, csize, a))
}

/// An accessor as floats (normalized integers scaled to 0-1 or -1-1, others as they are).
pub fn floats(g: &Glb, i: usize) -> R<(Vec<f32>, usize)> {
    let (b, comps, csize, a) = raw(g, i)?;
    let norm = a["normalized"].as_bool().unwrap_or(false);
    let ct = a["componentType"].as_u64().unwrap_or(0);
    let out = b
        .chunks_exact(csize)
        .map(|c| match ct {
            5126 => f32::from_le_bytes(c.try_into().unwrap()),
            5121 => c[0] as f32 / if norm { 255.0 } else { 1.0 },
            5120 => (c[0] as i8) as f32 / if norm { 127.0 } else { 1.0 },
            5123 => u16::from_le_bytes(c.try_into().unwrap()) as f32 / if norm { 65535.0 } else { 1.0 },
            5122 => i16::from_le_bytes(c.try_into().unwrap()) as f32 / if norm { 32767.0 } else { 1.0 },
            _ => u32::from_le_bytes(c.try_into().unwrap()) as f32,
        })
        .collect();
    Ok((out, comps))
}

pub fn indices(g: &Glb, i: usize) -> R<Vec<u32>> {
    let (b, _, csize, _) = raw(g, i)?;
    Ok(match csize {
        1 => b.iter().map(|&x| x as u32).collect(),
        2 => b.chunks_exact(2).map(|c| u32::from(u16::from_le_bytes([c[0], c[1]]))).collect(),
        _ => b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect(),
    })
}
/// Drop the accessors and views nothing uses any more (the primitives' old data), renumbering what's kept.
pub fn compact(j: &mut Value, bin: &[u8]) -> R<Vec<u8>> {
    let mut acc_used = vec![false; arr(j, "accessors").len()];
    let mark = |v: &Value, used: &mut Vec<bool>| {
        if let Some(i) = v.as_u64() {
            used[i as usize] = true;
        }
    };
    for m in arr(j, "meshes") {
        for p in arr(m, "primitives") {
            for a in p["attributes"].as_object().into_iter().flat_map(|o| o.values()) {
                mark(a, &mut acc_used);
            }
            mark(&p["indices"], &mut acc_used);
        }
    }
    for an in arr(j, "animations") {
        for s in arr(an, "samplers") {
            mark(&s["input"], &mut acc_used);
            mark(&s["output"], &mut acc_used);
        }
    }
    let acc_new: Vec<Option<usize>> = acc_used.iter().scan(0, |n, &u| Some(if u { *n += 1; Some(*n - 1) } else { None })).collect();
    let mut view_used = vec![false; arr(j, "bufferViews").len()];
    for (i, a) in arr(j, "accessors").iter().enumerate() {
        if acc_used[i] {
            mark(&a["bufferView"], &mut view_used);
        }
    }
    for im in arr(j, "images") {
        mark(&im["bufferView"], &mut view_used);
    }
    let mut nbin = Vec::new();
    let mut view_new = vec![None; view_used.len()];
    let mut views = Vec::new();
    for (i, v) in arr(j, "bufferViews").iter().enumerate() {
        if !view_used[i] {
            continue;
        }
        while nbin.len() % 4 != 0 {
            nbin.push(0);
        }
        let (o, l) = (v["byteOffset"].as_u64().unwrap_or(0) as usize, v["byteLength"].as_u64().unwrap_or(0) as usize);
        let mut nv = v.clone();
        nv["byteOffset"] = json!(nbin.len());
        nbin.extend_from_slice(bin.get(o..o + l).ok_or("a buffer view runs past the buffer")?);
        view_new[i] = Some(views.len());
        views.push(nv);
    }
    let remap = |v: &mut Value, table: &[Option<usize>]| {
        if let Some(i) = v.as_u64() {
            *v = json!(table[i as usize]);
        }
    };
    let accs: Vec<Value> = arr(j, "accessors")
        .iter()
        .enumerate()
        .filter(|(i, _)| acc_used[*i])
        .map(|(_, a)| {
            let mut a = a.clone();
            remap(&mut a["bufferView"], &view_new);
            a
        })
        .collect();
    for m in j.get_mut("meshes").and_then(Value::as_array_mut).into_iter().flatten() {
        for p in m.get_mut("primitives").and_then(Value::as_array_mut).into_iter().flatten() {
            for a in p["attributes"].as_object_mut().into_iter().flat_map(|o| o.values_mut()) {
                remap(a, &acc_new);
            }
            remap(&mut p["indices"], &acc_new);
        }
    }
    for an in j.get_mut("animations").and_then(Value::as_array_mut).into_iter().flatten() {
        for s in an.get_mut("samplers").and_then(Value::as_array_mut).into_iter().flatten() {
            remap(&mut s["input"], &acc_new);
            remap(&mut s["output"], &acc_new);
        }
    }
    for im in j.get_mut("images").and_then(Value::as_array_mut).into_iter().flatten() {
        if im.get("bufferView").is_some() {
            remap(&mut im["bufferView"], &view_new);
        }
    }
    j["accessors"] = json!(accs);
    j["bufferViews"] = json!(views);
    j["buffers"] = json!([{"byteLength": nbin.len()}]);
    Ok(glb_bytes(j, &nbin))
}

pub fn glb_bytes(j: &Value, bin: &[u8]) -> Vec<u8> {
    let mut js = j.to_string().into_bytes();
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let mut b = bin.to_vec();
    while b.len() % 4 != 0 {
        b.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + b.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&b);
    out
}
