//! A plant seen from the side as a mask (docs/foliage.feature FO12): its leaves through the atlas's alpha and its
//! bark's triangles, white on black, the ground at the bottom row. The project's crown check measures this and
//! photographs of real trees the same way (outline against its hull, sky inside), so a crown is held to real ones.

use image::{GrayImage, Luma, RgbaImage};

use crate::{Plant, Request, atlas, species};

/// The side view from `bearing` (radians round the trunk), `px` tall.
pub fn silhouette(req: &Request, plant: &Plant, px: u32, bearing: f32) -> Result<GrayImage, String> {
    let sp = species(&req.species).ok_or("no species")?;
    let sheet = atlas(sp, req.seed);
    let (c, s) = (bearing.cos(), bearing.sin());
    let flat = |p: [f32; 3]| (p[0] * c + p[2] * s, p[1]);
    let all = || std::iter::once(&plant.bark).chain(&plant.leaves).flat_map(|m| m.pos.iter().map(|&p| flat(p)));
    let (x0, x1) = all().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), (x, _)| (a.min(x), b.max(x)));
    let top = all().fold(0.0f32, |a, (_, y)| a.max(y));
    let scale = (px as f32 - 4.0) / top.max(1e-3);
    let mut img = GrayImage::new(((x1 - x0) * scale) as u32 + 5, px);
    let to = |p: [f32; 3]| {
        let (x, y) = flat(p);
        ((x - x0) * scale + 2.0, px as f32 - 2.0 - y * scale)
    };
    for (k, m) in std::iter::once(&plant.bark).chain(&plant.leaves).enumerate() {
        for t in m.idx.chunks_exact(3) {
            let v = [t[0] as usize, t[1] as usize, t[2] as usize];
            let uv = (k > 0).then(|| v.map(|i| m.uv[i]));
            fill(&mut img, v.map(|i| to(m.pos[i])), uv, &sheet);
        }
    }
    Ok(img)
}

fn fill(img: &mut GrayImage, t: [(f32, f32); 3], uv: Option<[[f32; 2]; 3]>, sheet: &RgbaImage) {
    let area = (t[1].0 - t[0].0) * (t[2].1 - t[0].1) - (t[2].0 - t[0].0) * (t[1].1 - t[0].1);
    if area.abs() < 1e-9 {
        return;
    }
    let (w, h) = (img.width() as i32, img.height() as i32);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in t {
        (x0, y0, x1, y1) = (x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1));
    }
    for y in (y0.floor() as i32).max(0)..=(y1.ceil() as i32).min(h - 1) {
        for x in (x0.floor() as i32).max(0)..=(x1.ceil() as i32).min(w - 1) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((t[1].0 - px) * (t[2].1 - py) - (t[2].0 - px) * (t[1].1 - py)) / area;
            let w1 = ((t[2].0 - px) * (t[0].1 - py) - (t[0].0 - px) * (t[2].1 - py)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            if let Some(uv) = uv {
                let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
                let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
                let (sw, sh) = (sheet.width() as f32, sheet.height() as f32);
                if sheet.get_pixel((u * sw).clamp(0.0, sw - 1.0) as u32, (v * sh).clamp(0.0, sh - 1.0) as u32)[3] < 128 {
                    continue;
                }
            }
            img.put_pixel(x as u32, y as u32, Luma([255]));
        }
    }
}
