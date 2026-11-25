//! Procedural arms and hands (docs/hands.feature): a body reaches for a thing, takes it with one hand or two, carries
//! it, and puts it down or throws it, with no capture of that action. Plain maths on glam and raea-sys-body (the
//! rig, IK, limits), no engine: `perform` is a function of time, so a render and live play ask it the same question
//! and get the same frame.
//!
//! - `hand`: hold points found on the thing, the wrist that lays a palm on one, fingers curled onto the surface.
//! - `grip`: how a hand lies on its hold, decided at the grab and kept; a thing in one hand turning with the forearm.
//! - `trunk`: the body bent and squatted for a low thing with its feet planted, the head turned to the thing, hands
//!   and head kept out of the scene's solids, and where to stand to take a thing.
//! - `interact`: which thing a body takes: the nearest in reach that its eyes see.
//! - `take`: reach, grab, lift, carry, put down or throw, as one timeline.
//! - `json`: takes, fitted poses and hand shapes asked for as JSON: the engine's native calls character.take,
//!   character.fit and character.hand (raea/character's take, fitPose and handShape).

pub mod grip;
pub mod hand;
pub mod interact;
pub mod json;
pub mod take;
pub mod trunk;

pub use hand::{Grip, Hold, HoldOptions, Touch, curl, find_holds, wrist_for};
pub use interact::pick;
pub use json::{TAKE_ANSWER, TAKE_ARGS, take_json};
pub use raea_sys_body::json::{FIT_ANSWER, FIT_ARGS, HAND_ANSWER, HAND_ARGS, SOMA_ANSWER, SOMA_ARGS, TURNS_ANSWER, TURNS_ARGS, TWISTS_ARGS, fit_json, hand_json, soma_json, turns_json, twists_json};
pub use take::{Frame, Key, Phase, Release, Take, perform};
pub use trunk::{clear, give_for, hand_points, look, own_solids, room, spread_for, stand_for, stoop, torso, LOOK_MOST};
