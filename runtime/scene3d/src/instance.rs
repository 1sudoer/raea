//! <Instances> drawn (sdk/render 3d/instances.tsx, the rows' `inst` column, runtime/scene's instances.rs): one child
//! a copy, each sharing the mesh and material handles, which Bevy's automatic batching draws as one instanced call per
//! mesh. A mesh row's own Mesh3d moves onto its copies at spawn; a model's scene is copied once it's settled (its
//! surfaces on), then the scene it was copied from is despawned. The parent keeps its transform, visibility and (a
//! mesh's) material, so sync_meshes still moves, hides and recolours the whole set. A mesh row with a colour a copy
//! (`inst_tint`, 13 numbers a copy) gives its copies one TintMaterial (raea-sys-material tint.rs) over its standard
//! material, each copy's MeshTag its number, and the buffer of colours follows the frame's list.

use super::*;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::render::storage::ShaderBuffer;
use raea_sys_copies::{copy_mesh, copy_scene, copy_tinted, markers, materials_of};
use raea_sys_material::tint::{self, copies, stride_of, Copies, TintMaterial};

/// A mesh or model entity drawn as instances: what sync_meshes moves in place of its Mesh3d.
#[derive(Component)]
pub struct Instanced;

/// A row's `inst`: its copies' placements.
pub fn placements(t: &Tup) -> Result<Option<Vec<Transform>>, String> {
    let Some(V::S(s)) = t.vals.get("inst") else { return Ok(None) };
    copies(&t.id, s, stride_of(t)).map(|l| Some(l.at))
}

/// An <Instances> whose placements may change (#85): the lists a render's row shows (`inst_all`; which one a frame
/// shows is keyed as `inst_k`) and the one shown, or a live row's `inst` as last applied; and where each of a placement's
/// copies sits in it (a model's meshes, in the model; a mesh's one copy, at the placement). Copies are its children,
/// placement by placement.
#[derive(Component)]
pub struct Placements {
    lists: Vec<String>,
    shown: usize,
    live: String,
    parts: Vec<Mat4>,
    stride: usize,
    /// a set with a colour a copy: the buffer its copies' TintMaterial reads, that material, and the standard material
    /// it was last made over
    tint: Option<Tinted>,
}

struct Tinted {
    colours: Handle<ShaderBuffer>,
    mat: Handle<TintMaterial>,
    base: AssetId<StandardMaterial>,
}

impl Placements {
    pub(crate) fn of(t: &Tup) -> Placements {
        let all = match t.vals.get("inst_all") { Some(V::S(s)) => s.split('|').map(str::to_string).collect(), _ => vec![] };
        let live = match t.vals.get("inst") { Some(V::S(s)) => s.clone(), _ => String::new() };
        Placements { lists: all, shown: 0, live, parts: vec![], stride: stride_of(t), tint: None }
    }
}

