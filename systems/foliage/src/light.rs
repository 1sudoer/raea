//! What each leaf sees of the sky through the crown's own leaves (docs/foliage.feature FO2). The crown's leaf area is
//! put in a grid of CELL m cells as leaf area density (m2 of leaf a m3); from each leaf, rays go out along 19
//! directions through it, and each lets through e^(-G * LAD * path) of the light (Beer and Lambert, with G = 0.5 for
//! leaves at random angles: Monsi and Saeki 1953). A leaf's sky view is the upper rays' light, weighted by their sine
//! of elevation (a cloudy sky's light on a level leaf); its bent normal is the rays' directions weighted by what they
//! let through. So a clump's outer face is bright and faces out of the clump, the hollows between clumps and the
//! crown's core are dark, and the crown shades as many masses, not as one ball.

use parry3d::glamx::Vec3;

const CELL: f32 = 0.5;
const G: f32 = 0.5;

fn dirs() -> Vec<(Vec3, f32)> {
    let mut out = vec![(Vec3::Y, 1.0)];
    for (elev, off) in [(60f32, 0.0f32), (20.0, 30.0), (-25.0, 0.0)] {
        for k in 0..6 {
            let (e, a) = (elev.to_radians(), (off + 60.0 * k as f32).to_radians());
            out.push((Vec3::new(e.cos() * a.cos(), e.sin(), e.cos() * a.sin()), e.sin()));
        }
    }
    out
}

/// For leaves at `at` with leaf area `area` (m2): each one's sky view (0-1) and bent normal.
pub(crate) fn light(at: &[Vec3], area: &[f32]) -> (Vec<f32>, Vec<Vec3>) {
    if at.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let lo = at.iter().fold(Vec3::splat(f32::INFINITY), |a, &p| a.min(p)) - Vec3::splat(CELL);
    let hi = at.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, &p| a.max(p)) + Vec3::splat(CELL);
    let n = ((hi - lo) / CELL).ceil();
    let (nx, ny, nz) = (n.x as usize, n.y as usize, n.z as usize);
    let index = |p: Vec3| -> Option<usize> {
        let q = (p - lo) / CELL;
        (q.x >= 0.0 && q.y >= 0.0 && q.z >= 0.0 && (q.x as usize) < nx && (q.y as usize) < ny && (q.z as usize) < nz).then(|| (q.x as usize * ny + q.y as usize) * nz + q.z as usize)
    };
    let mut lad = vec![0f32; nx * ny * nz];
    for (&p, &a) in at.iter().zip(area) {
        if let Some(i) = index(p) {
            lad[i] += a / (CELL * CELL * CELL);
        }
    }
    let dirs = dirs();
    let up: f32 = dirs.iter().filter(|d| d.1 > 0.0).map(|d| d.1).sum();
    let step = CELL * 0.5;
    let mut sky = Vec::with_capacity(at.len());
    let mut bent = Vec::with_capacity(at.len());
    for &p in at {
        let (mut s, mut b) = (0.0, Vec3::ZERO);
        for &(d, w) in &dirs {
            // from just outside the leaf's own cell share, out to the grid's edge
            let mut q = p + d * (CELL * 0.6);
            let mut tau = 0.0;
            while let Some(i) = index(q) {
                tau += G * lad[i] * step;
                q += d * step;
                if tau > 8.0 {
                    break;
                }
            }
            let t = (-tau).exp();
            b += d * t;
            if w > 0.0 {
                s += w * t;
            }
        }
        sky.push(s / up);
        bent.push(b.normalize_or(Vec3::Y));
    }
    (sky, bent)
}
