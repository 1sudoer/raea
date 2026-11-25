//! A node material's storage buffers and copies (docs/r3f.feature R3F22). The buffers: up to two shared buffers it reads
//! (`tsl_s0`, `tsl_s1`, which the SDK names in its WGSL and lists in the spec's wgsl `storage`: [{name, key, size, f32?}]),
//! bound at 11 and 12 read-only, the same buffers by key the computes write (raea-sys-compute shared.rs); the empty
//! one where it reads fewer. The copies: a BufferMesh with `copies` ({count, attributes?: {NAME: {size, values}},
//! matrices?}) is its geometry repeated once a copy (an InstancedMesh as three draws it, one draw), each copy's number
//! its own attribute `tsl_copy`, a per-copy attribute on each of the copy's vertices, its matrix baked into its
//! positions and normals. A material reading a buffer isn't culled by its mesh's bounds: what the computes write moves it.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use raea_sys_shader::shared::{first_contents, SharedBuffers};
use serde_json::Value;

use crate::{ThreeShader, ATTRIBUTE};

/// The buffers a node material may read, and the first one's binding.
pub const STORAGE: usize = 2;
pub const BINDING: u32 = 11;

/// The spec's wgsl `storage`: each buffer's handle by its binding (the empty one where none), its names, in order.
pub fn buffers(world: &mut World, wgsl: Option<&Value>) -> Result<([Handle<ShaderBuffer>; STORAGE], Vec<String>), String> {
    let list = wgsl.and_then(|w| w.get("storage")).and_then(Value::as_array).cloned().unwrap_or_default();
    if list.len() > STORAGE {
        return Err(format!("a three shader reads at most {STORAGE} storage buffers, got {}", list.len()));
    }
    world.init_resource::<SharedBuffers>();
    world.init_resource::<Assets<ShaderBuffer>>();
    world.resource_scope::<SharedBuffers, _>(|world, mut s| {
        let mut assets = world.resource_mut::<Assets<ShaderBuffer>>();
        let empty = s.empty(&mut assets);
        let mut out = [empty.clone(), empty];
        let mut names = vec![];
        for (i, b) in list.iter().enumerate() {
            let name = format!("tsl_s{i}");
            if b.get("name").and_then(Value::as_str) != Some(name.as_str()) {
                return Err(format!("a three shader's storage buffer {i} is {name}, got {}", b.get("name").unwrap_or(&Value::Null)));
            }
            let key = b.get("key").and_then(Value::as_str).ok_or_else(|| format!("storage buffer {name} has no key"))?;
            out[i] = s.get_or_make(&mut assets, key, first_contents(b).map_err(|e| format!("storage buffer {name}: {e}"))?)?;
            names.push(name);
        }
        Ok((out, names))
    })
}

/// Each stage's `var<storage, read> tsl_sN :` given its binding in the material's group.
pub fn bind(src: &str, names: &[String]) -> String {
    names.iter().enumerate().fold(src.to_string(), |s, (i, n)| {
        s.replace(&format!("var<storage, read> {n} :"), &format!("@group(constants::MATERIAL_BIND_GROUP) @binding({}) var<storage, read> {n} :", BINDING + i as u32))
    })
}

