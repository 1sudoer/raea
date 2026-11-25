//! Synthetic fronts: a door in a wall, a pair of leaves, a flight of steps beside a sill, a passage and a gateway, a
//! rail. Points are sampled on faces as an atlas's texels would be; the ground is the plane y = 0.

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};
use parry3d::shape::TriMesh;

use crate::{Mat, Point, Spot, Surface, mark};

#[derive(Default)]
struct B {
    pos: Vec<Vec3>,
    tri: Vec<[u32; 3]>,
    nrm: Vec<Vec3>,
    mat: Vec<Mat>,
    walk: Vec<bool>,
}

impl B {
    /// A quad facing `out`, its corners in order around it.
    fn quad(&mut self, c: [Vec3; 4], out: Vec3, m: Mat, walk: bool) {
        let n = (c[1] - c[0]).cross(c[2] - c[0]).normalize();
        let c = if n.dot(out) < 0.0 { [c[0], c[3], c[2], c[1]] } else { c };
        let i = self.pos.len() as u32;
        self.pos.extend(c);
        for t in [[i, i + 1, i + 2], [i, i + 2, i + 3]] {
            self.tri.push(t);
            self.nrm.push(out);
            self.mat.push(m);
            self.walk.push(walk);
        }
    }
    fn cube(&mut self, a: [f32; 3], b: [f32; 3], m: Mat) {
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let ([x0, y0, z0], [x1, y1, z1]) = (a, b);
        self.quad([v(x0, y0, z0), v(x0, y0, z1), v(x0, y1, z1), v(x0, y1, z0)], -Vec3::X, m, false);
        self.quad([v(x1, y0, z0), v(x1, y0, z1), v(x1, y1, z1), v(x1, y1, z0)], Vec3::X, m, false);
        self.quad([v(x0, y0, z0), v(x1, y0, z0), v(x1, y0, z1), v(x0, y0, z1)], -Vec3::Y, m, false);
        self.quad([v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)], Vec3::Y, m, false);
        self.quad([v(x0, y0, z0), v(x1, y0, z0), v(x1, y1, z0), v(x0, y1, z0)], -Vec3::Z, m, false);
        self.quad([v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)], Vec3::Z, m, false);
    }
    fn ground(mut self) -> B {
        let v = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        self.quad([v(-20.0, -20.0), v(20.0, -20.0), v(20.0, 20.0), v(-20.0, 20.0)], Vec3::Y, Mat::Other, true);
        self
    }
    fn mesh(&self) -> TriMesh {
        TriMesh::new(self.pos.clone(), self.tri.clone()).unwrap()
    }
    fn surface<'a>(&'a self, mesh: &'a TriMesh) -> Surface<'a> {
        Surface { pos: &self.pos, tri: &self.tri, nrm: &self.nrm, mat: &self.mat, walk: &self.walk, mesh, up: Vec3::Y }
    }
}

/// The point on the face at p looking along n (its triangle found by a ray from just in front).
fn at(mesh: &TriMesh, p: Vec3, n: Vec3) -> Point {
    let h = mesh.cast_local_ray_and_get_normal(&Ray::new(p + n * 0.02, -n), 0.05, false).expect("a face at the point");
    Point { p, n, tri: h.subshape, ground: p.y, rain: 1.0, flow: 0.0, edge: 0.0 }
}

/// A wall with a door opening 1.1 m wide and 2.3 m high at x = 0, its front at z = 0.
fn front(b: &mut B, half: f32) {
    b.cube([-6.0, 0.0, -0.5], [-half, 8.0, 0.0], Mat::Plaster);
    b.cube([half, 0.0, -0.5], [6.0, 8.0, 0.0], Mat::Plaster);
    b.cube([-half, 2.3, -0.5], [half, 8.0, 0.0], Mat::Plaster);
}

/// The leaf's front face, every 5 cm.
fn leaf_face(mesh: &TriMesh, x0: f32, x1: f32) -> Vec<Point> {
    let mut out = Vec::new();
    let mut x = x0 + 0.025;
    while x < x1 {
        for j in 0..44 {
            out.push(at(mesh, Vec3::new(x, 0.025 + j as f32 * 0.05, -0.25), Vec3::Z));
        }
        x += 0.05;
    }
    out
}

