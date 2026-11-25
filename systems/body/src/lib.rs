//! The character's body in Rust (docs/hands.feature): what sdk/body is in TypeScript, for the engine. Plain maths on
//! glam, no engine.
//!
//! - `rig`: the game_engine skeleton, poses, forward kinematics, a hand's frame, and a stand-in body.
//! - `arm`: two-bone IK with a pole, a reach that leans the trunk, the wrist's twist shared with the forearm, and
//!   SOMA-X's twist joints.
//! - `limits`: each joint's range as anatomical angles (sdk/body's), a turn brought inside it, a pose's violations.
//! - `fit`: a pose fitted to points seen (SOMA-X's analytical pose inversion), for motion that arrives as positions.
//! - `shape`: a solid as a signed distance: ray casts, normals, clearance.
//! - `json`: character.fit and character.hand, asked for as JSON.
//! - `soma`: motion on SOMA's 78-joint skeleton played on the rig, by the way its bones point.
//! - `prior`: SOMA-X's hand pose prior on raea's fingers: a hand's shape from a few numbers, and how odd a hand is.

pub mod arm;
pub mod fit;
pub mod json;
pub mod limits;
pub mod prior;
pub mod rig;
pub mod shape;
pub mod soma;
pub mod turns;

pub use arm::{ArmOptions, IkReport, forearm_twist, reach, settle_hinge, twist_joints, two_bone};
pub use soma::{from_soma, raea_joint};
pub use turns::{from_turns, set_twists, with_twists};
pub use fit::{Mark, fit, marks_of};
pub use json::{FIT_ANSWER, FIT_ARGS, HAND_ANSWER, HAND_ARGS, SOMA_ANSWER, SOMA_ARGS, TURNS_ANSWER, TURNS_ARGS, TWISTS_ARGS, fit_json, hand_json, soma_json, turns_json, twists_json};
pub use limits::{Angles, angles_of, clamp_turn, violations};
pub use shape::{Hit, Solid, Thing};
pub use rig::{HandRest, Pose, Posed, Sample, Side, Skeleton, hand_rest, mannequin};
