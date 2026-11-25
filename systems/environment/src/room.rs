//! three.js's RoomEnvironment as a panorama (#165): the soft studio light a three.js scene gets from
//! `PMREMGenerator.fromScene(new RoomEnvironment(), sigma)` with no HDR file, made by raea with no file either.
//!
//! The room is three's own (three/examples/jsm/environments/RoomEnvironment.js, r185, the same since r155's physical
//! lights): a white box room, six white boxes on its floor, six light boxes on its walls and ceiling and one point light
//! (900 cd, reaching 28 m), the whole scene 3.5 m down. Each direction from fromScene's cube camera (at the origin) is
//! traced to the box it hits, and that box is coloured as three draws it into its cube (no tone mapping, linear): a
//! light box (an emissive-only MeshLambertMaterial) is its emissive intensity; a wall or a box (a white
//! MeshStandardMaterial, roughness 1, metalness 0) is lit by the point light alone with three's direct lighting
//! (RE_Direct_Physical: Lambert's diffuse and BRDF_GGX, with its Schlick Fresnel, height-correlated Smith and GGX terms,
//! and three's distance attenuation), no shadows (nothing in the room casts any). three adds a multiple-scattering
//! term to BRDF_GGX that is under 1e-4 of a white wall's diffuse at roughness 1: left out. `sigma` blurs it as
//! fromScene does before filtering (three's two half blurs about +Y, clipped at its 20 taps). The panorama is then
//! filtered like any other (Bevy's generator in the place of three's PMREM).

use bevy::math::{Quat, Vec3};

use super::Equirect;

/// An <Environment room>'s source in the camera row's env_srcs (runtime/look camera.rs): `room:` and the blur, in
/// radians.
pub const PREFIX: &str = "room:";

/// The blur of a room source (`room:0.04`), or None for a panorama file.
pub fn blur_of(src: &str) -> Option<f32> {
    let b = src.strip_prefix(PREFIX)?;
    let b: f32 = if b.is_empty() { 0.0 } else { b.parse().ok()? };
    (b.is_finite() && b >= 0.0).then_some(b)
}

/// The panorama's size: three renders the room into a cube of 256 a face, which an equirectangular 1024 wide makes.
const WIDTH: usize = 1024;

/// How a box is drawn: the room (seen from inside: three's BackSide), a white box, or a light box's emissive.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Room,
    Solid,
    Light(f32),
}

/// One of the room's boxes: three's unit BoxGeometry placed by position, a turn about +Y (its only rotation) and scale.
struct Part {
    at: Vec3,
    turn: f32,
    size: Vec3,
    kind: Kind,
}

/// RoomEnvironment's scene is moved this far up (its position.y).
const DROP: f32 = -3.5;
/// The point light: its position, intensity (cd) and distance (its decay is 2).
const LIGHT: [f32; 3] = [0.418, 16.199, 0.300];
const LIGHT_K: f32 = 900.0;
const LIGHT_REACH: f32 = 28.0;

/// RoomEnvironment's boxes, as its constructor places them.
fn parts() -> Vec<Part> {
    let p = |at: [f32; 3], turn: f32, size: [f32; 3], kind: Kind| Part { at: Vec3::new(at[0], at[1] + DROP, at[2]), turn, size: Vec3::from(size), kind };
    vec![
        p([-0.757, 13.219, 0.717], 0.0, [31.713, 28.305, 28.591], Kind::Room),
        p([-10.906, 2.009, 1.846], -0.195, [2.328, 7.905, 4.651], Kind::Solid),
        p([-5.607, -0.754, -0.758], 0.994, [1.970, 1.534, 3.955], Kind::Solid),
        p([6.167, 0.857, 7.803], 0.561, [3.927, 6.285, 3.687], Kind::Solid),
        p([-2.017, 0.018, 6.124], 0.333, [2.002, 4.566, 2.064], Kind::Solid),
        p([2.291, -0.756, -2.621], -0.286, [1.546, 1.552, 1.496], Kind::Solid),
        p([-2.193, -0.369, -5.547], 0.516, [3.875, 3.487, 2.986], Kind::Solid),
        p([-16.116, 14.37, 8.208], 0.0, [0.1, 2.428, 2.739], Kind::Light(50.0)),
        p([-16.109, 18.021, -8.207], 0.0, [0.1, 2.425, 2.751], Kind::Light(50.0)),
        p([14.904, 12.198, -1.832], 0.0, [0.15, 4.265, 6.331], Kind::Light(17.0)),
        p([-0.462, 8.89, 14.520], 0.0, [4.38, 5.441, 0.088], Kind::Light(43.0)),
        p([3.235, 11.486, -12.541], 0.0, [2.5, 2.0, 0.1], Kind::Light(20.0)),
        p([0.0, 20.0, 0.0], 0.0, [1.0, 0.1, 1.0], Kind::Light(100.0)),
    ]
}