/// Each <Instances>' copies moved to the frame's placements when they're another list than the one shown: a render's
/// `inst_k`, or a live row's `inst`. A frame whose list has another count than the copies is an error.
#[allow(clippy::too_many_arguments)]
pub fn sync_instances(
    frame: Res<FrameRows>,
    mut q: Query<(Entity, &mut Placements, &Children, Option<&MeshMaterial3d<StandardMaterial>>)>,
    mut ts: Query<&mut Transform>,
    mut error: ResMut<Error>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut tints: ResMut<Assets<TintMaterial>>,
    stds: Res<Assets<StandardMaterial>>,
    mut changed: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let recoloured: Vec<AssetId<StandardMaterial>> = changed.read().filter_map(|ev| match ev { AssetEvent::Modified { id } => Some(*id), _ => None }).collect();
    for (e, mut pl, kids, std) in &mut q {
        // a tinted set's surface follows its row's standard material (sync_meshes recolours that one)
        if let (Some(tn), Some(std)) = (pl.tint.as_mut(), std) {
            if tn.base != std.0.id() || recoloured.contains(&tn.base) {
                tn.base = std.0.id();
                if let (Some(base), Some(mut m)) = (stds.get(&std.0).cloned(), tints.get_mut(&tn.mat)) {
                    m.base = base;
                }
            }
        }
        if pl.parts.is_empty() {
            continue;
        }
        let Some(v) = frame.vals(e) else { continue };
        // a live row's own list first (a render's `inst` is its first frame's on every frame, so it never differs)
        let s = match (v.get("inst"), v.get("inst_k")) {
            (Some(V::S(s)), _) if *s != pl.live => {
                pl.live = s.clone();
                s.clone()
            }
            (_, Some(k)) if !pl.lists.is_empty() => {
                let k = (k.num().round().max(0.0) as usize).min(pl.lists.len() - 1);
                if k == pl.shown {
                    continue;
                }
                pl.shown = k;
                pl.lists[k].clone()
            }
            _ => continue,
        };
        let Copies { at, tint } = match copies(&format!("{e}"), &s, pl.stride) {
            Ok(l) if l.at.len() * pl.parts.len() == kids.len() => l,
            Ok(l) => {
                error.0.get_or_insert(format!("Instances: a frame has {} placements where it began with {}; the count can't change", l.at.len(), kids.len() / pl.parts.len()));
                continue;
            }
            Err(err) => {
                error.0.get_or_insert(err);
                continue;
            }
        };
        if let Some(tn) = pl.tint.as_ref() {
            if let Some(mut b) = buffers.get_mut(&tn.colours) {
                *b = tint::colour_buffer(&tint);
            }
        }
        let places = at.iter().flat_map(|tr| pl.parts.iter().map(move |local| Transform::from_matrix(tr.to_matrix() * *local)));
        for (kid, want) in kids.iter().zip(places) {
            if let Ok(mut t) = ts.get_mut(kid) {
                *t = want;
            }
        }
    }
}

/// A mesh row with `inst`, after its spawn (shadows and layers on): its Mesh3d onto one child a copy, each sharing the
/// row's material, whatever its kind (a procedural or worn surface instances as a standard one does).
pub fn instance_mesh(world: &mut World, e: Entity, t: &Tup) -> Result<(), String> {
    let Some(V::S(s)) = t.vals.get("inst") else { return Ok(()) };
    let Copies { at, tint } = copies(&t.id, s, stride_of(t))?;
    if t.vals.contains_key("shader") {
        return Err(format!("mesh {}: instances take a standard surface (a shader over them isn't instanced yet)", t.id));
    }
    let mats = materials_of(world, e);
    if mats.is_empty() {
        return Err(format!("mesh {}: instances take a mesh material: standard, physical, basic, procedural, worn or grid (not toon)", t.id));
    }
    let std = world.get::<MeshMaterial3d<StandardMaterial>>(e).cloned();
    if !tint.is_empty() && std.is_none() {
        return Err(format!("mesh {}: a colour a copy tints a standard surface (a procedural or worn one is not tinted per copy yet)", t.id));
    }
    let Some(mesh) = world.entity_mut(e).take::<Mesh3d>() else { return Err(format!("mesh {}: no mesh to instance", t.id)) };
    let m = markers(world, e);
    let mut pl = Placements { parts: vec![Mat4::IDENTITY], ..Placements::of(t) };
    if let (Some(mat), false) = (std.as_ref(), tint.is_empty()) {
        // a colour a copy: one TintMaterial over the row's standard material
        let base = world.resource::<Assets<StandardMaterial>>().get(&mat.0).cloned().unwrap_or_default();
        let (colours, tm) = copy_tinted(world, e, mesh, base, &tint, at, &m);
        pl.tint = Some(Tinted { colours, mat: tm, base: mat.0.id() });
    } else {
        // the row's material component(s) cloned onto each copy: the same handle, so Bevy still batches the set
        copy_mesh(world, e, mesh, &mats, at, &m);
    }
    world.entity_mut(e).insert((Instanced, pl));
    Ok(())
}

/// A model row's placements, kept on its Model3d until its scene is copied (`instance_models`).
pub fn model_placements(t: &Tup) -> Result<Option<Vec<Transform>>, String> {
    let at = placements(t)?;
    if at.is_some() && ["anim", "mix", "graph", "morph_names"].iter().any(|k| t.vals.contains_key(*k)) {
        return Err(format!("model {}: instances can't play a clip or set morphs", t.id));
    }
    Ok(at)
}

