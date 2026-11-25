//! A lit node material (docs/r3f.feature R3F21): three's MeshStandardNodeMaterial or MeshPhysicalNodeMaterial with
//! nodes of its own, lit by Bevy's lights and shadows as raea's StandardMaterial is. three's builder writes the
//! material's surface without its lights (sdk/r3f's input/tsl-lit.ts), and its fragment stage returns that surface:
//!   fn three_surface(in: ThreeVaryings, front: bool) -> ThreeSurface
//!   struct ThreeSurface { base: vec4<f32>, normal: vec4<f32>, emissive: vec4<f32>, world: vec4<f32> }
//! base is the colour and opacity its colorNode and opacityNode give, normal.xyz its normal in view space (normalNode's,
//! or the mesh's) and normal.w its roughness, emissive.xyz its emission and emissive.w its metalness, world.xyz the
//! point in world space (moved by its positionNode). The varyings carry `three_instance`, the mesh's instance, which
//! the vertex entry sets: Bevy's mesh flags (whether it takes shadows) are read with it. This entry makes Bevy's
//! PbrInput of that surface and runs the standard material's lighting and its fog, tone mapping and alpha after.

use crate::module::DISPLAY_SLOT;
use bevy::prelude::AlphaMode;

/// The vertex entry's last line for a lit material: the varyings carry the instance to the fragment stage.
pub const VERTEX_TAIL: &str = "    var o = three_vertex(v);\n    o.three_instance = v.instance;\n    return o;\n";

/// What the lit fragment entry imports, in front of the module (WESL takes imports only at its top).
pub const IMPORTS: &str = "import bevy_pbr::render::pbr_types::{pbr_input_new, STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT};\n\
    import bevy_pbr::render::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, calculate_view};\n\
    import bevy_pbr::render::mesh_bindings::mesh;\n";

/// The fragment entry of a lit material (its module starts with IMPORTS).
pub fn fragment_entry() -> String {
    format!(
        "@fragment\n\
         fn fragment(in: ThreeVaryings, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {{\n\
         \x20   let s = three_surface(in, front);\n\
         \x20   var pbr = pbr_input_new();\n\
         \x20   pbr.material.base_color = s.base;\n\
         \x20   pbr.material.perceptual_roughness = s.normal.w;\n\
         \x20   pbr.material.metallic = s.emissive.w;\n\
         \x20   // past the camera's exposure, as raea's StandardMaterial's emissive is (core/scene props.rs EMISSIVE)\n\
         \x20   pbr.material.emissive = vec4<f32>(s.emissive.xyz, 0.0);\n\
         \x20   let look = three_slots.s[{DISPLAY_SLOT}];\n\
         \x20   pbr.material.flags = (u32(look.z + 0.5) << 29u) | select(0u, STANDARD_MATERIAL_FLAGS_FOG_ENABLED_BIT, look.w > 0.5);\n\
         \x20   pbr.frag_coord = in.clip;\n\
         \x20   pbr.world_position = vec4<f32>(s.world.xyz, 1.0);\n\
         \x20   let n = normalize((view.world_from_view * vec4<f32>(s.normal.xyz, 0.0)).xyz);\n\
         \x20   pbr.world_normal = n;\n\
         \x20   pbr.N = n;\n\
         \x20   pbr.is_orthographic = view.clip_from_view[3].w == 1.0;\n\
         \x20   pbr.V = calculate_view(pbr.world_position, pbr.is_orthographic);\n\
         \x20   pbr.flags = mesh[in.three_instance].flags;\n\
         \x20   return main_pass_post_lighting_processing(pbr, apply_pbr_lighting(pbr));\n\
         }}\n"
    )
}

/// The alpha mode as the standard material's flags count it (pbr_types' STANDARD_MATERIAL_FLAGS_ALPHA_MODE_*, shifted
/// down 29 bits): the lighting keeps an opaque surface's alpha at 1, and the post-lighting step premultiplies an
/// additive or multiplied one.
pub fn alpha_code(a: AlphaMode) -> f32 {
    match a {
        AlphaMode::Opaque => 0.0,
        AlphaMode::Mask(_) => 1.0,
        AlphaMode::Blend => 2.0,
        AlphaMode::Premultiplied => 3.0,
        AlphaMode::Add => 4.0,
        AlphaMode::Multiply => 5.0,
        AlphaMode::AlphaToCoverage => 6.0,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_surface_is_lit_by_bevy() {
        let f = super::fragment_entry();
        assert!(f.contains("let s = three_surface(in, front);"));
        assert!(f.contains("apply_pbr_lighting(pbr)"));
        assert!(f.contains("pbr.flags = mesh[in.three_instance].flags;"));
        assert!(f.contains(&format!("three_slots.s[{}]", super::DISPLAY_SLOT)));
        assert_eq!(super::alpha_code(bevy::prelude::AlphaMode::Add), 4.0);
        // WESL takes imports only at a module's top: a lit fragment module starts with the entry's
        let (v, m) = crate::modules("fn three_vertex(v: ThreeVertex) -> ThreeVaryings { var o: ThreeVaryings; return o; }", "fn three_surface(in: ThreeVaryings, front: bool) -> ThreeSurface { var s: ThreeSurface; return s; }", &[], Some(&[]), 0, true);
        assert!(m.starts_with(super::IMPORTS), "{m}");
        assert!(v.contains("o.three_instance = v.instance;"));
    }
}
