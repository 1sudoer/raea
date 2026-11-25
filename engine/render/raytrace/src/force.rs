//! RAEA_LIGHT (docs/realism.feature LT1): one light path for every 3D camera, whatever the composition asks, so a
//! scene can be measured on each path without editing it. `raster` takes SolariLighting and Pathtracer off the
//! cameras; `solari` puts SolariLighting (Bevy's defaults) on each camera in place of a Pathtracer; `pathtracer`
//! puts Pathtracer on each in place of SolariLighting, for the reference frame (with RAEA_LIGHT_SPP, runtime/export
//! bevy_frame.rs draws each frame that many times, so it converges, and starts each frame's accumulation afresh,
//! `restart_pathtracer`, so a thing that moves under a held camera isn't smeared across frames). Unset, nothing
//! changes.

use bevy::prelude::*;
use bevy::solari::pathtracer::Pathtracer;
use bevy::solari::prelude::SolariLighting;

/// The light path RAEA_LIGHT asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Path {
    Raster,
    Solari,
    Pathtracer,
}

/// RAEA_LIGHT's path, if it names one (anything else is said once and ignored).
pub fn forced() -> Option<Path> {
    static ON: std::sync::OnceLock<Option<Path>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| match std::env::var("RAEA_LIGHT").as_deref() {
        Ok("raster") => Some(Path::Raster),
        Ok("solari") => Some(Path::Solari),
        Ok("pathtracer") => Some(Path::Pathtracer),
        Ok("") | Err(_) => None,
        Ok(other) => {
            eprintln!("raea: RAEA_LIGHT={other} isn't a light path (raster, solari or pathtracer); left as the composition has it");
            None
        }
    })
}

/// Whether the forced path needs Solari's plugins, whatever the program asks.
pub fn needs_solari() -> bool {
    matches!(forced(), Some(Path::Solari | Path::Pathtracer))
}

pub fn add(app: &mut App) {
    if forced().is_some() {
        // before Solari's own camera setup (solari.rs `cameras`, in PostUpdate), so a forced camera is set up the frame
        // it's forced
        app.add_systems(PreUpdate, cameras);
    }
}

/// Every 3D camera on the forced path.
fn cameras(mut cmd: Commands, q: Query<(Entity, Has<SolariLighting>, Has<Pathtracer>), With<Camera3d>>) {
    let Some(path) = forced() else { return };
    for (e, solari, traced) in &q {
        match path {
            Path::Raster if solari || traced => {
                cmd.entity(e).remove::<(SolariLighting, Pathtracer)>();
            }
            Path::Solari if !solari || traced => {
                cmd.entity(e).remove::<Pathtracer>().insert_if_new(SolariLighting::default());
            }
            Path::Pathtracer if !traced || solari => {
                cmd.entity(e).remove::<SolariLighting>().insert_if_new(Pathtracer::default());
            }
            _ => {}
        }
    }
}