#[test]
fn handle_band_lands_on_the_lock_side_and_nowhere_else() {
    let mut b = B::default();
    front(&mut b, 0.55);
    b.cube([-0.5, 0.0, -0.3], [0.5, 2.25, -0.25], Mat::Wood);
    let b = b.ground();
    let mesh = b.mesh();
    let s = b.surface(&mesh);
    let mut pts = leaf_face(&mesh, -0.5, 0.5);
    let wall: Vec<Point> = (0..20).map(|j| at(&mesh, Vec3::new(3.0, 0.05 + j as f32 * 0.1, 0.0), Vec3::Z)).collect();
    pts.extend(&wall);
    let (touch, spots) = mark(&s, &pts);
    let held = |side: f32| pts.iter().zip(&touch).any(|(q, &v)| v > 0.5 && (0.9..1.15).contains(&q.p.y) && q.p.x * side > 0.2);
    assert!(held(1.0) != held(-1.0), "the handle band on exactly one side");
    let lock = if held(1.0) { 1.0 } else { -1.0 };
    for (q, &v) in pts.iter().zip(&touch) {
        let y = q.p.y;
        if q.p.z < -0.1 && y < 0.25 {
            let from_lock = 0.5 - q.p.x * lock;
            assert!(v > 0.3 && (from_lock > 0.2 || v > 0.85), "kick band at {:?}: {v}", q.p);
        }
        if (0.4..0.85).contains(&y) || y > 1.2 || q.p.z > -0.1 {
            assert_eq!(v, 0.0, "nothing at {:?}", q.p);
        }
    }
    let names: Vec<&Spot> = spots.iter().filter(|s| s.what == "nameplate").collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].at.x * lock > 0.5 && (names[0].at.y - 1.5).abs() < 0.01, "the nameplate beside the lock side: {:?}", names[0].at);
}

#[test]
fn a_pair_of_leaves_is_held_where_they_meet() {
    let mut b = B::default();
    front(&mut b, 1.05);
    b.cube([-1.0, 0.0, -0.3], [-0.005, 2.25, -0.25], Mat::Wood);
    b.cube([0.005, 0.0, -0.3], [1.0, 2.25, -0.25], Mat::Wood);
    let b = b.ground();
    let mesh = b.mesh();
    let pts = leaf_face(&mesh, -1.0, 1.0);
    let (touch, _) = mark(&b.surface(&mesh), &pts);
    for (q, &v) in pts.iter().zip(&touch) {
        if (0.95..=1.1).contains(&q.p.y) {
            assert_eq!(v > 0.0, q.p.x.abs() < 0.3, "at {:?}: {v}", q.p);
        }
    }
}

#[test]
fn treads_wear_at_the_middle_and_a_sill_does_not() {
    let mut b = B::default();
    for i in 0..3 {
        b.cube([-0.75, 0.0, -0.3 * (i + 1) as f32], [0.75, 0.17 * (i + 1) as f32, -0.3 * i as f32], Mat::Masonry);
    }
    b.cube([3.0, 0.9, 0.0], [4.2, 0.97, 0.25], Mat::Masonry);
    let b = b.ground();
    let mesh = b.mesh();
    let pts = vec![
        at(&mesh, Vec3::new(0.0, 0.34, -0.45), Vec3::Y),
        at(&mesh, Vec3::new(0.7, 0.34, -0.45), Vec3::Y),
        at(&mesh, Vec3::new(0.0, 0.17, -0.15), Vec3::Y),
        at(&mesh, Vec3::new(3.6, 0.97, 0.12), Vec3::Y),
    ];
    let (touch, _) = mark(&b.surface(&mesh), &pts);
    assert!(touch[0] > 0.9 && touch[2] > 0.9, "middles worn: {touch:?}");
    assert!(touch[1] < 0.3, "the end barely: {touch:?}");
    assert_eq!(touch[3], 0.0, "the sill isn't a tread");
}

#[test]
fn passage_shoulders_and_gateway_hubs() {
    let walls = |roof: bool| {
        let mut b = B::default();
        b.cube([-1.55, 0.0, -5.0], [-1.25, 4.0, 5.0], Mat::Masonry);
        b.cube([1.25, 0.0, -5.0], [1.55, 4.0, 5.0], Mat::Masonry);
        if roof {
            b.cube([-1.55, 4.0, -5.0], [1.55, 4.3, 5.0], Mat::Plaster);
        }
        b.ground()
    };
    for roof in [false, true] {
        let b = walls(roof);
        let mesh = b.mesh();
        let pts: Vec<Point> = [0.6, 1.0, 1.4].iter().map(|&y| at(&mesh, Vec3::new(-1.25, y, 0.0), Vec3::X)).collect();
        let (touch, _) = mark(&b.surface(&mesh), &pts);
        assert!((touch[2] - 0.8).abs() < 1e-4, "shoulder: {touch:?}");
        assert_eq!(touch[1], 0.0);
        assert_eq!(touch[0], if roof { 1.0 } else { 0.0 }, "hub only under a ceiling");
    }
}

