//! A standard material's map that names another render image frame by frame (docs/r3f.feature R3F23c): an app that
//! ping-pongs between two render targets (three's GPUComputationRenderer, a feedback or trail effect) shows the one it
//! drew this frame. raea-runtime-scene's parts.rs keys a mesh row's `map` when it names render images that change over
//! the frames (`maps`, one a line, and `map_k`, keyed by step); the image each names is the one its camera draws into
//! (raea-sys-frame's RenderImages). A map that names a picture keeps its first frame's.

use bevy::prelude::*;
use raea_sys_frame::{FrameRows, RenderImages};

/// Each mesh whose map is keyed: its material's base colour texture, the render image its row names this frame.
pub fn sync_render_maps(frame: Res<FrameRows>, renders: Option<Res<RenderImages>>, q: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let Some(renders) = renders.filter(|_| frame.may_have("map_k")) else { return };
    for (e, m) in &q {
        let Some(v) = frame.vals(e).filter(|v| v.contains_key("map_k")) else { continue };
        let Some(src) = raea_sys_frame::keyed(|c| v.get(c).cloned(), "map") else { continue };
        let Some(img) = RenderImages::name(&src).and_then(|n| renders.0.get(n)).map(|i| i.0.clone()) else { continue };
        // looked at first: get_mut marks the material changed whether or not it is
        let shown = match mats.get(&m.0) {
            Some(s) => s.base_color_texture.as_ref() == Some(&img),
            None => true,
        };
        if shown {
            continue;
        }
        if let Some(mut s) = mats.get_mut(&m.0) {
            s.base_color_texture = Some(img);
        }
    }
}
