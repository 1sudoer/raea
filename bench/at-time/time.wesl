// The left half grey by Bevy's globals.time, the right by the uniform the frame gives: each a quarter of its seconds.
import bevy_pbr::render::{
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> seconds: f32;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = select(seconds, globals.time, in.uv.x < 0.5) / 4.0;
    return vec4<f32>(t, t, t, 1.0);
}
