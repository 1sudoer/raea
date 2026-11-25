//! Storage buffers shared by key (docs/r3f.feature R3F22): a three.js storage attribute (TSL's `instancedArray`) is one
//! buffer the computes that write it (raea-sys-compute three.rs) and the node materials that read it
//! (raea-sys-three-shader storage.rs) bind by its key ("tsl:0"). Each is Bevy's ShaderBuffer, made once with its size and
//! first contents and kept from frame to frame on the GPU (only what the computes write changes it); a new three
//! session's first frame writes its first contents back. A material reading fewer buffers than it binds has the empty
//! one (16 zero bytes) in the others.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use serde_json::Value;

/// The label every shared buffer has: a buffer written again with the same size and label is written in place on the
/// GPU (GpuShaderBuffer), so the bind groups holding it stand.
const LABEL: &str = "raea shared storage";

/// One shared buffer: its asset and its first contents, kept to reset it to (the GPU upload takes the asset's own).
struct Shared {
    handle: Handle<ShaderBuffer>,
    first: Vec<u32>,
}

/// The shared buffers, by key, and the empty one.
#[derive(Resource, Default)]
pub struct SharedBuffers {
    by_key: HashMap<String, Shared>,
    empty: Option<Handle<ShaderBuffer>>,
}

fn buffer(words: Vec<u32>) -> ShaderBuffer {
    let mut b = ShaderBuffer::new(words, RenderAssetUsages::default());
    b.label = LABEL.into();
    b
}

/// A buffer's first contents from its spec, {size, f32 | u32 | i32}: `size` bytes (a multiple of 4, at least 4) as
/// 32-bit words, the numbers given from the start in the kind they're given as and zeros after.
pub fn first_contents(v: &Value) -> Result<Vec<u32>, String> {
    let size = v.get("size").and_then(Value::as_u64).filter(|n| *n >= 4 && n % 4 == 0 && *n <= 1 << 30).ok_or_else(|| format!("a shared buffer's size is a number of bytes, a multiple of 4, got {}", v["size"]))?;
    let mut words = vec![0u32; size as usize / 4];
    let given = [("f32", 'f'), ("u32", 'u'), ("i32", 'i')].into_iter().filter_map(|(k, c)| v.get(k).map(|x| (k, c, x))).collect::<Vec<_>>();
    if given.len() > 1 {
        return Err("a shared buffer's contents are f32, u32 or i32, one of them".into());
    }
    if let Some((k, c, x)) = given.first() {
        let list = x.as_array().ok_or_else(|| format!("a shared buffer's {k} is a list of numbers"))?;
        if list.len() > words.len() {
            return Err(format!("a shared buffer of {size} bytes holds {} numbers, given {}", words.len(), list.len()));
        }
        for (w, n) in words.iter_mut().zip(list) {
            let n = n.as_f64().ok_or_else(|| format!("a shared buffer's {k}: numbers, got {n}"))?;
            *w = match c {
                'f' => (n as f32).to_bits(),
                'u' => n.max(0.0) as u32,
                _ => n as i32 as u32,
            };
        }
    }
    Ok(words)
}

impl SharedBuffers {
    /// The buffer `key` names, made with `first` if it's new; one already made with another size is refused (a
    /// compute and a material disagreeing on what they share). One made all zeros given contents takes them as its
    /// first (a material reading it built before the compute that gave them).
    pub fn get_or_make(&mut self, assets: &mut Assets<ShaderBuffer>, key: &str, first: Vec<u32>) -> Result<Handle<ShaderBuffer>, String> {
        if let Some(s) = self.by_key.get_mut(key) {
            if s.first.len() != first.len() {
                return Err(format!("shared buffer {key} is {} bytes, given {} bytes", s.first.len() * 4, first.len() * 4));
            }
            if s.first != first && s.first.iter().all(|w| *w == 0) {
                s.first = first;
                if let Some(mut b) = assets.get_mut(&s.handle) {
                    *b = buffer(s.first.clone());
                }
            }
            return Ok(s.handle.clone());
        }
        let handle = assets.add(buffer(first.clone()));
        self.by_key.insert(key.to_string(), Shared { handle: handle.clone(), first });
        Ok(handle)
    }

    /// The buffer `key` names, if it's made.
    pub fn get(&self, key: &str) -> Option<&Handle<ShaderBuffer>> {
        self.by_key.get(key).map(|s| &s.handle)
    }

    /// The buffers `keys` name written back to their first contents (on the GPU before the frame's computes run).
    pub fn reset<'a>(&self, assets: &mut Assets<ShaderBuffer>, keys: impl IntoIterator<Item = &'a str>) {
        for k in keys {
            if let Some(s) = self.by_key.get(k)
                && let Some(mut b) = assets.get_mut(&s.handle)
            {
                *b = buffer(s.first.clone());
            }
        }
    }

    /// 16 zero bytes, bound where a material reads no buffer.
    pub fn empty(&mut self, assets: &mut Assets<ShaderBuffer>) -> Handle<ShaderBuffer> {
        self.empty.get_or_insert_with(|| assets.add(buffer(vec![0; 4]))).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_shared_buffer_is_made_once_by_key_with_its_first_contents() {
        assert_eq!(first_contents(&json!({"size": 16, "f32": [1.5, 2]})).unwrap(), vec![1.5f32.to_bits(), 2f32.to_bits(), 0, 0]);
        assert_eq!(first_contents(&json!({"size": 8, "i32": [-1]})).unwrap(), vec![u32::MAX, 0]);
        assert!(first_contents(&json!({"size": 6})).unwrap_err().contains("multiple of 4"));
        assert!(first_contents(&json!({"size": 4, "u32": [1, 2]})).unwrap_err().contains("holds 1"));
        let mut assets = Assets::<ShaderBuffer>::default();
        let mut s = SharedBuffers::default();
        let a = s.get_or_make(&mut assets, "tsl:0", vec![0; 4]).unwrap();
        assert_eq!(s.get_or_make(&mut assets, "tsl:0", vec![0; 4]).unwrap(), a);
        assert!(s.get_or_make(&mut assets, "tsl:0", vec![0; 8]).unwrap_err().contains("16 bytes"));
        assert_ne!(s.empty(&mut assets), a);
        assert_eq!(assets.get(&a).unwrap().buffer_size(), 16);
    }
}
