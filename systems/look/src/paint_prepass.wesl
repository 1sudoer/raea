// paint.rs: the paint look's prepass fragment stage (depth, normals, motion vectors; a masked material's shadow
// maps): Bevy 0.20's own (bevy_pbr render/pbr_prepass.wesl, without its meshlet path) with one change: a mesh of
// another world (paint_world.wesl) isn't drawn, as in the main pass, so the outline, the ambient occlusion and
// whatever stands behind it see the cut. The Paint struct is paint.wesl's (keep the three the same).
import bevy_pbr::render::{
    pbr_prepass_functions,
    pbr_bindings,
    pbr_types,
    pbr_functions,
    pbr_functions::SampleBias,
    mesh_bindings::mesh,
    mesh_view_bindings::view,
};
import bevy_pbr::prepass::io as prepass_io;
import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};
import raea_sys_look::paint_world::{world_at, world_shows};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

struct Paint {
    shadow: vec4<f32>,
    mid: vec4<f32>,
    ao: vec4<f32>,
    foam: vec4<f32>,
    patch_: vec4<f32>,
    // bands, ao height, ground y, water y
    k0: vec4<f32>,
    // foam height, ssao threshold, shadow-edge raggedness, breakup amount
    k1: vec4<f32>,
    // breakup scale, edge scale, wash, wash patch size (uv)
    k2: vec4<f32>,
    // stroke amount, stroke length, saturation, foliage
    k3: vec4<f32>,
    // foliage height, calm from, calm to, calm amount
    k4: vec4<f32>,
    calm: vec4<f32>,
    // texel (m: surfaces as pixel art, 0 for none), each cell's shade jitter, cloud shadows' drift (x, z, m)
    k5: vec4<f32>,
    // cloud shadows: cover (0 none to 1), size (m); this material sways in the wind (1: a cut-out leaf or blade), -
    k6: vec4<f32>,
    // wind: how far a blade's tip bends (m, 0 none), its direction (x, z), the time it has blown (s)
    k7: vec4<f32>,
    // haze: how thick (per metre), how fast it thins with height (m), where it starts (m), its most (0 to 1); its
    // colour (a: on), and its colour toward the sun (a: on)
    k8: vec4<f32>,
    haze: vec4<f32>,
    haze_sun: vec4<f32>,
    // worlds (paint_world.wesl): up to four cuts, each a ball (its centre, its radius in m; 0: off)
    cut: array<vec4<f32>, 4>,
    // what's inside each cut: its style (0: the look's own), the style's scale, how much of it (0 to 1), -
    style: array<vec4<f32>, 4>,
    style_a: array<vec4<f32>, 4>,
    style_b: array<vec4<f32>, 4>,
    // this mesh's world (0: outside every cut; 1 to 4: inside that cut alone; -1: everywhere), the cuts' torn edge
    // (m), their lit rim's width (m), -
    kw: vec4<f32>,
    // the rim's colour (a: on)
    cut_rim: vec4<f32>,
    // the eye-side rim's amount and power, the terminator's amount, -
    k9: vec4<f32>,
    rim: vec4<f32>,
    term: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> paint: Paint;

fn cut_away(p: vec3<f32>) -> bool {
    let world = world_at(p, paint.cut[0], paint.cut[1], paint.cut[2], paint.cut[3], paint.kw.y);
    return !world_shows(world.x, paint.kw.x);
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> prepass_io::FragmentOutput {
    if cut_away(in.world_position.xyz) {
        discard;
    }
@if(BINDLESS)
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
@if(BINDLESS)
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
@if(BINDLESS)
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
@if(!BINDLESS)
    let flags = pbr_bindings::material.flags;
@if(!BINDLESS)
    let uv_transform = pbr_bindings::material.uv_transform;

@if(VISIBILITY_RANGE_DITHER)
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);

    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);

    var out: prepass_io::FragmentOutput;

@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;

@if(NORMAL_PREPASS)
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let mesh_flags = mesh[in.instance_index].flags;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(mesh_flags, is_front);

        let world_normal = pbr_functions::prepare_world_normal(
            in.world_normal,
            double_sided,
            is_logically_front,
        );

        var normal = world_normal;

@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
@if(STANDARD_MATERIAL_NORMAL_MAP) {

@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
@else
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;

        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;

@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
@else
        let texture = pbr_bindings::normal_map_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
@else
        let sampler_ = pbr_bindings::normal_map_sampler;
        let Nt =
            textureSampleBias(
                texture,
                sampler_,
                uv,
                bias.mip_bias,
            ).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);

        normal = pbr_functions::apply_normal_mapping(
            flags,
            TBN,
            double_sided,
            is_logically_front,
            Nt,
        );

}
}
}

        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);

    return out;
}
@else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    if cut_away(in.world_position.xyz) {
        discard;
    }
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
}
