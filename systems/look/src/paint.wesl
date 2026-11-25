// paint.rs: a surface lit as an illustrator paints it (the paint and cartoon looks). The suns' light on it (how
// square it faces each, and whether it's in that sun's shadow, cut hard at half) is put into `bands` flat tones: lit
// is the surface's own colour, turned from the light its colour times `mid`, in a cast shadow its colour times
// `shadow` (bands 2: one shade, `shadow`, for both; 4 and more: the lit side stepped from `mid` up by how square it
// faces the light). Point and spot lights only where one asks (paint_lights.wesl). No specular, no ambient
// gradient: chosen colours, not physically blended ones. The rest of the look (ragged edges, grain, the sky, a
// palette) is look.wesl's mode 6.
import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions::alpha_discard;
import bevy_pbr::render::pbr_bindings;
import bevy_pbr::render::pbr_types::{STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT, STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS, STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE, STANDARD_MATERIAL_FLAGS_UNLIT_BIT};
import bevy_pbr::render::mesh_view_bindings as view_bindings;
import bevy_pbr::render::mesh_view_types;
import bevy_pbr::render::shadows;
import bevy_pbr::render::mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT;
import raea_sys_look::paint_lights::light_bands;
import raea_sys_look::paint_world::{world_at, world_shows};
@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};
@if(PREPASS_PIPELINE) import bevy_pbr::deferred::functions::deferred_output;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};
@if(!PREPASS_PIPELINE) import bevy_pbr::render::pbr_functions::main_pass_post_lighting_processing;

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
    // cloud shadows: cover (0 none to 1), size (m); this material sways in the wind (1: a cut-out leaf or blade); soft
    // (0: the light in flat bands; above it the light as a gradient this wide)
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

fn hash3(p: vec3<f32>) -> f32 {
    var q = bitcast<vec3<u32>>(vec3<i32>(floor(p)));
    q = q * 1664525u + 1013904223u;
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    q = q ^ (q >> vec3<u32>(16u));
    return f32(q.x + q.y + q.z) * (1.0 / 4294967295.0);
}

fn vnoise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

/// 0..1, three octaves of world-space noise
fn fbm3(x: vec3<f32>) -> f32 {
    return 0.55 * vnoise(x) + 0.3 * vnoise(x * 2.13 + 17.1) + 0.15 * vnoise(x * 4.37 - 5.3);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);

@if(PREPASS_PIPELINE) {
    return deferred_output(in, pbr_input);
} @else {
    var out: FragmentOutput;
    var c = pbr_input.material.base_color;
    // texels: with `texel` metres set, every surface is drawn as pixel art fixed to the world. The point is moved to
    // the centre of its cell in a world grid `texel` across (nudged a quarter cell off the surface first, so a face
    // lying on a grid plane doesn't flicker between cells), and everything the shading reads from the point (the
    // texture, the shadow, strokes, grades) is read there: each cell one colour, the frame still full resolution
    var texel = paint.k5.x;
    let fine = pbr_input.world_position.xyz;
    // the world the point is in (a <Look cuts>: paint_world.wesl), and that world's style and two colours
    let world = world_at(fine, paint.cut[0], paint.cut[1], paint.cut[2], paint.cut[3], paint.kw.y);
    var wstyle = vec4<f32>(0.0);
    var wa = vec3<f32>(0.0);
    var wb = vec3<f32>(1.0);
    if world.x > 0.5 {
        let wi = u32(world.x) - 1u;
        wstyle = paint.style[wi];
        wa = paint.style_a[wi].rgb;
        wb = paint.style_b[wi].rgb;
        // pixels: the world's surfaces in cells of its own size (not a mesh that is in every world: the cast keeps
        // its own detail, and takes the world's short ramp of colour)
        if wstyle.x > 0.5 && wstyle.x < 1.5 && paint.kw.x > -0.5 {
            texel = wstyle.y;
        }
    }
    var wp = fine;
    if texel > 0.0 {
        wp = (floor((fine + pbr_input.world_normal * texel * 0.25) / texel) + 0.5) * texel;
    }
    // the texture's derivatives (below) taken here, before any branch: WGSL (and WebGPU's compiler, which refuses
    // otherwise) wants them in uniform control flow, and the unlit return below depends on the material
    var uv0 = vec2<f32>(0.0);
    var px = vec3<f32>(0.0);
    var py = vec3<f32>(0.0);
    var ux = vec2<f32>(0.0);
    var uy = vec2<f32>(0.0);
@if(VERTEX_UVS_A) {
    uv0 = (pbr_input.material.uv_transform * vec3<f32>(in.uv, 1.0)).xy;
    px = dpdx(fine);
    py = dpdy(fine);
    ux = dpdx(uv0);
    uy = dpdy(uv0);
}
    // a mesh of another world isn't here
    if !world_shows(world.x, paint.kw.x) {
        discard;
    }
    // unlit (a MeshBasicMaterial, `unlit`): its own colour, as drawn
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out.color = main_pass_post_lighting_processing(pbr_input, vec4<f32>(c.rgb + pbr_input.material.emissive.rgb * view_bindings::view.exposure, c.a));
        return out;
    }
