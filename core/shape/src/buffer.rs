//! A mesh given by its own vertices: a <BufferMesh>'s shape, "buffer:{positions, indices?, normals?, uvs?}" (what
//! raea/r3f makes of every three.js geometry), read as the triangles a CSG expression's are, so contact,
//! `raea scene lint|explain` and `raea scene export` read it instead of skipping it; and any triangles written out as a
//! mesh file with their normals and texture coordinates. It needn't be closed, so it never goes through Manifold.

use crate::Tris;
use crate::mesh::{bounds, glb};

/// A mesh from its own vertex buffers (<BufferMesh>, or a Mesh handle given as {positions, ...}): JSON
/// {positions: [x, y, z, ...], indices?: [i, ...], normals?: [x, y, z, ...], uvs?: [u, v, ...]}, a triangle list
/// (indexed, or each three positions a triangle), as flat triangles; without normals each face's own. Its texture
/// coordinates when it gave them.
pub fn buffer_tris(json: &str) -> Result<(Tris, Option<Vec<[f32; 2]>>), String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("a buffer mesh isn't JSON: {e}"))?;
    let nums = |k: &str| -> Result<Option<Vec<f32>>, String> {
        match v.get(k) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Array(a)) => a.iter().map(|x| x.as_f64().map(|f| f as f32).ok_or_else(|| format!("a buffer mesh's {k}: numbers, got {x}"))).collect::<Result<Vec<_>, _>>().map(Some),
            Some(x) => Err(format!("a buffer mesh's {k}: an array of numbers, got {x}")),
        }
    };
    let pos = nums("positions")?.ok_or("a buffer mesh needs positions: [x, y, z, ...]")?;
    if pos.len() % 3 != 0 || pos.is_empty() {
        return Err(format!("a buffer mesh's positions: x, y, z for each vertex, got {} numbers", pos.len()));
    }
    let n = pos.len() / 3;
    let idx: Vec<usize> = match nums("indices")? {
        Some(i) => i.iter().map(|x| *x as usize).collect(),
        None => (0..n).collect(),
    };
    if idx.len() % 3 != 0 || idx.iter().any(|&i| i >= n) {
        return Err(format!("a buffer mesh's indices: three a triangle, each under {n} (the vertices), got {}", idx.len()));
    }
    let normals = nums("normals")?;
    let uvs = nums("uvs")?;
    if normals.as_ref().is_some_and(|x| x.len() != pos.len()) || uvs.as_ref().is_some_and(|u| u.len() != n * 2) {
        return Err("a buffer mesh's normals are x, y, z and its uvs u, v for each vertex".into());
    }
    let p3 = |i: usize| [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
    let mut t = Tris { positions: Vec::with_capacity(idx.len()), normals: Vec::with_capacity(idx.len()) };
    let mut uv = uvs.as_ref().map(|_| Vec::with_capacity(idx.len()));
    for tri in idx.chunks(3) {
        let (a, b, c) = (p3(tri[0]), p3(tri[1]), p3(tri[2]));
        let face = face_normal(a, b, c);
        for &i in tri {
            t.positions.push(p3(i));
            t.normals.push(match &normals {
                Some(nr) => [nr[i * 3], nr[i * 3 + 1], nr[i * 3 + 2]],
                None => face,
            });
            if let (Some(out), Some(u)) = (uv.as_mut(), uvs.as_ref()) {
                out.push([u[i * 2], u[i * 2 + 1]]);
            }
        }
    }
    Ok((t, uv))
}

/// A triangle's unit normal (counter-clockwise front), zero for a triangle of no area.
fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (u, w) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    let n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 1e-12 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0; 3] }
}