/// Where a ray from `o` along unit `d` meets a part as three draws it (a BackSide room where the ray leaves it, a
/// FrontSide box where it enters), past three's cube camera's near plane: the distance and the face's normal, facing
/// the ray.
fn hit(part: &Part, o: Vec3, d: Vec3) -> Option<(f32, Vec3)> {
    let back = Quat::from_rotation_y(-part.turn);
    let lo = back * (o - part.at) / part.size;
    let ld = back * d / part.size;
    let (mut t0, mut t1, mut a0, mut a1) = (f32::NEG_INFINITY, f32::INFINITY, 0usize, 0usize);
    for k in 0..3 {
        if ld[k].abs() < 1e-12 {
            if lo[k].abs() > 0.5 {
                return None;
            }
            continue;
        }
        let (mut ta, mut tb) = ((-0.5 - lo[k]) / ld[k], (0.5 - lo[k]) / ld[k]);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        if ta > t0 {
            (t0, a0) = (ta, k);
        }
        if tb < t1 {
            (t1, a1) = (tb, k);
        }
    }
    if t0 > t1 {
        return None;
    }
    let (t, axis) = if part.kind == Kind::Room { (t1, a1) } else { (t0, a0) };
    if t < 0.1 {
        return None;
    }
    let mut n = Vec3::ZERO;
    n[axis] = (lo[axis] + ld[axis] * t).signum();
    if part.kind == Kind::Room {
        n = -n;
    }
    Some((t, Quat::from_rotation_y(part.turn) * n))
}

/// three's BRDF_GGX for a white dielectric at roughness 1 (f0 0.04, f90 1, alpha 1).
fn ggx(l: Vec3, v: Vec3, n: Vec3) -> f32 {
    let sat = |x: f32| x.clamp(0.0, 1.0);
    let (nl, nv, vh) = (sat(n.dot(l)), sat(n.dot(v)), sat(v.dot((l + v).normalize())));
    let fresnel = ((-5.55473 * vh - 6.98316) * vh).exp2();
    let f = 0.04 * (1.0 - fresnel) + fresnel;
    // at alpha 1 (a2 = 1) V_GGX_SmithCorrelated is 0.5 / (nl + nv) and D_GGX is 1 / pi, whatever the half vector
    f * 0.5 / (nl + nv).max(1e-6) / std::f32::consts::PI
}

/// The room's light along a world direction from its cube camera at the origin.
fn radiance(parts: &[Part], d: Vec3) -> f32 {
    let mut best: Option<(f32, Vec3, Kind)> = None;
    for p in parts {
        if let Some((t, n)) = hit(p, Vec3::ZERO, d) {
            if best.is_none_or(|b| t < b.0) {
                best = Some((t, n, p.kind));
            }
        }
    }
    let Some((t, n, kind)) = best else { return 0.0 };
    if let Kind::Light(k) = kind {
        return k;
    }
    let at = d * t;
    let to = Vec3::from(LIGHT) + Vec3::new(0.0, DROP, 0.0) - at;
    let dist = to.length();
    let l = to / dist;
    // getDistanceAttenuation(distance, 28, 2)
    let fall = 1.0 / (dist * dist).max(0.01) * (1.0 - (dist / LIGHT_REACH).powi(4)).clamp(0.0, 1.0).powi(2);
    let irradiance = n.dot(l).clamp(0.0, 1.0) * LIGHT_K * fall;
    irradiance / std::f32::consts::PI + irradiance * ggx(l, -d, n)
}