@if(VERTEX_UVS_A) {
    // the texture read at the cell's centre: the uv there found from how uv and the world move across the screen
    // (the derivatives taken above, before any branch)
@if(!BINDLESS) {
    if texel > 0.0 && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        let d = wp - fine;
        // the screen offset (a, b) whose world step px * a + py * b is nearest d (least squares)
        let xx = dot(px, px);
        let xy = dot(px, py);
        let yy = dot(py, py);
        let det = xx * yy - xy * xy;
        if abs(det) > 1e-20 {
            let a = (yy * dot(px, d) - xy * dot(py, d)) / det;
            let b = (xx * dot(py, d) - xy * dot(px, d)) / det;
            let t = textureSampleLevel(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, uv0 + ux * a + uy * b, 0.0);
            c = vec4<f32>(t.rgb * pbr_bindings::material.base_color.rgb, c.a);
        }
    }
}
    // the wash: a textured surface takes its colour as the texture's average over a patch around the point (a spiral
    // of 12 taps), so a scanned model reads as painted colour rather than photographic detail; its alpha (a leaf's
    // cut-out) stays fine
    let wash = paint.k2.z;
    if wash > 0.0 && (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        let uv = (pbr_input.material.uv_transform * vec3<f32>(in.uv, 1.0)).xy;
        let r = paint.k2.w;
        var sum = vec3<f32>(0.0);
        var wsum = 0.0;
        for (var k = 0; k < 12; k = k + 1) {
            let a = f32(k) * 2.3998;
            let d = r * sqrt((f32(k) + 0.5) / 12.0);
            let tap = textureSampleLevel(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, uv + d * vec2<f32>(cos(a), sin(a)), 0.0);
            // taps in the cut-out (a leaf's transparent margin) don't count
            sum += tap.rgb * tap.a;
            wsum += tap.a;
        }
        let coarse = sum / max(wsum, 1e-3);
        @if(BINDLESS)
        let tint = c.rgb;
        @else
        let tint = pbr_bindings::material.base_color.rgb;
        c = vec4<f32>(mix(c.rgb, coarse * tint, wash), c.a);
    }
}
    // foliage: a cut-out leaf or blade (any material with alpha: masked, blended or to coverage) is shaded as part of its plant's mass, as a painter shades a
    // clump of grass: its normal bent up toward the sky, so the clump takes the light as one shape, and its colour
    // graded from a dark root to a light tip over `foliage height` metres
    let alpha_mode = pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if paint.k3.w > 0.0 && alpha_mode != STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        pbr_input.N = normalize(mix(pbr_input.N, vec3<f32>(0.0, 1.0, 0.0), paint.k3.w));
        let h = clamp((wp.y - paint.k0.z) / max(paint.k4.x, 0.01), 0.0, 1.0);
        c = vec4<f32>(c.rgb * mix(1.0, mix(0.55, 1.3, h), paint.k3.w), c.a);
    }
    // brush strokes: the colour lightened and darkened in short strokes laid along one direction in the world
    if paint.k3.x > 0.0 {
        let q = wp / paint.k3.y;
        let sk = fbm3(vec3<f32>(q.x * 0.35 + q.z * 0.2, q.y * 1.6, q.z * 0.35 - q.x * 0.2));
        c = vec4<f32>(c.rgb * (1.0 + (sk - 0.5) * 1.2 * paint.k3.x), c.a);
    }
    // the mesh's breakup: patches of noise over it, in its patch colour, and the same noise wandering its tone
    // boundaries (a mottled field, rust on a container, a painted wall's unevenness)
    let breakup = paint.k1.w;
    var wobble = 0.0;
    if breakup > 0.0 {
        let b = fbm3(wp / paint.k2.x);
        wobble = (b - 0.5) * breakup;
        if paint.patch_.w > 0.0 && b > 1.0 - 0.5 * breakup {
            c = vec4<f32>(mix(c.rgb, paint.patch_.rgb, paint.patch_.w), c.a);
        }
    }
    // the shadows' cut wanders by noise: a torn edge, not a ruled one
    let edge = (fbm3(wp / paint.k2.y + vec3<f32>(3.1, 0.0, 7.7)) - 0.5) * paint.k1.z;
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), pbr_input.world_position);
    let suns = view_bindings::lights.n_directional_lights;
    // how square the surface faces the suns (`ndl`), and whether a sun that it faces reaches it (`lit`)
    var ndl = 0.0;
    var lit = false;
    // soft: how far the surface is turned to a sun (`face`, 0 to 1 over `soft` of the turn) and how much of that
    // light arrives past the cast shadows (`reach`), both as gradients
    var soft = paint.k6.w;
    // a world of pixels or of poster tones keeps its light flat
    if (wstyle.x > 0.5 && wstyle.x < 1.5) || (wstyle.x > 6.5 && wstyle.x < 7.5) {
        soft = 0.0;
    }
    var face = 0.0;
    var reach = 0.0;
    if suns == 0u {
        ndl = 1.0;
        lit = true;
        face = 1.0;
        reach = 1.0;
    }
    for (var i: u32 = 0u; i < suns; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        let d = dot(pbr_input.N, (*light).direction_to_light) + wobble;
        if d <= 0.03 {
            continue;
        }
        var s = 1.0;
        if (pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && ((*light).flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            // the cell's shadow (at its centre, lifted a cell off the surface so the surface doesn't shadow itself)
            var sp = pbr_input.world_position;
            if texel > 0.0 {
                sp = vec4<f32>(wp + pbr_input.world_normal * texel, 1.0);
            }
            s = shadows::fetch_directional_shadow(i, sp, pbr_input.world_normal, view_z, pbr_input.frag_coord.xy);
        }
        // inside a cut the sun's shadows are the other world's (its walls and furniture still stand in the shadow
        // map): the world is lit by how it faces the sun alone
        if world.x > 0.5 {
            s = 1.0;
        }
        if soft > 0.0 {
            let e = smoothstep(0.0, soft, d);
            face = max(face, e);
            reach = max(reach, e * s);
        }
        // the shadow's filtered edge cut at half: a shadow is a shape, not a gradient
        if s >= 0.5 + edge {
            lit = true;
            ndl = max(ndl, d);
        } else if !lit {
            ndl = -1.0;
        }
    }
    // the tones, as an illustrator sorts them: lit; turned from the light (form shadow, `mid`); facing it but shaded
    // by something else (cast shadow, `shadow`). Two bands: lit and one shade (`shadow`) for both. Four and more: the
    // lit side in bands - 2 steps from `mid` to the surface's own colour, by how square it faces the light.
    let n = max(round(paint.k0.x), 1.0);
    var tint = vec3<f32>(1.0);
    if n > 1.5 && !lit {
        let shaded = ndl < 0.0;
        if n < 2.5 || shaded {
            tint = paint.shadow.rgb;
        } else {
            tint = paint.mid.rgb;
        }
    } else if n > 3.5 && lit {
        let steps = n - 2.0;
        let j = min(steps, 1.0 + floor(clamp(ndl, 0.0, 1.0) * steps));
        tint = mix(paint.mid.rgb, vec3<f32>(1.0), j / steps);
    }
    // soft: the same tones as a gradient, the form shadow turning into the light over `soft` and the cast shadow as
    // soft as its filtered edge
    if soft > 0.0 && n > 1.5 {
        var turned = paint.mid.rgb;
        if n < 2.5 {
            turned = paint.shadow.rgb;
        }
        tint = mix(mix(turned, vec3<f32>(1.0), face), paint.shadow.rgb, clamp(face - reach, 0.0, 1.0));
        // the terminator: where the surface turns out of the light its tone takes a colour of its own (a warm
        // bleed, as light through skin or paint), most at the half turn
        if paint.k9.z > 0.0 {
            tint = mix(tint, tint * paint.term.rgb, 4.0 * face * (1.0 - face) * paint.k9.z);
        }
    }
    // cloud shadows: big soft-edged shapes of shade drifting over the land, cut hard like the cast shadows
    if lit && paint.k6.x > 0.0 {
        let q = vec2<f32>(wp.x - paint.k5.z, wp.z - paint.k5.w) / max(paint.k6.y, 1.0);
        // one broad octave and a finer one for the shapes' edges; the noise keeps to about 0.25 to 0.55 (measured on
        // the meadow), so the cut runs through that: cover 0.5 shades about half the land, 1 nearly all
        let cn = vnoise(vec3<f32>(q.x, 0.5, q.y)) * 0.8 + vnoise(vec3<f32>(q.x * 3.1, 0.5, q.y * 3.1)) * 0.2;
        if cn > 0.55 - 0.3 * paint.k6.x {
            tint = paint.shadow.rgb;
        }
    }
    // the point and spot lights that ask for paint, in bands of their own colour (paint_lights.wesl)
    tint = light_bands(tint, lit, pbr_input.world_position, pbr_input.N, pbr_input.world_normal, pbr_input.frag_coord, view_z, pbr_input.is_orthographic, pbr_input.flags, wobble, edge, soft);
    let up = abs(pbr_input.N.y);
    // ambient occlusion as one band: walls near the ground (ao height, ragged) and creases the SSAO finds
    if paint.ao.w > 0.0 {
        let n2 = fbm3(wp * 1.7 + vec3<f32>(11.0, 0.0, 5.0));
        // the ground's bounce band is on the shade side: where the sun falls it's the sun's light
        var occluded = !lit && paint.k0.y > 0.0 && up < 0.7 && wp.y - paint.k0.z < paint.k0.y * (0.6 + 0.8 * n2);
        if paint.k1.y > 0.0 && pbr_input.diffuse_occlusion.r < paint.k1.y + (n2 - 0.5) * 0.15 {
            occluded = true;
        }
        if soft > 0.0 {
            // soft: the band's edges as gradients (up the wall, and over the SSAO's last `soft` before its threshold)
            var o = 0.0;
            if !lit && paint.k0.y > 0.0 && up < 0.7 {
                o = 1.0 - smoothstep(0.0, paint.k0.y * (0.6 + 0.8 * n2), wp.y - paint.k0.z);
            }
            if paint.k1.y > 0.0 {
                o = max(o, 1.0 - smoothstep(paint.k1.y * (1.0 - soft), paint.k1.y + 1e-3, pbr_input.diffuse_occlusion.r));
            }
            tint = mix(tint, tint * paint.ao.rgb, o);
        } else if occluded {
            tint = tint * paint.ao.rgb;
        }
    }
    var rgb = c.rgb * tint + pbr_input.material.emissive.rgb * view_bindings::view.exposure;
    // foam where things stand in the water: broken white along the waterline of anything not facing up
    if paint.foam.w > 0.0 && up < 0.8 {
        let h = wp.y - paint.k0.w;
        let f = fbm3(vec3<f32>(wp.x * 3.0, wp.y * 1.2, wp.z * 3.0) + 2.0);
        if h > -0.05 && h < paint.k1.x * (0.2 + 1.4 * f) {
            rgb = paint.foam.rgb;
        }
    }
    // calm: low foliage (cut-out blades and petals) settled toward one colour with distance, so grass and flowers
    // finer than a pixel read as a field rather than speckle; trunks, rocks and the ground keep theirs
    if paint.calm.w > 0.0 && alpha_mode != STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        let far = smoothstep(paint.k4.y, paint.k4.z, length(wp - view_bindings::view.world_position.xyz));
        let low = 1.0 - smoothstep(paint.k4.x * 2.0, paint.k4.x * 3.0, wp.y - paint.k0.z);
        rgb = mix(rgb, paint.calm.rgb * tint, far * low * paint.k4.w);
    }
    // each texel a shade of its own, lighter or darker by up to `jitter`: a flat colour reads as hand-placed pixels
    if texel > 0.0 && paint.k5.y > 0.0 {
        rgb = rgb * (1.0 + (hash3(wp / texel + vec3<f32>(0.37)) - 0.5) * 2.0 * paint.k5.y);
    }
    let to_eye = normalize(view_bindings::view.world_position.xyz - fine);
    let fres = 1.0 - saturate(dot(pbr_input.N, to_eye));
    // the rim: what turns from the eye takes an edge of light in the look's rim colour, fuller on the lit side
    if paint.k9.x > 0.0 {
        var lightk = reach;
        if soft <= 0.0 {
            lightk = f32(lit);
        }
        rgb = rgb + c.rgb * paint.rim.rgb * pow(fres, paint.k9.y) * paint.k9.x * mix(0.3, 1.0, lightk);
    }
    // the world's style: the lit colour redrawn as that world draws things
    if wstyle.x > 0.5 && wstyle.z > 0.0 {
        let kind = u32(wstyle.x + 0.5);
        let sc = max(wstyle.y, 1e-4);
        let lum = clamp(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
        let fc = pbr_input.frag_coord.xy;
        var styled = rgb;
        if kind == 1u {
            // pixels: each cell's colour from a short ramp
            styled = floor(rgb * 5.0 + vec3<f32>(0.5)) / 5.0;
        } else if kind == 2u {
            // ink: paper and ink, the half tones hatched by lines `scale` pixels apart
            let h1 = fract((fc.x + fc.y) / sc);
            let h2 = fract((fc.x - fc.y) / sc);
            var ink = 0.0;
            if lum < 0.16 || (lum < 0.36 && (h1 < 0.38 || h2 < 0.38)) || (lum < 0.6 && h1 < 0.3) {
                ink = 1.0;
            }
            styled = mix(wb, wa, ink);
        } else if kind == 3u || kind == 5u {
            // blueprint and neon: lines ruled through the world every `scale` metres, and an edge of light
            let g = abs(fract(fine / sc + vec3<f32>(0.5)) - vec3<f32>(0.5)) * sc;
            let lw = max(0.0016 * length(view_bindings::view.world_position.xyz - fine), sc * 0.012);
            let an = abs(pbr_input.world_normal);
            var ruled = 0.0;
            if (an.x < 0.92 && g.x < lw) || (an.y < 0.92 && g.y < lw) || (an.z < 0.92 && g.z < lw) {
                ruled = 1.0;
            }
            if kind == 3u {
                styled = mix(wa * (0.45 + 0.75 * lum), wb, max(ruled, pow(fres, 2.5) * 0.7));
            } else {
                styled = wa * 0.05 + c.rgb * lum * 0.22 + wb * (ruled * 1.6 + pow(fres, 2.0) * 1.8);
            }
        } else if kind == 4u {
            // halftone: flat colour, its shade a screen of dots `scale` pixels apart
            let q = vec2<f32>(0.866 * fc.x - 0.5 * fc.y, 0.5 * fc.x + 0.866 * fc.y) / sc;
            let dd = length(fract(q) - vec2<f32>(0.5));
            let rad = sqrt(1.0 - lum) * 0.66;
            let dots = 1.0 - smoothstep(rad - 0.06, rad + 0.06, dd);
            styled = mix(c.rgb * wb, c.rgb * wa, dots);
        } else if kind == 6u {
            // thermal: the tone as heat
            if lum < 0.5 {
                styled = mix(wa, wb, lum * 2.0);
            } else {
                styled = mix(wb, vec3<f32>(1.0, 0.96, 0.75), lum * 2.0 - 1.0);
            }
        } else if kind == 7u {
            // poster: three flat tones of two colours
            if lum < 0.22 {
                styled = wa;
            } else if lum < 0.55 {
                styled = mix(wa, wb, 0.55);
            } else {
                styled = wb;
            }
        } else if kind == 8u {
            // wash: watercolour, the colour let down toward the paper and pooled by a noise `scale` metres wide
            let pool = fbm3(fine / sc);
            styled = mix(rgb, wb, 0.1 + 0.3 * pow(fres, 2.0)) * (0.84 + 0.32 * pool);
        } else if kind == 9u {
            // mono: one ink's tones on one paper
            styled = mix(wa, wb, smoothstep(0.04, 0.85, lum));
        }
        // (a mesh that is in every world, the cast, takes less than half of a world's style: it has to read in all of them)
        var much = wstyle.z;
        if paint.kw.x < -0.5 {
            much = much * 0.4;
        }
        rgb = mix(rgb, styled, much);
    }
    // a cut's edge, lit: the last `rim width` metres inside it go to the rim's colour
    if paint.cut_rim.w > 0.0 && world.x > 0.5 && world.y < paint.kw.z {
        let k = 1.0 - world.y / paint.kw.z;
        rgb = mix(rgb, paint.cut_rim.rgb * 2.5, k * k);
    }
    // saturation: painted colour is fuller than photographed colour
    let grey = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = max(vec3<f32>(grey) + (rgb - vec3<f32>(grey)) * paint.k3.z, vec3<f32>(0.0));
    // haze: air lying low over the land, thick with distance and thinning with height, so far ground and the feet
    // of things dissolve into light while their tops stand clear (a painter's aerial perspective, without a box's
    // edges); read at the true point, not the texel
    // As a painter (and an atmosphere) does it: the air takes the sky's colour, never white; it starts some way off
    // (`start`), so the foreground keeps its full colour; it never quite covers anything (`most`); it takes a
    // thing's saturation and contrast before its value, so the distance goes cool and soft rather than pale; and
    // toward the sun it glows the sun's colour
    if paint.haze.w > 0.0 {
        let eye = view_bindings::view.world_position.xyz;
        let d = max(length(fine - eye) - paint.k8.z, 0.0);
        let lift = max(fine.y - paint.k0.z, 0.0);
        let a = clamp((1.0 - exp(-d * paint.k8.x)) * exp(-lift / max(paint.k8.y, 0.01)), 0.0, 1.0) * paint.k8.w;
        var air = paint.haze.rgb;
        if paint.haze_sun.w > 0.0 && view_bindings::lights.n_directional_lights > 0u {
            let toward = max(dot(normalize(fine - eye), view_bindings::lights.directional_lights[0].direction_to_light), 0.0);
            air = mix(air, paint.haze_sun.rgb, pow(toward, 6.0));
        }
        // first the colour drains toward the air's (half again as fast as the value moves), then the value follows
        let lum = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let drained = mix(rgb, vec3<f32>(lum) * air / max(dot(air, vec3<f32>(0.2126, 0.7152, 0.0722)), 1e-3), clamp(a * 1.5, 0.0, 1.0));
        rgb = mix(drained, air, a);
    }
    out.color = vec4<f32>(rgb, c.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
}
