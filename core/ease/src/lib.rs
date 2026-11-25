//! How a value moves between two keys, as a pure function of time (forked from Animato 1.7.1, MIT OR Apache-2.0,
//! Aarambh Dev Hub; see NOTICE, and rebuilt to be frame-pure): `easing`'s curves (with text names, `reverse()` and
//! stretched springs), `spring`'s damped oscillator in closed form, `color`'s CSS colours blended in Oklab, and
//! `interpolate`, what a keyframed value has to do.

pub mod color;
pub mod easing;
pub mod interpolate;
pub mod spring;

/// 0 below `a`, 1 above `b`, and between them the cubic Hermite ease (WGSL's smoothstep), for a value that fades in
/// over a range rather than a key's time.
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
