// tint.rs: a colour a copy over the standard material (Bevy's extended-material fragment, its lighting kept): the
// copy's colour, found by its MeshTag above compose's two pass bits, multiplies the base colour before the light.
import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions::alpha_discard;
import bevy_pbr::render::mesh_functions::get_tag;
@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};
@if(PREPASS_PIPELINE) import bevy_pbr::deferred::functions::deferred_output;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};
@if(!PREPASS_PIPELINE) import bevy_pbr::render::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing};

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<storage, read> colours: array<vec4<f32>>;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let n = arrayLength(&colours);
    let copy = min(get_tag(in.instance_index) >> 2u, max(n, 1u) - 1u);
    let tint = colours[copy];
    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * tint.rgb, pbr_input.material.base_color.a * tint.a);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);

@if(PREPASS_PIPELINE) {
    return deferred_output(in, pbr_input);
} @else {
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
}
