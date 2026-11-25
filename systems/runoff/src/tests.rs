//! A wall 4 m wide and 6 m high facing +z, standing on the ground, with a stone sill and an iron bracket on it. The
//! wall alone is in the atlas (D texels a metre, row 0 at its top); every other face sends its texels to one junk
//! texel past the wall's columns.

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;

use crate::{Drop, Kind, Out, Role, Surface, run};

const D: f32 = 10.0;
const W: f32 = 4.0;
const H: f32 = 6.0;
const AW: u32 = (W * D) as u32 + 1;
const AH: u32 = (H * D) as u32;

struct Model {
    pos: Vec<Vec3>,
    tri: Vec<[u32; 3]>,
    nrm: Vec<Vec3>,
    role: Vec<Role>,
    uv: Vec<[[f32; 2]; 3]>,
}

const PLAIN: Role = Role { gives: Kind::None, walk: false, drains: false };

impl Model {
    fn quad(&mut self, c: [Vec3; 4], role: Role, wall: bool) {
        let at = self.pos.len() as u32;
        self.pos.extend(c);
        let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize();
        let uv = |p: Vec3| if wall { [p.x * D, (H - p.y) * D] } else { [AW as f32 - 0.5, 0.5] };
        for t in [[at, at + 1, at + 2], [at, at + 2, at + 3]] {
            self.tri.push(t);
            self.nrm.push(n);
            self.role.push(role);
            self.uv.push(t.map(|i| uv(self.pos[i as usize])));
        }
    }
    /// An axis-aligned box's six faces, each wound outward.
    fn cuboid(&mut self, lo: Vec3, hi: Vec3, role: Role) {
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let (a, b) = (lo, hi);
        self.quad([v(a.x, a.y, b.z), v(b.x, a.y, b.z), v(b.x, b.y, b.z), v(a.x, b.y, b.z)], role, false); // +z
        self.quad([v(b.x, a.y, a.z), v(a.x, a.y, a.z), v(a.x, b.y, a.z), v(b.x, b.y, a.z)], role, false); // -z
        self.quad([v(b.x, a.y, b.z), v(b.x, a.y, a.z), v(b.x, b.y, a.z), v(b.x, b.y, b.z)], role, false); // +x
        self.quad([v(a.x, a.y, a.z), v(a.x, a.y, b.z), v(a.x, b.y, b.z), v(a.x, b.y, a.z)], role, false); // -x
        self.quad([v(a.x, b.y, b.z), v(b.x, b.y, b.z), v(b.x, b.y, a.z), v(a.x, b.y, a.z)], role, false); // +y
        self.quad([v(a.x, a.y, a.z), v(b.x, a.y, a.z), v(b.x, a.y, b.z), v(a.x, a.y, b.z)], role, false); // -y
    }
}

fn front() -> Model {
    let mut m = Model { pos: vec![], tri: vec![], nrm: vec![], role: vec![], uv: vec![] };
    let v = Vec3::new;
    m.quad([v(0.0, 0.0, 0.0), v(W, 0.0, 0.0), v(W, H, 0.0), v(0.0, H, 0.0)], PLAIN, true);
    m.quad([v(-2.0, 0.0, 4.0), v(6.0, 0.0, 4.0), v(6.0, 0.0, -2.0), v(-2.0, 0.0, -2.0)], Role { walk: true, ..PLAIN }, false);
    // a sill 1.6 m long, 0.18 deep, its top dusty; an iron bracket further along
    m.cuboid(v(1.0, 3.0, 0.0), v(2.6, 3.1, 0.18), Role { gives: Kind::Grime, ..PLAIN });
    m.cuboid(v(3.0, 4.0, 0.0), v(3.4, 4.4, 0.1), Role { gives: Kind::Rust, ..PLAIN });
    m
}

