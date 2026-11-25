//! Synthetic fronts checked by number: a wall facing south and one facing north, a wall facing a storm and one in
//! its lee, a gutter against a kerb, and the sun's height on the solstices.

use parry3d::glamx::Vec3;
use parry3d::shape::TriMesh;

use crate::{DECEMBER, Driven, Exposer, Frame, YEAR, sun_path};

const BERLIN: Frame = Frame { up: Vec3::Y, north: Vec3::NEG_Z, lat: 52.5 };

#[derive(Default)]
struct B {
    pos: Vec<Vec3>,
    tri: Vec<[u32; 3]>,
}

impl B {
    fn cube(&mut self, a: [f32; 3], b: [f32; 3]) {
        let i = self.pos.len() as u32;
        for k in 0..8 {
            self.pos.push(Vec3::new(if k & 1 == 0 { a[0] } else { b[0] }, if k & 2 == 0 { a[1] } else { b[1] }, if k & 4 == 0 { a[2] } else { b[2] }));
        }
        for q in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
            self.tri.push([i + q[0], i + q[1], i + q[2]]);
            self.tri.push([i + q[0], i + q[2], i + q[3]]);
        }
    }
    fn mesh(self) -> TriMesh {
        TriMesh::new(self.pos, self.tri).unwrap()
    }
}

/// Snow on a storm from the east-north-east, falling 60 degrees off the vertical.
fn storm() -> Driven {
    let east = BERLIN.north.cross(BERLIN.up).normalize();
    let from = (east * 22.5f32.to_radians().cos() + BERLIN.north * 22.5f32.to_radians().sin()).normalize();
    let (tilt, spread) = (60f32.to_radians(), 25f32.to_radians());
    Driven { dir: (-from * tilt.sin() - BERLIN.up * tilt.cos()).normalize(), spread }
}

#[test]
fn the_sun_stands_14_degrees_at_the_december_solstice_and_61_in_june() {
    let top = |day: f32| sun_path(&BERLIN, &[day], 0.25).iter().map(|s| s.y.asin().to_degrees()).fold(0.0, f32::max);
    assert!((top(355.0) - 14.1).abs() < 0.3, "{}", top(355.0));
    assert!((top(172.0) - 60.9).abs() < 0.3, "{}", top(172.0));
    assert!(sun_path(&BERLIN, &DECEMBER, 1.0).len() < sun_path(&BERLIN, &YEAR, 2.0).len());
}

#[test]
fn a_south_wall_sees_the_december_sun_and_a_north_one_does_not() {
    let mut b = B::default();
    b.cube([-5.0, 0.0, -1.0], [5.0, 10.0, 0.0]);
    let mesh = b.mesh();
    let e = Exposer::new(&mesh, &BERLIN, storm(), 32);
    let south = e.at(1, Vec3::new(0.0, 5.0, 0.0), Vec3::Z);
    let north = e.at(2, Vec3::new(0.0, 5.0, -1.0), Vec3::NEG_Z);
    assert!(south.december > 0.3 && north.december == 0.0, "{south:?} {north:?}");
    assert!(north.sun > 0.0, "a north wall sees the summer's early and late sun: {north:?}");
    assert!(south.sun > 3.0 * north.sun);
}

#[test]
fn a_wall_facing_the_storm_takes_the_drift_and_its_lee_none() {
    let mut b = B::default();
    b.cube([-0.5, 0.0, -5.0], [0.0, 10.0, 5.0]);
    let mesh = b.mesh();
    let e = Exposer::new(&mesh, &BERLIN, storm(), 64);
    let east = e.at(1, Vec3::new(0.0, 5.0, 0.0), Vec3::X);
    let west = e.at(2, Vec3::new(-0.5, 5.0, 0.0), Vec3::NEG_X);
    let roof = e.at(3, Vec3::new(-0.25, 10.0, 0.0), Vec3::Y);
    assert!(east.drift > 0.8 && west.drift < 0.05, "{east:?} {west:?}");
    assert!((roof.drift - 1.0).abs() < 0.1, "a level roof is the measure: {roof:?}");
}

#[test]
fn litter_gathers_in_the_gutter_against_a_kerb_and_in_a_walls_lee() {
    let mut b = B::default();
    b.cube([-20.0, -0.15, -20.0], [20.0, 0.0, 20.0]);
    b.cube([-20.0, 0.0, 0.0], [20.0, 0.15, 0.3]);
    b.cube([-10.0, 0.0, 10.0], [-9.7, 3.0, 14.0]);
    let mesh = b.mesh();
    let e = Exposer::new(&mesh, &BERLIN, storm(), 16);
    let gutter = e.at(1, Vec3::new(0.0, 0.0, -0.05), Vec3::Y);
    let road = e.at(2, Vec3::new(0.0, 0.0, -6.0), Vec3::Y);
    let lee = e.at(3, Vec3::new(-10.6, 0.0, 12.0), Vec3::Y);
    let wall = e.at(4, Vec3::new(5.0, 0.07, 0.0), Vec3::NEG_Z);
    assert!(gutter.litter > 0.1 && road.litter == 0.0, "{gutter:?} {road:?}");
    assert!(lee.litter > road.litter, "the lee of a wall against the wind: {lee:?}");
    assert_eq!(wall.litter, 0.0, "nothing settles on a face that doesn't look up");
    assert_eq!(e.at(1, Vec3::new(0.0, 0.0, -0.05), Vec3::Y), gutter, "the same point, the same exposure");
}
