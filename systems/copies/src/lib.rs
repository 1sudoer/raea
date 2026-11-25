//! An <Instances>' copies (runtime/scene3d instance.rs, which knows the rows and calls these): a mesh's or a model's
//! parts spawned under a parent at each placement. Each copy shares its source's mesh and material handles, whatever
//! the material's kind (any MeshMaterial3d<M>, found by its type path, and a shader cover's link), so Bevy's automatic
//! batching draws a set as one instanced call per mesh. A copy takes its parent's shadow markers and render layers.

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::component::ComponentId;
use bevy::ecs::entity::EntityCloner;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use raea_sys_material::tint::{self, Tint, TintMaterial};

#[cfg(test)]
mod tests;

/// What a copy takes from its parent: shadows not cast, not received, render layers.
pub type Markers = (bool, bool, Option<RenderLayers>);

pub fn markers(world: &World, e: Entity) -> Markers {
    (world.get::<NotShadowCaster>(e).is_some(), world.get::<NotShadowReceiver>(e).is_some(), world.get::<RenderLayers>(e).cloned())
}

/// A copy with its mesh, place and the parent's markers, its material still to put on.
pub fn spawn_bare(world: &mut World, parent: Entity, mesh: Mesh3d, at: Transform, (no_cast, no_receive, layers): &Markers) -> Entity {
    let mut c = world.spawn((mesh, at, ChildOf(parent)));
    if *no_cast {
        c.insert(NotShadowCaster);
    }
    if *no_receive {
        c.insert(NotShadowReceiver);
    }
    if let Some(l) = layers {
        c.insert(l.clone());
    }
    c.id()
}

/// Every MeshMaterial3d<M> on an entity, whatever M: the standard surface, or one raea extends (procedural, worn, grid,
/// a shadow catcher). Found by type path, since each MaterialPlugin registers its MeshMaterial3d's type.
pub fn materials_of(world: &World, e: Entity) -> Vec<ComponentId> {
    let (Some(reg), Ok(er)) = (world.get_resource::<AppTypeRegistry>(), world.get_entity(e)) else { return vec![] };
    let reg = reg.read();
    let material = |id: &ComponentId| {
        let ty = world.components().get_info(*id).and_then(|i| i.type_id());
        ty.and_then(|ty| reg.get_type_info(ty)).is_some_and(|ti| ti.type_path().contains("::MeshMaterial3d<"))
    };
    er.archetype().components().iter().filter(|id| material(id)).copied().collect()
}

/// A cloner of just these components (a material's, as handles), with none they'd require.
fn cloner(world: &mut World, ids: &[ComponentId]) -> EntityCloner {
    let mut b = EntityCloner::build_opt_in(world);
    b.without_required_components(|b| {
        b.allow_by_ids(ids.iter().copied());
    });
    b.finish()
}

/// `mesh` copied under `e` at each of `at`, each copy with `e`'s material components `mats` (the same handles).
pub fn copy_mesh(world: &mut World, e: Entity, mesh: Mesh3d, mats: &[ComponentId], at: Vec<Transform>, m: &Markers) {
    let mut c = cloner(world, mats);
    for tr in at {
        let k = spawn_bare(world, e, mesh.clone(), tr, m);
        c.clone_entity(world, e, k);
    }
}

/// `mesh` copied under `e` at each of `at`, each with a colour of its own (`tint`, a copy's colours as raea-sys-material
/// reads them): one TintMaterial over `base`, each copy's number in its MeshTag. Returns the colours' buffer and the
/// material, for the set to update when its colours or its standard material change.
pub fn copy_tinted(world: &mut World, e: Entity, mesh: Mesh3d, base: StandardMaterial, tint: &[[f32; 3]], at: Vec<Transform>, m: &Markers) -> (Handle<ShaderBuffer>, Handle<TintMaterial>) {
    let colours = world.resource_mut::<Assets<ShaderBuffer>>().add(tint::colour_buffer(tint));
    let tm = world.resource_mut::<Assets<TintMaterial>>().add(TintMaterial { base, extension: Tint { colours: colours.clone() } });
    for (i, tr) in at.into_iter().enumerate() {
        let k = spawn_bare(world, e, mesh.clone(), tr, m);
        world.entity_mut(k).insert((MeshMaterial3d(tm.clone()), MeshTag::new(tint::copy_tag(i, 0))));
    }
    (colours, tm)
}

/// A model's scene (`roots`, its children) copied under `e` to every placement in `at`: each mesh with its place in
/// the model (its transforms multiplied up to the model) and its material's components, its standard (glTF) one or
/// what a cover put over it (a shader's surface, with the link that keeps it following the standard one the rows
/// animate; a procedural or worn one), cloned onto each copy as handles. Copies go placement by placement, each one's
/// parts in the same order (a set's placements move them in that order). Returns each part's place in the model.
pub fn copy_scene(world: &mut World, e: Entity, roots: &[Entity], at: &[Transform]) -> Result<Vec<Mat4>, String> {
    let shaded = world.components().component_id::<raea_sys_shader::Shaded>();
    let mut parts: Vec<(Mesh3d, Entity, Vec<ComponentId>, Mat4)> = vec![];
    let mut stack: Vec<(Entity, Mat4)> = roots.iter().map(|r| (*r, Mat4::IDENTITY)).collect();
    while let Some((d, above)) = stack.pop() {
        let here = above * world.get::<Transform>(d).map(Transform::to_matrix).unwrap_or(Mat4::IDENTITY);
        if let Some(mesh) = world.get::<Mesh3d>(d) {
            if world.get::<SkinnedMesh>(d).is_some() {
                return Err("a skinned mesh can't be instanced; use a <Model>".into());
            }
            let mut mats = materials_of(world, d);
            if mats.is_empty() {
                return Err("instances take its mesh materials: standard, shaded, procedural or worn (a toon or look surface isn't instanced yet)".into());
            }
            if let Some(id) = shaded.filter(|_| world.get::<raea_sys_shader::Shaded>(d).is_some()) {
                mats.push(id);
            }
            parts.push((mesh.clone(), d, mats, here));
        }
        if let Some(kids) = world.get::<Children>(d) {
            stack.extend(kids.iter().map(|k| (k, here)));
        }
    }
    let m = markers(world, e);
    let mut cloners: Vec<EntityCloner> = parts.iter().map(|(_, _, mats, _)| cloner(world, mats)).collect();
    for tr in at {
        let place = tr.to_matrix();
        for (k, (mesh, from, _, local)) in parts.iter().enumerate() {
            let c = spawn_bare(world, e, mesh.clone(), Transform::from_matrix(place * *local), &m);
            cloners[k].clone_entity(world, *from, c);
        }
    }
    Ok(parts.iter().map(|p| p.3).collect())
}