/// A pixel's world direction in raea's panoramas (Equirect::sample's: the middle straight ahead, -Z).
fn direction(x: f32, y: f32, w: usize, h: usize) -> Vec3 {
    let lon = (x / w as f32 - 0.5) * std::f32::consts::TAU;
    let polar = y / h as f32 * std::f32::consts::PI;
    Vec3::new(polar.sin() * lon.sin(), polar.cos(), -polar.sin() * lon.cos())
}

/// three's RoomEnvironment as a panorama, blurred by `sigma` radians as fromScene blurs it.
pub fn room(sigma: f32) -> Equirect {
    let (w, h) = (WIDTH, WIDTH / 2);
    let parts = parts();
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let v = radiance(&parts, direction(x as f32 + 0.5, y as f32 + 0.5, w, h));
            px.push([v; 3]);
        }
    }
    let mut eq = Equirect { w, h, px };
    if sigma > 0.0 {
        blur(&mut eq, sigma);
    }
    eq
}

/// three's fromScene blur (PMREMGenerator._blur about its default pole, +Y) on the panorama. three blurs its 256 cube
/// twice: turning each direction about +Y (along its latitude, by angle, not arc) and then along its meridian through
/// the pole, each with taps a texel (pi / 510 radians) apart out to 3 sigma, Gaussian weights summing to 1, and at
/// most 20 taps (a larger sigma is clipped to 19 texels a side, which three warns of). On a panorama both are
/// straight lines: a row, then a column carried over the pole.
fn blur(eq: &mut Equirect, sigma: f32) {
    let step = std::f32::consts::PI / (2.0 * 255.0);
    let sigma_px = sigma / step;
    let samples = (1 + (3.0 * sigma_px).floor() as usize).min(20);
    let weight = |i: usize| (-(i as f32 / sigma_px).powi(2) / 2.0).exp();
    let sum: f32 = weight(0) + (1..samples).map(|i| 2.0 * weight(i)).sum::<f32>();
    let taps: Vec<f32> = (0..samples).map(|i| weight(i) / sum).collect();
    let (w, h) = (eq.w, eq.h);
    let at = |px: &[[f32; 3]], x: f32, y: usize| {
        let fx = x.rem_euclid(w as f32);
        let x0 = fx.floor() as usize % w;
        let t = fx - fx.floor();
        let (a, b) = (px[y * w + x0][0], px[y * w + (x0 + 1) % w][0]);
        a * (1.0 - t) + b * t
    };
    // along each row: a turn about +Y by i steps is i steps of longitude
    let dx = step * w as f32 / std::f32::consts::TAU;
    let src = eq.px.clone();
    for y in 0..h {
        for x in 0..w {
            let mut v = taps[0] * src[y * w + x][0];
            for (i, k) in taps.iter().enumerate().skip(1) {
                v += k * (at(&src, x as f32 - i as f32 * dx, y) + at(&src, x as f32 + i as f32 * dx, y));
            }
            eq.px[y * w + x] = [v; 3];
        }
    }
    // along each meridian, over the pole onto the opposite one
    let dy = step * h as f32 / std::f32::consts::PI;
    let src = eq.px.clone();
    let meridian = |x: usize, y: f32| {
        let (mut fy, mut fx) = (y, x as f32);
        if fy < 0.0 {
            (fy, fx) = (-fy, fx + w as f32 / 2.0);
        } else if fy > h as f32 {
            (fy, fx) = (2.0 * h as f32 - fy, fx + w as f32 / 2.0);
        }
        let fy = (fy - 0.5).clamp(0.0, (h - 1) as f32);
        let y0 = fy.floor() as usize;
        let t = fy - y0 as f32;
        at(&src, fx, y0) * (1.0 - t) + at(&src, fx, (y0 + 1).min(h - 1)) * t
    };
    for y in 0..h {
        for x in 0..w {
            let c = y as f32 + 0.5;
            let mut v = taps[0] * src[y * w + x][0];
            for (i, k) in taps.iter().enumerate().skip(1) {
                v += k * (meridian(x, c - i as f32 * dy) + meridian(x, c + i as f32 * dy));
            }
            eq.px[y * w + x] = [v; 3];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Directions and the light three draws there, from three's own RoomEnvironment object (its matrices and
    /// materials, traced and lit by three's formulas with the multiple-scattering term in): each box, three of the light
    /// boxes, walls, the ceiling, the floor and a corner.
    const THREE: [(&str, [f32; 3], f32); 14] = [
        ("box1", [-0.977138, -0.133588, 0.165395], 0.3917562),
        ("box2", [-0.792082, -0.600949, -0.107080], 0.7193024),
        ("box3", [0.599263, -0.256827, 0.758237], 0.3620106),
        ("box4", [-0.275255, -0.475180, 0.835727], 0.1959535),
        ("box5", [0.416670, -0.774050, -0.476688], 0.8218551),
        ("box6", [-0.308452, -0.544187, -0.780203], 0.2548782),
        ("light1", [-0.763754, 0.515141, 0.388986], 50.0),
        ("light4", [-0.029816, 0.347853, 0.937075], 43.0),
        ("light5", [0.212609, 0.524852, -0.824213], 20.0),
        ("wall", [0.309426, 0.206284, -0.928279], 0.5509641),
        ("light1 again", [-0.780399, 0.520266, 0.346844], 50.0),
        ("ceiling", [0.431934, 0.863868, 0.259161], 0.4399573),
        ("floor", [0.512148, -0.384111, 0.768221], 0.2772276),
        ("corner", [-0.577350, -0.577350, -0.577350], 0.5250023),
    ];

    #[test]
    fn the_room_is_lit_as_three_draws_it() {
        let parts = parts();
        for (name, d, want) in THREE {
            let got = radiance(&parts, Vec3::from(d).normalize());
            assert!((got - want).abs() <= want * 1e-3, "{name}: {got}, three draws {want}");
        }
        // straight up is the ceiling's light box, straight down the floor under the light
        assert_eq!(radiance(&parts, Vec3::Y), 100.0);
        assert!((radiance(&parts, -Vec3::Y) - 0.72742).abs() < 1e-3);
    }

    #[test]
    fn a_pixel_is_the_direction_sample_reads() {
        let eq = room(0.0);
        assert_eq!((eq.w, eq.h), (1024, 512));
        let parts = parts();
        for (name, d, _) in THREE {
            let d = Vec3::from(d).normalize();
            let (got, want) = (eq.sample(d)[0], radiance(&parts, d));
            // the panorama's bilinear sample of a smooth wall is the light there; a box's or light's edge may be a
            // pixel off, so only the smooth ones
            if want < 10.0 && !name.starts_with("box") {
                assert!((got - want).abs() < want * 0.02, "{name}: the panorama gives {got}, the room {want}");
            }
        }
    }

    #[test]
    fn the_blur_averages_and_softens_an_edge() {
        // its weights sum to 1: light the same everywhere stays so, over the pole too
        let mut even = Equirect { w: 64, h: 32, px: vec![[2.0; 3]; 64 * 32] };
        blur(&mut even, 0.04);
        assert!(even.px.iter().all(|p| (p[0] - 2.0).abs() < 1e-4));
        let (sharp, soft, wide) = (room(0.0), room(0.04), room(0.2));
        // a smooth wall barely changes (the ceiling over the light, curved, by 1.7%)
        for (name, d, _) in [THREE[9], THREE[11], THREE[12]] {
            let (a, b) = (sharp.sample(Vec3::from(d))[0], soft.sample(Vec3::from(d))[0]);
            assert!((a - b).abs() < a * 0.03, "{name}: {a} blurred to {b}");
        }
        // the ceiling's light box (1 m across, 16.5 m up: 0.06 radians) is softened by 0.04 at its middle, and a
        // larger sigma, clipped at 19 texels a side, softens it more
        let up = |e: &Equirect| e.sample(Vec3::Y)[0];
        assert_eq!(up(&sharp), 100.0);
        assert!(up(&soft) < 90.0 && up(&soft) > 30.0, "{}", up(&soft));
        assert!(up(&wide) < up(&soft), "{} vs {}", up(&wide), up(&soft));
    }

    #[test]
    fn a_room_source_is_named_by_its_blur() {
        assert_eq!(blur_of("room:0.04"), Some(0.04));
        assert_eq!(blur_of("room:0"), Some(0.0));
        assert_eq!(blur_of("room:"), Some(0.0));
        assert_eq!(blur_of("sky.hdr"), None);
        assert_eq!(blur_of("room:-1"), None);
    }
}
