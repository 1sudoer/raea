// paint_world.wesl: the paint look's worlds (a <Look cuts>). A cut is a ball in the world; a point is in the last cut
// that holds it (world 1 to 4), or in none (world 0). A mesh is drawn only where the point is in its own world (a
// PaintExtension's `world`; 0 by default, so the scene outside every cut; -1: everywhere), in the main pass and the
// prepass alike (paint.wesl, paint_prepass.wesl), so one world is cut against the next pixel by pixel, in depth, and
// the outline and the ambient occlusion follow the cut. `rag` (m) tears the cuts' edges by a noise fixed in the world.

fn wh3(p: vec3<f32>) -> f32 {
    var q = bitcast<vec3<u32>>(vec3<i32>(floor(p)));
    q = q * 1664525u + 1013904223u;
    q.x += q.y * q.z;
    q.y += q.z * q.x;
    q.z += q.x * q.y;
    q = q ^ (q >> vec3<u32>(16u));
    return f32(q.x + q.y + q.z) * (1.0 / 4294967295.0);
}

fn wnoise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    let f = fract(x);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(wh3(i), wh3(i + vec3(1.0, 0.0, 0.0)), u.x), mix(wh3(i + vec3(0.0, 1.0, 0.0)), wh3(i + vec3(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(wh3(i + vec3(0.0, 0.0, 1.0)), wh3(i + vec3(1.0, 0.0, 1.0)), u.x), mix(wh3(i + vec3(0.0, 1.0, 1.0)), wh3(i + vec3(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

/// how far inside a cut a point is (m; negative outside), its edge torn by `n` (less while the cut is small)
fn cut_depth(p: vec3<f32>, c: vec4<f32>, n: f32) -> f32 {
    return c.w - length(p - c.xyz) + n * saturate(c.w * 0.5);
}

/// the world a point is in: x the last cut that holds it (1 to 4; 0: none), y how far inside that cut's edge (m)
fn world_at(p: vec3<f32>, c0: vec4<f32>, c1: vec4<f32>, c2: vec4<f32>, c3: vec4<f32>, rag: f32) -> vec2<f32> {
    var w = vec2<f32>(0.0, 0.0);
    if c0.w <= 0.0 && c1.w <= 0.0 && c2.w <= 0.0 && c3.w <= 0.0 {
        return w;
    }
    var n = 0.0;
    if rag > 0.0 {
        n = (wnoise(p * 1.9) * 0.65 + wnoise(p * 5.3 + vec3<f32>(3.7)) * 0.35 - 0.5) * 2.0 * rag;
    }
    var d = cut_depth(p, c0, n);
    if c0.w > 0.0 && d > 0.0 {
        w = vec2<f32>(1.0, d);
    }
    d = cut_depth(p, c1, n);
    if c1.w > 0.0 && d > 0.0 {
        w = vec2<f32>(2.0, d);
    }
    d = cut_depth(p, c2, n);
    if c2.w > 0.0 && d > 0.0 {
        w = vec2<f32>(3.0, d);
    }
    d = cut_depth(p, c3, n);
    if c3.w > 0.0 && d > 0.0 {
        w = vec2<f32>(4.0, d);
    }
    return w;
}

/// whether a mesh of world `mesh` is drawn at a point in world `world`
fn world_shows(world: f32, mesh: f32) -> bool {
    return mesh < -0.5 || abs(world - mesh) < 0.5;
}
