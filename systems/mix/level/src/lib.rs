//! raea's mix before its master (raea-sys-mix masters it): every line of dialogue at one level (level.rs) and the
//! room's tone at a join the mix drops into dead silence (joins.rs). Live play and the render level clips the same way;
//! the render's master fills the joins.
pub mod joins;
pub mod level;