/// A BufferMesh's `copies` onto its mesh (one copy, as made from the geometry): the vertices and indices repeated a copy
/// at a time, its matrix baked in, and its own attributes (`own`, a0.. in order): `tsl_copy` the copy's number, a
/// per-copy attribute the copy's value, any other the geometry's per vertex (`v`'s attributes).
pub fn fit_copies(mesh: &mut Mesh, v: &Value, own: &[String]) -> Result<(), String> {
    let c = v.get("copies").ok_or("no copies")?;
    let count = c.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
    if !(1..=1 << 24).contains(&count) {
        return Err(format!("a BufferMesh's copies: count, 1 to {}", 1 << 24));
    }
    let n = mesh.count_vertices();
    let f4 = |a: &Option<&VertexAttributeValues>| match a {
        Some(VertexAttributeValues::Float32x4(x)) => Some(x.clone()),
        _ => None,
    };
    let get3 = |id| match mesh.attribute(id) {
        Some(VertexAttributeValues::Float32x3(x)) => Ok(x.clone()),
        _ => Err("a mesh of copies has positions and normals, three floats each".to_string()),
    };
    let (pos, nor) = (get3(Mesh::ATTRIBUTE_POSITION)?, get3(Mesh::ATTRIBUTE_NORMAL)?);
    let uv = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(x)) => x.clone(),
        _ => vec![[0.0; 2]; n],
    };
    let col = f4(&mesh.attribute(Mesh::ATTRIBUTE_COLOR)).unwrap_or_else(|| vec![[1.0; 4]; n]);
    let nums = |x: &Value, what: &str| -> Result<Vec<f32>, String> { x.as_array().ok_or_else(|| format!("{what}: numbers"))?.iter().map(|x| x.as_f64().map(|f| f as f32).ok_or_else(|| format!("{what}: numbers, got {x}"))).collect() };
    let mats: Option<Vec<Mat4>> = match c.get("matrices") {
        None | Some(Value::Null) => None,
        Some(x) => {
            let m = nums(x, "copies' matrices")?;
            if m.len() != count * 16 {
                return Err(format!("a BufferMesh's copies' matrices: 16 numbers for each of {count} copies, got {}", m.len()));
            }
            Some(m.chunks(16).map(Mat4::from_cols_slice).collect())
        }
    };
    // each own attribute: the copy's number, a per-copy value, or the geometry's per vertex (as fit_mesh lays it)
    enum Own {
        Copy,
        PerCopy(usize, Vec<f32>),
        PerVertex(Vec<[f32; 4]>),
    }
    let verts = v.get("positions").and_then(Value::as_array).map(|p| p.len() / 3).unwrap_or(0);
    let indexed = mesh.indices().is_some();
    let corners = if indexed { (0..verts).collect() } else { raea_sys_material::mesh::buffer_corners(v, verts) };
    let mut owns = vec![];
    for name in own {
        if name == "tsl_copy" {
            owns.push(Own::Copy);
        } else if let Some(a) = c.get("attributes").and_then(|a| a.get(name)) {
            let size = a.get("size").and_then(Value::as_u64).unwrap_or(1).clamp(1, 4) as usize;
            let vals = nums(a.get("values").unwrap_or(&Value::Null), &format!("copies' attribute {name}"))?;
            if vals.len() < count * size {
                return Err(format!("copies' attribute {name}: {size} numbers for each of {count} copies, got {}", vals.len()));
            }
            owns.push(Own::PerCopy(size, vals));
        } else {
            let a = v.get("attributes").and_then(|a| a.get(name)).ok_or_else(|| format!("the shader reads attribute {name}, which the mesh of copies doesn't give"))?;
            let size = a.get("size").and_then(Value::as_u64).unwrap_or(1).clamp(1, 4) as usize;
            let vals = nums(a.get("values").unwrap_or(&Value::Null), &format!("attribute {name}"))?;
            if vals.len() < verts * size {
                return Err(format!("attribute {name}: {size} numbers for each of {verts} vertices, got {}", vals.len()));
            }
            let per: Vec<[f32; 4]> = corners.iter().map(|&i| padded(&vals, i, size)).collect();
            if per.len() != n {
                return Err(format!("attribute {name}: {} vertices, the mesh has {n}", per.len()));
            }
            owns.push(Own::PerVertex(per));
        }
    }
    let total = n * count;
    let (mut p, mut q, mut u, mut k) = (Vec::with_capacity(total), Vec::with_capacity(total), Vec::with_capacity(total), Vec::with_capacity(total));
    let mut a: Vec<Vec<[f32; 4]>> = owns.iter().map(|_| Vec::with_capacity(total)).collect();
    for i in 0..count {
        let (m, nm) = match &mats {
            Some(m) => (m[i], Mat3::from_mat4(m[i]).inverse().transpose()),
            None => (Mat4::IDENTITY, Mat3::IDENTITY),
        };
        for j in 0..n {
            p.push(m.transform_point3(Vec3::from(pos[j])).to_array());
            q.push((nm * Vec3::from(nor[j])).normalize_or_zero().to_array());
            u.push(uv[j]);
            k.push(col[j]);
            for (out, o) in a.iter_mut().zip(&owns) {
                out.push(match o {
                    Own::Copy => [i as f32, 0.0, 0.0, 0.0],
                    Own::PerCopy(s, vals) => padded(vals, i, *s),
                    Own::PerVertex(per) => per[j],
                });
            }
        }
    }
    let idx: Option<Vec<u32>> = mesh.indices().map(|ix| match ix {
        Indices::U16(v) => v.iter().map(|&x| u32::from(x)).collect(),
        Indices::U32(v) => v.clone(),
    });
    let mut out = Mesh::new(mesh.primitive_topology(), mesh.asset_usage);
    out.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    out.insert_attribute(Mesh::ATTRIBUTE_NORMAL, q);
    out.insert_attribute(Mesh::ATTRIBUTE_UV_0, u);
    out.insert_attribute(Mesh::ATTRIBUTE_COLOR, k);
    for (i, vals) in a.into_iter().enumerate() {
        out.insert_attribute(ATTRIBUTE[i].clone(), vals);
    }
    if let Some(idx) = idx {
        out.insert_indices(Indices::U32((0..count).flat_map(|i| idx.iter().map(move |x| x + (i * n) as u32)).collect()));
    }
    *mesh = out;
    Ok(())
}

