use super::{prepare::PathtracerAccumulationTexture, Pathtracer};
use bevy_camera::Camera;
use bevy_ecs::{
    change_detection::DetectChanges,
    system::{Commands, ResMut},
    world::Ref,
};
use bevy_render::{sync_world::RenderEntity, MainWorld};
use bevy_transform::components::GlobalTransform;

// raea: `reset` is one-shot, as SolariLighting's is (realtime/extract.rs): cleared in the main world once it's
// extracted. It stayed set, so a cut under the path tracer (raea-render-raytrace `cut`) restarted its accumulation
// on every frame after it, and a reference never converged (docs/realism.feature LT1)
pub fn extract_pathtracer(mut main_world: ResMut<MainWorld>, mut commands: Commands) {
    let mut cameras_3d = main_world.query::<(
        RenderEntity,
        &Camera,
        Ref<GlobalTransform>,
        Option<&mut Pathtracer>,
    )>();
    for (entity, camera, global_transform, pathtracer) in cameras_3d.iter_mut(&mut main_world) {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        if let Some(mut pathtracer) = pathtracer
            && camera.is_active
        {
            let mut extracted = pathtracer.clone();
            extracted.reset |= global_transform.is_changed();
            entity_commands.insert(extracted);
            pathtracer.reset = false;
        } else {
            entity_commands.remove::<(Pathtracer, PathtracerAccumulationTexture)>();
        }
    }
}
