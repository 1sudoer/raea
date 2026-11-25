//! The call on a small scene: a 3 m wall standing on a 6 m floor in a project's assets/, found by its media path,
//! baked into the project's store; the same request comes back from the store; a missing src says where it looked;
//! the verb writes its answer where -o says.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{bake_in, cli};

/// The runtime's rule for /media/x, cut down: the project's assets/x.
fn media(dir: &Path, src: &str) -> PathBuf {
    match src.strip_prefix("/media/") {
        Some(rest) => dir.join("assets").join(rest),
        None => dir.join(src),
    }
}

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("raea-wear-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("assets/corner.glb"), corner()).unwrap();
    dir
}

/// A wall (x 0-6, y 0-3 at z 0, facing +z) of "facade" and a floor (z 0-4) of "ground".
fn corner() -> Vec<u8> {
    let quads: [([f32; 3], [f32; 3], [f32; 3], [f32; 3]); 2] = [([0.0, 0.0, 0.0], [6.0, 0.0, 0.0], [6.0, 3.0, 0.0], [0.0, 3.0, 0.0]), ([0.0, 0.0, 4.0], [6.0, 0.0, 4.0], [6.0, 0.0, 0.0], [0.0, 0.0, 0.0])];
    let mut bin = Vec::new();
    let (mut views, mut accessors, mut prims) = (Vec::new(), Vec::new(), Vec::new());
    for (k, (a, b, c, d)) in quads.iter().enumerate() {
        let pos: Vec<f32> = [a, b, c, d].iter().flat_map(|p| p.iter().copied()).collect();
        let off = bin.len();
        bin.extend(pos.iter().flat_map(|f| f.to_le_bytes()));
        views.push(json!({ "buffer": 0, "byteOffset": off, "byteLength": 48 }));
        let lo: Vec<f32> = (0..3).map(|i| [a[i], b[i], c[i], d[i]].into_iter().fold(f32::INFINITY, f32::min)).collect();
        let hi: Vec<f32> = (0..3).map(|i| [a[i], b[i], c[i], d[i]].into_iter().fold(f32::NEG_INFINITY, f32::max)).collect();
        accessors.push(json!({ "bufferView": views.len() - 1, "componentType": 5126, "count": 4, "type": "VEC3", "min": lo, "max": hi }));
        let off = bin.len();
        bin.extend([0u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()));
        bin.extend([0u8; 4]);
        views.push(json!({ "buffer": 0, "byteOffset": off, "byteLength": 12 }));
        accessors.push(json!({ "bufferView": views.len() - 1, "componentType": 5123, "count": 6, "type": "SCALAR" }));
        prims.push(json!({ "attributes": { "POSITION": 2 * k }, "indices": 2 * k + 1, "material": k }));
    }
    let j = json!({
        "asset": { "version": "2.0" }, "scene": 0, "scenes": [{ "nodes": [0] }], "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": prims }], "materials": [{ "name": "facade" }, { "name": "ground" }],
        "buffers": [{ "byteLength": bin.len() }], "bufferViews": views, "accessors": accessors,
    });
    raea_sys_glb::glb_bytes(&j, &bin)
}

fn request() -> Value {
    json!({ "src": "/media/corner.glb", "env": { "density": 4, "walk": ["ground"], "rays": { "sky": 8, "cavity": 4 } }, "classes": { "facade": "plaster", "ground": "ground" } })
}

#[test]
fn a_bake_is_made_once_in_the_projects_store() {
    let dir = project("once");
    let a = bake_in(&dir, media, &request()).unwrap();
    let store = dir.join("assets").join("wear");
    let file = PathBuf::from(a["file"].as_str().unwrap());
    assert!(file.starts_with(&store) && file.is_file(), "the model in the project's store: {file:?}");
    for m in ["causes0", "causes1", "causes2", "causes3"] {
        assert!(Path::new(a["maps"][m].as_str().unwrap()).is_file(), "{m}");
    }
    let b = bake_in(&dir, media, &request()).unwrap();
    assert_eq!(a["key"], b["key"]);
    assert_eq!(a, b, "the second call reads the first's answer");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_src_says_where_it_looked() {
    let dir = project("missing");
    let e = bake_in(&dir, media, &json!({ "src": "/media/nope.glb" })).unwrap_err();
    assert!(e.contains("nope.glb") && e.contains("no file"), "{e}");
    assert!(bake_in(&dir, media, &json!({})).unwrap_err().contains("src"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_verb_writes_its_answer() {
    let dir = project("verb");
    std::fs::write(dir.join("req.json"), request().to_string()).unwrap();
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let got = cli(&["wear".into(), "bake".into(), "req.json".into(), "-o".into(), "out.json".into()], media);
    std::env::set_current_dir(here).unwrap();
    assert_eq!(got.unwrap(), "out.json");
    let out: Value = serde_json::from_slice(&std::fs::read(dir.join("out.json")).unwrap()).unwrap();
    assert!(out["file"].as_str().unwrap().ends_with("model.glb"));
    assert!(cli(&["wear".into(), "bake".into()], media).unwrap_err().contains("raea wear bake"));
    let _ = std::fs::remove_dir_all(&dir);
}