/// Rain on every level top and on the wall above the sill: a drop every 2 cm.
fn rain(m: &Model, mesh: &TriMesh) -> Vec<Drop> {
    let _ = mesh;
    let mut out = vec![];
    for (t, c) in m.tri.iter().enumerate() {
        let n = m.nrm[t];
        let up_top = n.y > 0.9 && !m.role[t].walk;
        let wall_above = m.uv[t][0][0] < AW as f32 - 1.0 && n.z > 0.9;
        if !(up_top || wall_above) {
            continue;
        }
        let [a, b, cc] = c.map(|i| m.pos[i as usize]);
        for i in 0..60 {
            for j in 0..60 - i {
                let (u, w) = ((i as f32 + 0.33) / 60.0, (j as f32 + 0.33) / 60.0);
                let p = a + (b - a) * u + (cc - a) * w;
                if wall_above && p.y < 3.1 {
                    continue;
                }
                out.push(Drop { p: p + n * 0.001, tri: t as u32, water: if up_top { 1e-3 } else { 2e-4 } });
            }
        }
    }
    out
}

fn bake(m: &Model) -> Out {
    let mesh = TriMesh::new(m.pos.clone(), m.tri.clone()).unwrap();
    let s = Surface { pos: &m.pos, tri: &m.tri, nrm: &m.nrm, role: &m.role, uv: &m.uv, mesh: &mesh };
    run(&s, &rain(m, &mesh), Vec3::Y, AW, AH)
}

/// The sum over wall texels with x in [x0, x1) metres and height in [y0, y1).
fn sum(v: &[f32], x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    let mut s = 0.0;
    for ty in ((H - y1) * D) as u32..((H - y0) * D) as u32 {
        for tx in (x0 * D) as u32..(x1 * D) as u32 {
            s += v[(ty * AW + tx) as usize];
        }
    }
    s
}

#[test]
fn a_sill_streaks_the_wall_under_its_ends_not_its_middle() {
    let o = bake(&front());
    let ends = sum(&o.flow, 0.85, 1.05, 1.0, 2.8) + sum(&o.flow, 2.55, 2.75, 1.0, 2.8);
    let middle = sum(&o.flow, 1.5, 2.1, 1.0, 2.8);
    assert!(ends > 0.0, "nothing ran down from the sill's ends");
    assert!(ends > 5.0 * middle, "ends {ends} middle {middle}");
    // and the wall beside the sill, clear of it, gets only the rain on itself above
    let beside = sum(&o.flow, 0.2, 0.6, 1.0, 2.8);
    assert!(ends > 3.0 * beside, "ends {ends} beside {beside}");
}

#[test]
fn iron_bleeds_rust_onto_the_wall_below_it_and_not_beside_it() {
    let o = bake(&front());
    let kinds_below: Vec<Kind> = (((H - 3.9) * D) as u32..((H - 2.0) * D) as u32)
        .flat_map(|ty| ((2.85 * D) as u32..(3.55 * D) as u32).map(move |tx| (ty * AW + tx) as usize))
        .filter(|&i| o.deposit[i] > 0.0)
        .map(|i| o.kind[i])
        .collect();
    assert!(!kinds_below.is_empty(), "nothing left under the bracket");
    assert!(kinds_below.iter().all(|k| *k == Kind::Rust), "{kinds_below:?}");
    assert_eq!(sum(&o.deposit, 0.1, 0.8, 0.5, 5.5), 0.0, "rust beside the bracket");
}

#[test]
fn the_same_rain_gives_the_same_sums() {
    let (a, b) = (bake(&front()), bake(&front()));
    assert_eq!(a.flow, b.flow);
    assert_eq!(a.deposit, b.deposit);
}

#[test]
fn a_drop_on_a_face_that_drains_ends_there() {
    let mut m = front();
    for r in m.role.iter_mut() {
        if !r.walk {
            r.drains = true;
        }
    }
    let o = bake(&m);
    assert_eq!(sum(&o.flow, 0.0, W, 0.0, H), 0.0);
}