/// Triangles written as OBJ ("obj") or glTF binary ("glb"), three vertices a triangle, with their normals and, when
/// given, texture coordinates (glTF's and raea's v down; OBJ's v up).
pub fn export_tris(t: &Tris, uvs: Option<&[[f32; 2]]>, path: &str, format: &str) -> Result<usize, String> {
    let uvs = uvs.filter(|u| u.len() == t.positions.len());
    let bytes = match format {
        "obj" => {
            let mut s = String::from("# raea mesh\n");
            for (p, n) in t.positions.iter().zip(&t.normals) {
                s += &format!("v {} {} {}\nvn {} {} {}\n", p[0], p[1], p[2], n[0], n[1], n[2]);
            }
            for u in uvs.unwrap_or(&[]) {
                s += &format!("vt {} {}\n", u[0], 1.0 - u[1]);
            }
            for k in (1..=t.positions.len()).step_by(3) {
                let c = |i: usize| if uvs.is_some() { format!("{i}/{i}/{i}") } else { format!("{i}//{i}") };
                s += &format!("f {} {} {}\n", c(k), c(k + 1), c(k + 2));
            }
            s.into_bytes()
        }
        "glb" => {
            let mut bin = Vec::new();
            let mut views = Vec::new();
            let mut view = |bin: &mut Vec<u8>, data: &mut dyn Iterator<Item = f32>, target: u32| {
                let at = bin.len();
                data.for_each(|x| bin.extend_from_slice(&x.to_le_bytes()));
                views.push(serde_json::json!({"buffer": 0, "byteOffset": at, "byteLength": bin.len() - at, "target": target}));
            };
            view(&mut bin, &mut t.positions.iter().flatten().copied(), 34962);
            view(&mut bin, &mut t.normals.iter().flatten().copied(), 34962);
            if let Some(u) = uvs {
                view(&mut bin, &mut u.iter().flatten().copied(), 34962);
            }
            // sequential indices, so a reader that wants them (read_glb, Godot) has them
            let at = bin.len();
            (0..t.positions.len() as u32).for_each(|i| bin.extend_from_slice(&i.to_le_bytes()));
            views.push(serde_json::json!({"buffer": 0, "byteOffset": at, "byteLength": bin.len() - at, "target": 34963}));
            let (lo, hi) = bounds(t.positions.iter().copied());
            let count = t.positions.len();
            let mut accessors = vec![
                serde_json::json!({"bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3", "min": lo, "max": hi}),
                serde_json::json!({"bufferView": 1, "componentType": 5126, "count": count, "type": "VEC3"}),
            ];
            let mut attributes = serde_json::json!({"POSITION": 0, "NORMAL": 1});
            if uvs.is_some() {
                accessors.push(serde_json::json!({"bufferView": 2, "componentType": 5126, "count": count, "type": "VEC2"}));
                attributes["TEXCOORD_0"] = serde_json::json!(2);
            }
            accessors.push(serde_json::json!({"bufferView": views.len() - 1, "componentType": 5125, "count": count, "type": "SCALAR"}));
            let json = serde_json::json!({
                "asset": {"version": "2.0", "generator": "raea"},
                "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
                "meshes": [{"primitives": [{"attributes": attributes, "indices": accessors.len() - 1}]}],
                "buffers": [{"byteLength": bin.len()}],
                "bufferViews": views,
                "accessors": accessors
            });
            glb(json, bin)?
        }
        f => return Err(format!("export: {f}? (obj, glb)")),
    };
    std::fs::write(path, &bytes).map_err(|e| format!("{path}: {e}"))?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // a quad of two triangles, open (no Manifold solid), with uvs
    const QUAD: &str = r#"{"positions": [0,0,0, 1,0,0, 1,1,0, 0,1,0], "indices": [0,1,2, 0,2,3], "uvs": [0,1, 1,1, 1,0, 0,0]}"#;

    #[test]
    fn a_buffer_mesh_reads_as_its_triangles() {
        let (t, uv) = buffer_tris(QUAD).unwrap();
        assert_eq!(t.positions.len(), 6);
        assert_eq!(t.normals[0], [0.0, 0.0, 1.0]);
        assert_eq!(uv.unwrap()[2], [1.0, 0.0]);
        // and through the shape cache, as a CSG expression would be
        assert_eq!(crate::cached(&format!("buffer:{QUAD}")).unwrap().positions.len(), 6);
        assert!(buffer_tris(r#"{"positions": [0,0,0, 1,0,0, 1,1,0], "indices": [0,1,7]}"#).is_err());
    }

    #[test]
    fn triangles_write_as_a_glb_and_an_obj_that_read_back() {
        let (t, uv) = buffer_tris(QUAD).unwrap();
        let dir = std::env::temp_dir().join(format!("raea-shape-buffer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (g, o) = (dir.join("q.glb"), dir.join("q.obj"));
        export_tris(&t, uv.as_deref(), &g.to_string_lossy(), "glb").unwrap();
        export_tris(&t, uv.as_deref(), &o.to_string_lossy(), "obj").unwrap();
        let (v, i) = crate::read_glb(&g.to_string_lossy()).unwrap();
        assert_eq!((v.len(), i.len()), (18, 6));
        let (v, i) = crate::read_obj(&o.to_string_lossy()).unwrap();
        assert_eq!((v.len(), i.len()), (18, 6));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
