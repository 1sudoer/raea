//! A camera drawing into an image (<PerspectiveCamera target="render:NAME:WxH">: three's setRenderTarget then render,
//! docs/r3f.feature R3F23a) as a row of its own, "rcam0", "rcam1", ... by its track. It is placed and framed as a
//! screen camera is (camera.rs's placed); its image is `into` (not `target`, which a live row reads as the row a
//! component is on); `on` is keyed where it doesn't draw every frame (`active` false, or no such camera that frame);
//! it has no tone curve unless it names one (three writes a target linear), and the ambient light on its own layers.
//! The engine spawns it as a camera on that image (raea-render's render_camera), before the screen's by its `order`.

use super::*;

/// The scene's cameras into images as their rows (`tracks`: tracks_of's for "rendercam", each a camera by its image
/// and its place among that image's passes a frame), pushed as "rendercam" spawns.
pub fn render_cameras(all: &[Vec<Item>], tracks: &[(String, Vec<Option<Item>>)], p: &mut Program) {
    let mut rows = Vec::new();
    for (k, (_, track)) in tracks.iter().enumerate() {
        let id = format!("rcam{k}");
        let its = filled(track);
        let mut row = BTreeMap::new();
        row.insert("id".into(), V::S(id.clone()));
        placed(all, p, &mut row, &id, &its);
        let first = &its[0].props;
        row.insert("into".into(), V::S(first.get("target").and_then(Value::as_str).unwrap_or_default().to_string()));
        let on: Vec<f64> = track.iter().map(|t| t.as_ref().is_some_and(|i| i.props.get("active") != Some(&Value::Bool(false))) as u8 as f64).collect();
        if on.iter().any(|o| *o == 0.0) {
            row.insert("on".into(), V::N(on[0]));
            p.keys.insert((id.clone(), "on".into()), step_keys(&on));
        }
        row.insert("tonemap".into(), V::S(first.get("tonemap").and_then(Value::as_str).unwrap_or("none").to_string()));
        // its own aspect where its image's shape isn't it (three draws a projection into any target; matchmove's OffAxis)
        if let Some(a) = first.get("aspect").and_then(Value::as_f64) {
            row.insert("aspect".into(), V::N(a));
        }
        ambient_columns(all, p, &mut row, &id, &layers_of(first));
        rows.push(row);
    }
    if !rows.is_empty() {
        p.spawns.push(("rendercam".into(), rows));
    }
}

/// A camera's ambient light, from the AmbientLights on its `layers`: their intensities summed per frame (`ambient`,
/// keyed) and the first one's colour. Any such light sends the column, so one of brightness 0 turns the ambient off as
/// Bevy's does; with none, the camera keeps Bevy's default 80.
pub(crate) fn ambient_columns(all: &[Vec<Item>], p: &mut Program, row: &mut BTreeMap<String, V>, id: &str, layers: &[u64]) {
    let Some(first) = ambients(all.iter().flatten(), layers).next() else { return };
    column(p, row, id, "ambient", &all.iter().map(|its| ambient_on(its, layers)).collect::<Vec<_>>());
    if let Some(c) = hex(first.props.get("color")).filter(|c| c != "#ffffff") {
        row.insert("ambient_color".into(), V::S(c));
    }
}

/// A part's render layers (its `layers` prop), layer 0 when it names none, as Bevy's RenderLayers default.
pub fn layers_of(props: &Map<String, Value>) -> Vec<u64> {
    match props.get("layers").and_then(Value::as_array) {
        Some(l) => l.iter().filter_map(Value::as_u64).collect(),
        None => vec![0],
    }
}

/// Whether a camera on `layers` sees the part `i` (they share a layer).
pub fn sees(layers: &[u64], i: &Item) -> bool {
    layers_of(&i.props).iter().any(|l| layers.contains(l))
}

/// A frame's ambient light on `layers`: the brightness of every AmbientLight a camera on them sees, summed.
pub fn ambient_on(frame: &[Item], layers: &[u64]) -> f64 {
    ambients(frame, layers).map(|i| light_power("ambient", &i.props)).sum()
}

/// The AmbientLights among `its` that a camera on `layers` sees.
fn ambients<'a>(its: impl IntoIterator<Item = &'a Item>, layers: &'a [u64]) -> impl Iterator<Item = &'a Item> {
    its.into_iter().filter(move |i| i.kind == "ambient" && sees(layers, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &'static str, props: Value) -> Item {
        Item { kind, props: std::sync::Arc::new(props.as_object().cloned().unwrap_or_default()), shape: None, material: None, at: None, world: bevy::math::Mat4::IDENTITY }
    }

    #[test]
    fn a_camera_into_an_image_is_its_own_row_drawn_on_its_frames() {
        let cam = |active: bool| item("rendercam", serde_json::json!({"target": "render:fbo:64x64", "position": [0, 0, 3], "order": -10, "bg": "#000000ff", "layers": [5], "active": active}));
        let amb = |layers: Value| item("ambient", serde_json::json!({"intensity": 1, "layers": layers}));
        let frames = vec![vec![cam(true), amb(serde_json::json!([5])), amb(serde_json::json!([0]))], vec![cam(false), amb(serde_json::json!([5]))], vec![]];
        let tracks = tracks_of(&frames, &["rendercam"]);
        let mut p = Program::default();
        render_cameras(&frames, &tracks, &mut p);
        let (kind, rows) = &p.spawns[0];
        assert_eq!(kind, "rendercam");
        let r = &rows[0];
        assert_eq!(r["id"], V::S("rcam0".into()));
        assert_eq!(r["into"], V::S("render:fbo:64x64".into()));
        assert_eq!(r["order"], V::N(-10.0));
        assert_eq!(r["layers"], V::S("5".into()));
        assert_eq!(r["tonemap"], V::S("none".into()));
        // drawn on frame 0 only: inactive on 1, and missing on 2
        let on = &p.keys[&("rcam0".to_string(), "on".to_string())];
        assert_eq!(on.iter().map(|k| (k.0, k.1)).collect::<Vec<_>>(), vec![(0, 1.0), (1, 0.0)]);
        // its ambient is the one on layer 5 only
        let one = light_power("ambient", &amb(serde_json::json!([5])).props);
        assert_eq!(r["ambient"], V::N(one));
    }
}
