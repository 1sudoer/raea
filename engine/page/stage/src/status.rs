//! What the stage is doing, as stage.status answers it (calls.rs).

use serde_json::Value;

/// What the stage is doing, for the page's controls (stage.status).
#[derive(Default, Clone)]
pub(super) struct Status {
    pub(super) showing: Option<String>,
    pub(super) frame: i64,
    pub(super) playing: bool,
    pub(super) fps: f64,
    pub(super) painted: u64,
    pub(super) size: (u32, u32),
    pub(super) error: Option<String>,
    pub(super) render: bool,
    /// files opened, and files taken in place of opening them (open::adopt)
    pub(super) opens: u64,
    pub(super) adopted: u64,
    /// the last show: "opened" or "adopted", and its milliseconds from the call to the composition ready
    pub(super) last_show: Option<(&'static str, f64)>,
    pub(super) props: PropsSeen,
    /// what the last props held (Live::held), the composition's controls and the values its frame is drawn with
    pub(super) held: Vec<Value>,
    pub(super) controls: Value,
    pub(super) values: Value,
}

/// Props changes: sent to the stage, drawn, passed over for a newer one, and the last drawn's milliseconds from the
/// call to its frame painted into the stage's texture.
#[derive(Default, Clone)]
pub(super) struct PropsSeen {
    pub(super) sent: u64,
    pub(super) drawn: u64,
    pub(super) dropped: u64,
    pub(super) ms: f64,
}
