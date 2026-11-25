//! The plant as one .glb: one node, one mesh, a primitive for the bark and one for each leaf group that has leaves.
//! Materials: "bark" (the species' colour, rough) and "leaf-0" to "leaf-7" over the atlas (alpha mask at 0.1, both
//! sides drawn: the atlas's alpha is a distance, 0.5 on the edge, and the hook cuts at 0.5 near and lower far). A group is hidden by giving its material an opacity of 0 (its shadow goes with it).

use serde_json::{Value, json};

use crate::species::Species;
use crate::{GROUPS, Mesh, Plant};

struct Bin {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Bin {
    fn view(&mut self, data: &[u8], target: Option<u32>) -> usize {
        while self.bytes.len() % 4 != 0 {
            self.bytes.push(0);
        }
        let mut v = json!({ "buffer": 0, "byteOffset": self.bytes.len(), "byteLength": data.len() });
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.bytes.extend_from_slice(data);
        self.views.push(v);
        self.views.len() - 1
    }
    fn floats<const N: usize>(&mut self, xs: &[[f32; N]], kind: &str, bounds: bool) -> usize {
        let data: Vec<u8> = xs.iter().flat_map(|x| x.iter().flat_map(|f| f.to_le_bytes())).collect();
        let view = self.view(&data, Some(34962));
        let mut a = json!({ "bufferView": view, "componentType": 5126, "count": xs.len(), "type": kind });
        if bounds {
            let lo: Vec<f32> = (0..N).map(|k| xs.iter().map(|x| x[k]).fold(f32::INFINITY, f32::min)).collect();
            let hi: Vec<f32> = (0..N).map(|k| xs.iter().map(|x| x[k]).fold(f32::NEG_INFINITY, f32::max)).collect();
            a["min"] = json!(lo);
            a["max"] = json!(hi);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
    fn indices(&mut self, idx: &[u32]) -> usize {
        let data: Vec<u8> = idx.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&data, Some(34963));
        self.accessors.push(json!({ "bufferView": view, "componentType": 5125, "count": idx.len(), "type": "SCALAR" }));
        self.accessors.len() - 1
    }
    fn primitive(&mut self, m: &Mesh, material: usize) -> Value {
        let mut attrs = json!({
            "POSITION": self.floats(&m.pos, "VEC3", true),
            "NORMAL": self.floats(&m.nrm, "VEC3", false),
            "TEXCOORD_0": self.floats(&m.uv, "VEC2", false),
        });
        if !m.uv1.is_empty() {
            attrs["TEXCOORD_1"] = json!(self.floats(&m.uv1, "VEC2", false));
        }
        if !m.col.is_empty() {
            attrs["COLOR_0"] = json!(self.floats(&m.col, "VEC4", false));
        }
        json!({ "attributes": attrs, "indices": self.indices(&m.idx), "material": material })
    }
}

pub(crate) fn glb(sp: &Species, plant: &Plant, atlas_png: &[u8]) -> Vec<u8> {
    let mut b = Bin { bytes: Vec::new(), views: Vec::new(), accessors: Vec::new() };
    let mut prims = vec![b.primitive(&plant.bark, 0)];
    for (k, m) in plant.leaves.iter().enumerate() {
        if !m.idx.is_empty() {
            prims.push(b.primitive(m, 1 + k));
        }
    }
    let image = b.view(atlas_png, None);
    let [r, g, bl] = sp.bark;
    let mut materials = vec![json!({ "name": "bark", "pbrMetallicRoughness": { "baseColorFactor": [r, g, bl, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.92 } })];
    for k in 0..GROUPS {
        materials.push(json!({
            "name": format!("leaf-{k}"),
            "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 }, "metallicFactor": 0.0, "roughnessFactor": 0.6 },
            "alphaMode": "MASK", "alphaCutoff": 0.1, "doubleSided": true,
        }));
    }
    let j = json!({
        "asset": { "version": "2.0", "generator": crate::VERSION },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": sp.id, "mesh": 0 }],
        "meshes": [{ "name": sp.id, "primitives": prims }],
        "materials": materials,
        "images": [{ "bufferView": image, "mimeType": "image/png" }],
        "samplers": [{ "magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071 }],
        "textures": [{ "source": 0, "sampler": 0 }],
        "buffers": [{ "byteLength": b.bytes.len() }],
        "bufferViews": b.views,
        "accessors": b.accessors,
    });
    raea_sys_glb::glb_bytes(&j, &b.bytes)
}