#[test]
fn a_rails_top_is_held() {
    let mut b = B::default();
    b.cube([-1.0, 0.9, 2.0], [1.0, 0.95, 2.05], Mat::Metal);
    let b = b.ground();
    let mesh = b.mesh();
    let pts = vec![at(&mesh, Vec3::new(0.3, 0.95, 2.025), Vec3::Y), at(&mesh, Vec3::new(0.3, 0.92, 2.05), Vec3::Z)];
    let (touch, _) = mark(&b.surface(&mesh), &pts);
    assert_eq!(touch, vec![1.0, 0.0]);
}

#[test]
fn street_plates_on_a_corner_and_the_same_twice() {
    let mut b = B::default();
    b.cube([-6.0, 0.0, -8.0], [0.0, 8.0, 0.0], Mat::Plaster);
    let b = b.ground();
    let mesh = b.mesh();
    let mut pts = Vec::new();
    for j in 0..16 {
        let y = 0.125 + j as f32 * 0.25;
        for (p, n) in [(Vec3::new(-0.1, y, 0.0), Vec3::Z), (Vec3::new(0.0, y, -0.1), Vec3::X)] {
            pts.push(Point { edge: 1.0, rain: 0.2, ..at(&mesh, p, n) });
        }
    }
    let (_, spots) = mark(&b.surface(&mesh), &pts);
    let plates: Vec<&Spot> = spots.iter().filter(|s| s.what == "street plate").collect();
    assert_eq!(plates.len(), 2, "{spots:?}");
    assert!(plates.iter().all(|p| (2.5..=3.0).contains(&p.at.y) && p.by == "city"));
    assert!(spots.iter().filter(|s| s.what == "bill").all(|s| (0.95..=2.05).contains(&s.at.y)));
    assert_eq!(mark(&b.surface(&mesh), &pts).1, spots, "the same input, the same spots");
}

#[test]
fn a_leaf_set_into_a_step_is_held_from_the_step() {
    let mut b = B::default();
    front(&mut b, 0.55);
    b.cube([-0.5, 0.0, -0.3], [0.5, 2.25, -0.25], Mat::Wood);
    b.cube([-0.8, 0.0, -0.25], [0.8, 0.17, 0.4], Mat::Masonry);
    let b = b.ground();
    let mesh = b.mesh();
    let pts = leaf_face(&mesh, -0.5, 0.5);
    let (touch, _) = mark(&b.surface(&mesh), &pts);
    let band = |y0: f32, y1: f32| pts.iter().zip(&touch).any(|(q, &v)| v > 0.5 && q.p.x.abs() > 0.2 && (y0..y1).contains(&q.p.y));
    assert!(band(1.13, 1.26), "the handle band from the step's top");
    assert!(!band(0.95, 1.05), "not from the leaf's buried foot");
}

#[test]
fn a_windows_reveals_are_not_a_passage() {
    let mut b = B::default();
    b.cube([-3.0, 0.0, -0.5], [-0.6, 3.0, 0.0], Mat::Plaster);
    b.cube([0.6, 0.0, -0.5], [3.0, 3.0, 0.0], Mat::Plaster);
    b.cube([-0.6, 0.0, -0.5], [0.6, 0.9, 0.0], Mat::Plaster);
    b.cube([-0.65, 0.85, -0.5], [0.65, 0.9, 0.15], Mat::Masonry);
    let b = b.ground();
    let mesh = b.mesh();
    let pts: Vec<Point> = [1.0, 1.3, 1.5].iter().map(|&y| at(&mesh, Vec3::new(-0.6, y, -0.25), Vec3::X)).collect();
    let (touch, _) = mark(&b.surface(&mesh), &pts);
    assert_eq!(touch, vec![0.0; 3], "a window's reveals aren't a passage");
}

#[test]
fn bills_go_on_plain_walls_not_beside_a_door() {
    let mut b = B::default();
    front(&mut b, 0.55);
    b.cube([-0.5, 0.0, -0.3], [0.5, 2.25, -0.25], Mat::Wood);
    b.cube([10.0, 0.0, -0.5], [20.0, 4.0, 0.0], Mat::Masonry);
    let b = b.ground();
    let mesh = b.mesh();
    let mut pts = Vec::new();
    for x in [-5.0f32, -4.0, -3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 11.0, 13.0, 15.0, 17.0, 19.0] {
        pts.push(Point { rain: 0.1, ..at(&mesh, Vec3::new(x, 1.5, 0.0), Vec3::Z) });
    }
    let (_, spots) = mark(&b.surface(&mesh), &pts);
    let bills: Vec<&Spot> = spots.iter().filter(|s| s.what == "bill").collect();
    assert!(!bills.is_empty() && bills.iter().all(|s| s.at.x > 12.9 && s.at.x < 17.1), "{bills:?}");
}