/// Each instanced model whose scene is settled: its meshes (with their places in the model) copied to every placement,
/// then the scene despawned. Before then the model isn't settled, so await_models holds the first frame for it.
pub fn instance_models(world: &mut World) {
    let mut q = world.query::<(Entity, &Model3d)>();
    let todo: Vec<Entity> = q.iter(world).filter(|(_, m)| m.inst.is_some() && m.ready() && m.surfaces.is_empty() && m.toon.is_empty() && m.shaders.is_empty()).map(|(e, _)| e).collect();
    for e in todo {
        let at = world.get_mut::<Model3d>(e).and_then(|mut m| m.inst.take()).unwrap_or_default();
        let src = world.get::<Model3d>(e).map(|m| m.src.clone()).unwrap_or_default();
        let roots: Vec<Entity> = world.get::<Children>(e).map(|c| c.iter().collect()).unwrap_or_default();
        let parts = match copy_scene(world, e, &roots, &at) {
            Ok(p) => p,
            Err(f) => {
                world.resource_mut::<Error>().0.get_or_insert(format!("model {src}: {f}"));
                continue;
            }
        };
        for r in roots {
            world.entity_mut(r).despawn();
        }
        if let Some(mut pl) = world.get_mut::<Placements>(e) {
            pl.parts = parts;
        }
        world.entity_mut(e).insert(Instanced);
    }
}

/// A ready model with `shadow={false}` or `receiveShadow={false}`: each of its meshes given the marker the model has
/// (Bevy reads them per mesh entity, not from a parent), whenever one lacks it.
#[allow(clippy::type_complexity)]
pub fn model_shadow_markers(
    mut commands: Commands,
    models: Query<(Entity, Has<NotShadowCaster>, Has<NotShadowReceiver>), (With<ModelReady>, Or<(With<NotShadowCaster>, With<NotShadowReceiver>)>)>,
    kids: Query<&Children>,
    meshes: Query<(Has<NotShadowCaster>, Has<NotShadowReceiver>), With<Mesh3d>>,
) {
    for (e, no_cast, no_receive) in &models {
        for d in descendants(e, &kids) {
            let Ok((c, r)) = meshes.get(d) else { continue };
            if no_cast && !c {
                commands.entity(d).insert(NotShadowCaster);
            }
            if no_receive && !r {
                commands.entity(d).insert(NotShadowReceiver);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mesh row with `inst` (10 numbers a copy: position, rotation as x y z w, scale), two copies.
    fn row() -> Tup {
        let inst = "0 0 0 0 0 0 1 1 1 1  2 0 0 0 0 0 1 1 1 1";
        Tup { kind: "mesh".into(), id: "m".into(), vals: [("inst".to_string(), V::S(inst.into()))].into_iter().collect() }
    }

    fn world() -> World {
        let mut world = World::new();
        let reg = AppTypeRegistry::default();
        // as MaterialPlugin<M> does for every material raea draws with (the tint extension stands in for a procedural
        // or worn one: any MeshMaterial3d<M> that isn't the standard surface)
        reg.write().register::<MeshMaterial3d<TintMaterial>>();
        world.insert_resource(reg);
        world
    }

    #[test]
    fn copies_share_any_mesh_material() {
        let mut world = world();
        let mat = MeshMaterial3d::<TintMaterial>(Handle::default());
        let e = world.spawn((Mesh3d(Handle::default()), mat.clone(), Transform::default())).id();
        instance_mesh(&mut world, e, &row()).unwrap();
        let kids: Vec<Entity> = world.get::<Children>(e).expect("copies").iter().collect();
        assert_eq!(kids.len(), 2);
        for k in kids {
            assert_eq!(world.get::<MeshMaterial3d<TintMaterial>>(k).map(|m| m.0.id()), Some(mat.0.id()));
            assert!(world.get::<Mesh3d>(k).is_some());
        }
        // the mesh moved onto the copies; the parent keeps its material (sync_meshes recolours the set through it)
        assert!(world.get::<Mesh3d>(e).is_none());
        assert!(world.get::<MeshMaterial3d<TintMaterial>>(e).is_some());
    }

    #[test]
    fn a_mesh_with_no_material_is_refused() {
        let mut world = world();
        let e = world.spawn((Mesh3d(Handle::default()), Transform::default())).id();
        let err = instance_mesh(&mut world, e, &row()).unwrap_err();
        assert!(err.contains("instances take a mesh material"), "{err}");
    }
}
