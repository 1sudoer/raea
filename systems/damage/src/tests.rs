//! A plastered front with a window opening 1.2 m wide and 1.6 m high, recessed 0.25 m to its glass, and the wall's
//! end at x = 6 (a building's outer corner). Points are sampled every 5 cm on the front as an atlas's texels would be;
//! the ground is the plane y = 0.

use parry3d::glamx::Vec3;
use parry3d::query::{Ray, RayCast};
use parry3d::shape::TriMesh;

use raea_sys_touch::{Mat, Point, Surface};

use crate::{CRACKED, REACH, corners, cracked, cracks, debris, loss};

#[derive(Default)]
struct B {
    pos: Vec<Vec3>,
    tri: Vec<[u32; 3]>,
    nrm: Vec<Vec3>,
    mat: Vec<Mat>,
    walk: Vec<bool>,
}

impl B {
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
    fn mesh(&self) -> TriMesh {
        TriMesh::new(self.pos.clone(), self.tri.clone()).unwrap()
    }
    fn surface<'a>(&'a self, mesh: &'a TriMesh) -> Surface<'a> {
        Surface { pos: &self.pos, tri: &self.tri, nrm: &self.nrm, mat: &self.mat, walk: &self.walk, mesh, up: Vec3::Y }
    }
}

const WIN: [f32; 4] = [-0.6, 0.6, 3.0, 4.6];

/// The front at z = 0 from x = -6 to 6 and 8 m up, of `m`, with the window cut through it, its glass 0.25 m in.
fn front(m: Mat) -> B {
    let mut b = B::default();
    let [x0, x1, y0, y1] = WIN;
    b.cube([-6.0, 0.0, -0.5], [x0, 8.0, 0.0], m);
    b.cube([x1, 0.0, -0.5], [6.0, 8.0, 0.0], m);
    b.cube([x0, 0.0, -0.5], [x1, y0, 0.0], m);
    b.cube([x0, y1, -0.5], [x1, 8.0, 0.0], m);
    b.cube([x0, y0, -0.5], [x1, y1, -0.25], Mat::Other);
    let v = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    b.quad([v(-20.0, -20.0), v(20.0, -20.0), v(20.0, 20.0), v(-20.0, 20.0)], Vec3::Y, Mat::Other, true);
    b
}

/// The front's texels every 5 cm, outside the opening; `flow` and `rain` as the other maps would say.
fn texels(mesh: &TriMesh, flow: f32, rain: f32) -> Vec<Point> {
    let mut out = Vec::new();
    for i in 0..239 {
        for j in 0..159 {
            let p = Vec3::new(-5.95 + i as f32 * 0.05, 0.025 + j as f32 * 0.05, 0.0);
            let [x0, x1, y0, y1] = WIN;
            if p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1 {
                continue;
            }
            let h = mesh.cast_local_ray_and_get_normal(&Ray::new(p + Vec3::Z * 0.02, -Vec3::Z), 0.05, false).expect("the front");
            out.push(Point { p, n: Vec3::Z, tri: h.subshape, ground: p.y, rain, flow, edge: 0.0 });
        }
    }
    out
}

// the four corners of the opening, each leaving away from it; the wall's end at x = 6 isn't one
#[test]
fn an_openings_four_corners_and_no_others() {
    let b = front(Mat::Plaster);
    let mesh = b.mesh();
    let s = b.surface(&mesh);
    let cs = corners(&s, &texels(&mesh, 0.0, 1.0));
    assert_eq!(cs.len(), 4, "{cs:?}");
    let [x0, x1, y0, y1] = WIN;
    for (cx, cy) in [(x0, y0), (x0, y1), (x1, y0), (x1, y1)] {
        let c = cs.iter().find(|c| c.at.distance(Vec3::new(cx, cy, 0.0)) < 0.2).unwrap_or_else(|| panic!("no corner at {cx}, {cy}: {cs:?}"));
        let out = Vec3::new(cx.signum(), if cy == y1 { 1.0 } else { -1.0 }, 0.0).normalize();
        assert!(c.away.dot(out) > 0.95, "corner {cx}, {cy} leaves along {:?}", c.away);
        assert!(c.n.dot(Vec3::Z) > 0.99);
    }
}