/// Item `i` of a list of `size`-number items, as a vec4 (zeros after its numbers).
fn padded(vals: &[f32], i: usize, size: usize) -> [f32; 4] {
    std::array::from_fn(|k| if k < size { vals[i * size + k] } else { 0.0 })
}

/// A mesh whose three shader reads a shared buffer isn't culled by its bounds (where the computes put its vertices).
pub fn unbounded(mut cmd: Commands, parts: Query<(Entity, &MeshMaterial3d<ThreeShader>), (Changed<MeshMaterial3d<ThreeShader>>, Without<NoFrustumCulling>)>, mats: Res<Assets<ThreeShader>>) {
    for (e, m) in &parts {
        if mats.get(&m.0).is_some_and(|m| m.unbounded) {
            cmd.entity(e).insert(NoFrustumCulling);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;

    #[test]
    fn copies_repeat_the_geometry_with_their_number_matrix_and_values() {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 3]);
        m.insert_indices(Indices::U32(vec![0, 1, 2]));
        let mut mats: Vec<f32> = Mat4::IDENTITY.to_cols_array().to_vec();
        mats.extend(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)).to_cols_array());
        let v = serde_json::json!({ "positions": [0, 0, 0, 1, 0, 0, 0, 1, 0], "indices": [0, 1, 2], "copies": { "count": 2, "attributes": { "instanceColor": { "size": 3, "values": [1, 0, 0, 0, 1, 0] } }, "matrices": mats } });
        fit_copies(&mut m, &v, &["tsl_copy".into(), "instanceColor".into()]).unwrap();
        assert_eq!(m.count_vertices(), 6);
        let Some(VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        assert_eq!(p[4], [6.0, 0.0, 0.0]);
        let Some(VertexAttributeValues::Float32x4(a0)) = m.attribute(ATTRIBUTE[0].clone()) else { panic!() };
        assert_eq!((a0[2][0], a0[3][0]), (0.0, 1.0));
        let Some(VertexAttributeValues::Float32x4(a1)) = m.attribute(ATTRIBUTE[1].clone()) else { panic!() };
        assert_eq!(a1[5], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(m.indices().unwrap().iter().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(bind("var<storage, read> tsl_s0 : B;", &["tsl_s0".into()]), "@group(constants::MATERIAL_BIND_GROUP) @binding(11) var<storage, read> tsl_s0 : B;");
    }
}
