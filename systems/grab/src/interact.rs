//! Which thing a body takes: musthafa45/Ik-Object-Holding's Interactor (MIT). There, the first collider inside a
//! radius of the player is taken if a line from the player to it is clear. Here it is the nearest thing inside the
//! radius that the eyes have a clear line to, past the scene's solids, so a thing behind a wall is not chosen and
//! the choice doesn't depend on the order things were made in.

use glam::Vec3;

use raea_sys_body::shape::Thing;

/// The thing to take: the nearest of `things` within `radius` of the body at `from` that `eye` sees past `solids`.
pub fn pick(things: &[Thing], from: Vec3, eye: Vec3, radius: f32, solids: &[Thing]) -> Option<usize> {
    let seen = |thing: &Thing| {
        let to = thing.at - eye;
        let far = to.length();
        let Some(dir) = to.try_normalize() else { return true };
        // clear when no solid's surface comes before the thing's own
        let met = |solid: &Thing, within: f32| solid.cast(eye, dir, within).map(|h| h.at.distance(eye));
        let reach = met(thing, far).unwrap_or(far);
        solids.iter().all(|s| met(s, reach).is_none_or(|d| d >= reach - 1e-3))
    };
    (0..things.len())
        .filter(|&i| (things[i].at - from).length() <= radius + things[i].solid.bound() && seen(&things[i]))
        .min_by(|&a, &b| (things[a].at - from).length().total_cmp(&(things[b].at - from).length()))
}