// each crack runs on the wall away from its corner, steeper than 45 degrees; texels on it are 0 from it, far ones
// REACH; the same front gives the same cracks
#[test]
fn cracks_leave_the_corners_along_the_wall() {
    let b = front(Mat::Plaster);
    let mesh = b.mesh();
    let s = b.surface(&mesh);
    let pts = texels(&mesh, 0.0, 1.0);
    let (d, cs) = cracks(&s, &pts);
    assert_eq!(d.len(), pts.len());
    let [x0, x1, y0, y1] = WIN;
    for c in &cs {
        let path = crate::crack(&s, c);
        assert!(path.len() >= 8, "a crack of {} steps from {:?}", path.len(), c.at);
        let end = *path.last().unwrap();
        let run = end - c.at;
        assert!(run.y.abs() > run.x.abs(), "steeper than 45: {run:?}");
        assert_eq!(run.y.signum(), c.away.y.signum(), "up from the head's corners, down from the sill's");
        assert_eq!(run.x.signum(), c.away.x.signum(), "away from the opening");
        for p in &path {
            assert!(!(p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1), "into the opening at {p:?}");
            assert!(p.z.abs() < 1e-4);
        }
    }
    let near = pts.iter().zip(&d).filter(|(_, d)| **d < 0.03).count();
    assert!(near > 20, "{near} texels on the cracks");
    let far = pts.iter().zip(&d).find(|(q, _)| q.p.distance(Vec3::new(-4.0, 1.0, 0.0)) < 0.05).unwrap();
    assert_eq!(*far.1, REACH);
    assert_eq!(cracks(&s, &pts).0, d);
}

// the loss potential: the damp foot, a soaked face the rain doesn't wash, a knocked arris low down, a crack; nothing
// on a dry, rain-washed face up the wall
#[test]
fn what_loosens_a_coat() {
    let q = |ground: f32, rain: f32, flow: f32, edge: f32| Point { p: Vec3::ZERO, n: Vec3::Z, tri: 0, ground, rain, flow, edge };
    assert!((loss(&q(0.2, 1.0, 0.0, 0.0), REACH) - 1.0).abs() < 1e-6, "the foot");
    assert_eq!(loss(&q(3.0, 1.0, 0.0, 0.0), REACH), 0.0, "a plain face up the wall");
    assert!((loss(&q(3.0, 0.1, 0.95, 0.0), REACH) - 0.8).abs() < 1e-6, "soaked under a sill");
    assert_eq!(loss(&q(3.0, 0.95, 0.95, 0.0), REACH), 0.0, "run-off the rain washes");
    assert!((loss(&q(1.0, 1.0, 0.0, 1.0), REACH) - 0.7).abs() < 0.2, "an arris low down");
    assert_eq!(loss(&q(3.0, 1.0, 0.0, 1.0), REACH), 0.0, "an arris out of reach");
    assert!((loss(&q(3.0, 1.0, 0.0, 0.0), 0.0) - 0.6).abs() < 1e-6, "on a crack");
}

// debris on the walk in front of the plaster's foot, 1.5 m apart, each with the potential above it; none in front of
// stone, whose face doesn't come off in sheets
#[test]
fn debris_under_the_plasters_foot() {
    let b = front(Mat::Plaster);
    let mesh = b.mesh();
    let s = b.surface(&mesh);
    let pts = texels(&mesh, 0.0, 1.0);
    let (d, _) = cracks(&s, &pts);
    let spots = debris(&s, &pts, &d);
    assert!(spots.len() >= 5, "{} heaps along 12 m", spots.len());
    for (sp, l) in &spots {
        assert!(sp.at.y.abs() < 1e-4 && (sp.at.z - 0.2).abs() < 1e-4, "on the walk in front: {:?}", sp.at);
        assert!(*l >= 0.5 && *l <= 1.0);
        assert_eq!(sp.what, "debris");
    }
    for (i, a) in spots.iter().enumerate() {
        assert!(spots[i + 1..].iter().all(|b| a.0.at.distance(b.0.at) >= 1.5));
    }
    // a wall the rain doesn't reach is indoors; a plinth's top under the foot is no walk
    assert!(debris(&s, &texels(&mesh, 0.0, 0.0), &d).is_empty(), "indoors");
    let mut b = front(Mat::Plaster);
    b.cube([-6.0, 0.0, 0.0], [6.0, 0.6, 0.3], Mat::Masonry);
    let mesh = b.mesh();
    let s = b.surface(&mesh);
    let pts: Vec<Point> = texels(&mesh, 0.0, 1.0).into_iter().filter(|q| q.p.y > 0.6).map(|q| Point { ground: q.p.y, ..q }).collect();
    let (d, _) = cracks(&s, &pts);
    assert!(debris(&s, &pts, &d).is_empty(), "on a plinth 0.3 m out, a heap 0.2 m out lands on its top");
    let stone = front(Mat::Masonry);
    let mesh = stone.mesh();
    let s = stone.surface(&mesh);
    let pts = texels(&mesh, 0.0, 1.0);
    assert!(debris(&s, &pts, &vec![REACH; pts.len()]).is_empty());
}

// about CRACKED in a hundred corners crack, the same ones every time: a front of 20 x 20 windows' corners
#[test]
fn about_half_the_corners_crack() {
    let at: Vec<Vec3> = (0..400).map(|i| Vec3::new((i % 20) as f32 * 2.5 - 0.6, (i / 20) as f32 * 3.4 + 1.5, 0.0)).collect();
    let n = at.iter().filter(|&&p| cracked(p)).count();
    assert!((n as i64 - CRACKED as i64 * 4).abs() < 60, "{n} of 400 crack");
    assert!(at.iter().all(|&p| cracked(p) == cracked(p)));
}
