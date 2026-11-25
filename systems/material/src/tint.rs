//! A colour a copy over an <Instances>' shared surface (three's instanceColor, docs/r3f.feature R3F20a): the standard
//! material extended by a storage buffer of the copies' linear colours, which multiply its base colour before the light.
//! Every copy holds the one TintMaterial, so Bevy's automatic batching still draws the set as one instanced call; a
//! copy finds its colour by its MeshTag above the two low bits (compose.rs keeps a mesh's composition pass in those).
//!
//! scene3d's instance.rs makes it for a mesh row with `inst_tint` 1 and rewrites the buffer when the frame's list
//! changes; the standard material it extends is the row's own, copied in again when that changes.

use super::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::render::render_resource::AsBindGroup;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use raea_core_program::eval::Tup;

pub type TintMaterial = ExtendedMaterial<StandardMaterial, Tint>;

pub fn install(app: &mut App) {
    bevy::asset::embedded_asset!(app, "tint.wesl");
    app.add_plugins(MaterialPlugin::<TintMaterial>::default());
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct Tint {
    /// each copy's linear rgba, by its MeshTag >> 2
    #[storage(100, read_only)]
    pub colours: Handle<ShaderBuffer>,
}

impl MaterialExtension for Tint {
    fn fragment_shader() -> ShaderRef {
        "embedded://raea_sys_material/tint.wesl".into()
    }
}

/// The copies' colours as the buffer holds them: linear rgba, alpha 1. A buffer of the same size is written in place
/// on the GPU (GpuShaderBuffer reuses it), so the material's bind group stands.
pub fn colour_buffer(rgb: &[[f32; 3]]) -> ShaderBuffer {
    let mut words: Vec<[f32; 4]> = Vec::with_capacity(rgb.len());
    for &[r, g, b] in rgb {
        words.push([r, g, b, 1.0]);
    }
    ShaderBuffer::new(words, bevy::asset::RenderAssetUsages::default())
}

/// A copy's MeshTag: its number above the two bits a composition pass keeps
pub fn copy_tag(copy: usize, pass: u32) -> u32 {
    ((copy as u32) << 2) | (pass & 3)
}

/// How many numbers a copy a row's lists hold: 10 (position, rotation xyzw, scale), 13 with a colour (`inst_tint`)
pub fn stride_of(t: &Tup) -> usize {
    if t.vals.get("inst_tint").is_some_and(V::truthy) { 13 } else { 10 }
}

/// One list of copies: each one's placement, and its linear colour when the list has them
#[derive(Debug)]
pub struct Copies {
    pub at: Vec<Transform>,
    pub tint: Vec<[f32; 3]>,
}

/// A row's list of copies (`stride` numbers each) as placements and colours
pub fn copies(id: &str, s: &str, stride: usize) -> Result<Copies, String> {
    let mut n: Vec<f32> = Vec::with_capacity(s.len() / 6);
    for x in s.split_ascii_whitespace() {
        n.push(x.parse().map_err(|e| format!("{id}: inst: {x}: {e}"))?);
    }
    if n.len() % stride != 0 {
        return Err(format!("{id}: inst is {stride} numbers a copy, got {}", n.len()));
    }
    let at = n.chunks(stride).map(|c| Transform { translation: Vec3::new(c[0], c[1], c[2]), rotation: Quat::from_xyzw(c[3], c[4], c[5], c[6]).normalize(), scale: Vec3::new(c[7], c[8], c[9]) }).collect();
    let tint = if stride == 13 { n.chunks(13).map(|c| [c[10], c[11], c[12]]).collect() } else { vec![] };
    Ok(Copies { at, tint })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_tag_keeps_the_pass_bits() {
        assert_eq!(copy_tag(5, 2), 22);
        assert_eq!(copy_tag(5, 2) >> 2, 5);
        assert_eq!(copy_tag(5, 2) & 3, 2);
    }

    #[test]
    fn a_list_of_13_a_copy_holds_colours() {
        let c = copies("m", "1 2 3 0 0 0 1 1 1 1 0.5 0.25 0", 13).unwrap();
        assert_eq!((c.at[0].translation, c.tint[0]), (Vec3::new(1.0, 2.0, 3.0), [0.5, 0.25, 0.0]));
        assert!(copies("m", "1 2 3", 10).unwrap_err().contains("10 numbers a copy"));
    }
}
