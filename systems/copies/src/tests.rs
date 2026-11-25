//! A model's parts copied with their surfaces, a shaded one keeping its link, and a part with no material refused.

use super::*;

fn world() -> World {
    let mut world = World::new();
    let reg = AppTypeRegistry::default();
    // as MaterialPlugin<M> does for every material raea draws with (the tint extension stands in for a shader's or a
    // procedural surface: any MeshMaterial3d<M> that isn't the standard one)
    reg.write().register::<MeshMaterial3d<TintMaterial>>();
    reg.write().register::<MeshMaterial3d<StandardMaterial>>();
    world.insert_resource(reg);
    world
}

/// A model whose cover put a shader over a part (a tree's leaves): each placement gets every part, the shaded one
/// with its surface and its link to the standard material the rows animate, placement by placement.
#[test]
fn a_models_shaded_parts_are_copied_with_their_surface() {
    let mut world = world();
    let e = world.spawn(Transform::default()).id();
    let root = world.spawn((Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(e))).id();
    let std = MeshMaterial3d::<StandardMaterial>(Handle::default());
    let shaded = MeshMaterial3d::<TintMaterial>(Handle::default());
    world.spawn((Mesh3d(Handle::default()), std.clone(), Transform::default(), ChildOf(root)));
    world.spawn((Mesh3d(Handle::default()), shaded.clone(), raea_sys_shader::Shaded { std: Handle::default() }, Transform::from_xyz(0.0, 2.0, 0.0), ChildOf(root)));
    let at = [Transform::from_xyz(10.0, 0.0, 0.0), Transform::from_xyz(20.0, 0.0, 0.0)];
    let parts = copy_scene(&mut world, e, &[root], &at).unwrap();
    assert_eq!(parts.len(), 2);
    let kids: Vec<Entity> = world.get::<Children>(e).unwrap().iter().filter(|k| *k != root).collect();
    assert_eq!(kids.len(), 4, "two parts at two placements");
    for (i, k) in kids.iter().enumerate() {
        let y = world.get::<Transform>(*k).unwrap().translation;
        assert_eq!(y.x, if i < 2 { 10.0 } else { 20.0 }, "placement by placement");
        let is_shaded = world.get::<MeshMaterial3d<TintMaterial>>(*k).is_some();
        assert_eq!(is_shaded, world.get::<raea_sys_shader::Shaded>(*k).is_some(), "a shaded copy keeps its link");
        assert_ne!(is_shaded, world.get::<MeshMaterial3d<StandardMaterial>>(*k).is_some(), "one surface a copy");
        assert_eq!(y.y, if is_shaded { 3.0 } else { 1.0 }, "its place in the model");
    }
}

#[test]
fn a_models_part_with_no_material_is_refused() {
    let mut world = world();
    let e = world.spawn(Transform::default()).id();
    let root = world.spawn((Transform::default(), ChildOf(e))).id();
    world.spawn((Mesh3d(Handle::default()), Transform::default(), ChildOf(root)));
    let err = copy_scene(&mut world, e, &[root], &[Transform::default()]).unwrap_err();
    assert!(err.contains("instances take its mesh materials"), "{err}");
}
