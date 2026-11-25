//! A composition's props set while it plays (docs/realtime.feature RT4): laid over its defaults and drawn on the frame
//! it is on, paused or playing, the frame taken once its marker says they have landed (RT4e); a written file whose only
//! change is its defaults' values taken in place of opening it (`adopt`, RT4f); and what of the 3D a running world
//! keeps from a part's first frame that a change touches (`held`, RT4g), which a host shows and opens the file again for.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use raea_core_program::ir::V;
use serde_json::Map;

use super::*;

/// A frame's 3D parts by identity (the part's name, else its kind and shape or file, counted in order), each with the
/// columns a running world keeps from the part's first frame.
type Holds = HashMap<String, BTreeMap<String, V>>;

/// How long a props pass may take to land before the step is refused (a composition that hangs on its new props).
const LAND_SECS: f64 = 5.0;

#[derive(Default)]
pub(crate) struct Props {
    /// the composition's defaultProps and controls, as its settings marker carries them (the bundle's CONF)
    defaults: Map<String, Value>,
    controls: Value,
    /// what an adopted file's defaults change of the bundle's (adopt), and the props set over them (set_props)
    adopted: Map<String, Value>,
    over: Map<String, Value>,
    /// raea-props sent; the pass a step waits for (step_inner), once
    sent: u64,
    pub(crate) wanted: Option<u64>,
    /// the frame marker's entity, once found
    pub(crate) marker: Option<Entity>,
    /// what the last props held (each {part, kind, cols}), and the frame's holds after it, until a step
    held: Vec<Value>,
    pub(crate) base: Option<(i64, Holds)>,
}

impl Props {
    /// From the composition's settings (`props`, `controls`: the bundle's CONF).
    pub(crate) fn of(conf: &Map<String, Value>) -> Props {
        Props { defaults: conf.get("props").and_then(Value::as_object).cloned().unwrap_or_default(), controls: conf.get("controls").cloned().unwrap_or_else(|| json!({})), ..Default::default() }
    }
}

impl Live {
    /// The composition's props set over its defaults while it plays (the entry's raea-props), replacing the last set:
    /// the frame it is on is drawn again with them, paused or playing, once they have landed (RT4e). A game's state is
    /// kept; its props only change. Answers what of the 3D they change that the running world keeps from a part's
    /// first frame ({held: [{part, kind, cols}]}, RT4g): those show only when the file is opened again.
    pub fn set_props(&mut self, props: Value) -> R<Value> {
        let f = self.frame;
        let before = self.props.base.take().filter(|(at, _)| *at == f).map(|(_, h)| h).or_else(|| self.holds());
        self.props.over = props.as_object().cloned().unwrap_or_default();
        self.send_props();
        self.props.wanted = Some(self.props.sent);
        // the frame opened ready (frame 0, its 3D warm) is stepped again all the same: its props' commit is new
        if let Some(l) = self.live3d.as_mut() {
            l.warm = false;
        }
        self.step_world(f, &[], None, 0.0)?;
        let after = self.holds();
        self.props.held = held(before.as_ref(), after.as_ref());
        if let Some(h) = after {
            self.props.base = Some((f, h));
        }
        Ok(json!({"held": self.props.held}))
    }

    /// `set_props` before any frame is stepped: sent, and drawn with the first frame.
    pub fn props_before(&mut self, props: Value) {
        self.props.over = props.as_object().cloned().unwrap_or_default();
        self.send_props();
    }

    /// A file written from this one with only its defaults' values changed (raea_sys_tweak::values_only), taken in place
    /// of opening it (RT4f): its values laid over the bundle's defaults, the props set over them dropped (a fresh open
    /// has none), and the frame drawn again, as `set_props` draws it and with its answer. A host opens the file after
    /// all when that answer holds something.
    pub fn adopt(&mut self, defaults: Map<String, Value>) -> R<Value> {
        self.props.adopted = defaults.into_iter().filter(|(k, v)| self.props.defaults.get(k) != Some(v)).collect();
        self.set_props(Value::Object(Map::new()))
    }

    /// The props the frame is drawn with: the defaults, an adopted file's values over them, the props set over those.
    pub fn values(&self) -> Value {
        let mut v = self.props.defaults.clone();
        v.extend(self.props.adopted.clone());
        v.extend(self.props.over.clone());
        Value::Object(v)
    }

    /// The controls the composition declares, by defaultProps key (raea's registry: kind, cost, group, ...).
    pub fn controls(&self) -> &Value {
        &self.props.controls
    }

    /// What the last props set (or file adopted) held: [{part, kind, cols}], empty when the running world shows it all.
    pub fn held(&self) -> &[Value] {
        &self.props.held
    }

    fn send_props(&mut self) {
        let mut v = self.props.adopted.clone();
        v.extend(self.props.over.clone());
        self.props.sent += 1;
        self.send("raea-props", Value::Object(v));
    }

    /// The 3D's parts at the frame as it stands, with the columns a running world keeps from their first frame (None
    /// with no 3D played live): a render's rows for the frame (LiveScene::open on a scratch scene), keyed by identity.
    fn holds(&mut self) -> Option<Holds> {
        self.live3d.as_ref()?;
        let (w, h, fps) = (self.w, self.h, self.fps);
        raea_runtime_react::scene::edit_dir(&self.dir);
        let world = self.app.world_mut();
        let scene = raea_runtime_react::scene::scene_of(world);
        let clear = raea_runtime_react::scene::scene_clear(world);
        let p = raea_runtime_react::scene::LiveScene::default().open(raea_runtime_react::scene::Shot { scene: &scene, clear: clear.as_deref() }, w, h, fps);
        let mut out = Holds::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (kind, rows) in &p.spawns {
            if !raea_runtime_scene::LIVE_GROUPS.contains(&kind.as_str()) {
                continue;
            }
            for row in rows {
                let s = |k: &str| match row.get(k) {
                    Some(V::S(s)) => Some(s.as_str()),
                    _ => None,
                };
                let base = s("name").map(|n| format!("{kind}|{n}")).unwrap_or_else(|| format!("{kind}~{}", s("shape").or(s("src")).unwrap_or("")));
                let n = seen.entry(base.clone()).or_default();
                *n += 1;
                let cols = row.iter().filter(|(k, _)| keeps(kind, k)).map(|(k, v)| (k.clone(), v.clone())).collect();
                out.insert(format!("{base}#{}", *n - 1), cols);
            }
        }
        Some(out)
    }
}

/// A column the running world keeps from a part's first frame: not one it takes frame by frame (live_dynamic), nor its
/// identity (name), nor one live play sends when it changes (a mesh's colour, keyed once it does, and its glow).
fn keeps(kind: &str, col: &str) -> bool {
    !matches!(col, "id" | "name") && !raea_runtime_scene::live_dynamic(kind).contains(&col) && !(kind == "mesh" && matches!(col, "color" | "emissive"))
}

/// The parts a change held: those whose kept columns differ between the frame before it and after it, with which.
fn held(before: Option<&Holds>, after: Option<&Holds>) -> Vec<Value> {
    let (Some(a), Some(b)) = (before, after) else { return Vec::new() };
    let mut out: Vec<Value> = b
        .iter()
        .filter_map(|(part, cols)| {
            let was = a.get(part)?;
            let changed: Vec<&String> = cols.iter().filter(|(k, v)| was.get(*k) != Some(*v)).map(|(k, _)| k).chain(was.keys().filter(|k| !cols.contains_key(*k))).collect();
            let kind = part.split(['|', '~']).next().unwrap_or("");
            (!changed.is_empty()).then(|| json!({"part": part, "kind": kind, "cols": changed}))
        })
        .collect();
    out.sort_by(|x, y| x.get("part").and_then(Value::as_str).cmp(&y.get("part").and_then(Value::as_str)));
    out
}

/// Stepped (`again`: the frame settled once more) until the frame marker carries props pass `n` (the entry's
/// data-raea-props), at most LAND_SECS. A bundle whose marker doesn't carry it (made before it did: an exported game)
/// isn't waited on.
pub(crate) fn landed(app: &mut App, marker: &mut Option<Entity>, n: u64, mut seen: u64, again: &mut dyn FnMut(&mut App) -> R<()>) -> R<()> {
    let t0 = Instant::now();
    loop {
        match marked(app, marker) {
            Some(m) if m < n => {}
            _ => return Ok(()),
        }
        if t0.elapsed().as_secs_f64() > LAND_SECS {
            return Err(format!("the props set weren't drawn within {LAND_SECS} s (pass {n})"));
        }
        let ask = app.world().get_resource::<raea_runtime_react::layout::LayoutAsk>().map(|a| a.0.clone());
        seen = raea_runtime_react::wait_flush(seen, Duration::from_millis(100), ask.as_deref());
        again(app)?;
    }
}

/// The props pass the frame marker says has landed (None: no marker, or one that doesn't say).
fn marked(app: &mut App, marker: &mut Option<Entity>) -> Option<u64> {
    let w = app.world_mut();
    let is_marker = |w: &World, e: Entity| w.get::<raea_runtime_react::ReactProps>(e).is_some_and(|p| p.0.contains_key("data-raea-frame"));
    if !marker.is_some_and(|e| is_marker(w, e)) {
        let mut q = w.query::<(Entity, &raea_runtime_react::ReactProps)>();
        *marker = q.iter(w).find(|(_, p)| p.0.contains_key("data-raea-frame")).map(|(e, _)| e);
    }
    w.get::<raea_runtime_react::ReactProps>((*marker)?)?.0.get("data-raea-props")?.as_f64().map(|v| v as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holds(parts: &[(&str, &[(&str, V)])]) -> Holds {
        parts.iter().map(|(p, cols)| (p.to_string(), cols.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())).collect()
    }

    /// A model's file changed by a prop is held (its row keeps its first file); its place, which a running world
    /// takes frame by frame, never counts; a part with nothing kept changed isn't named.
    #[test]
    fn what_a_change_held() {
        assert!(keeps("model", "src") && keeps("mesh", "shape") && keeps("mesh", "surface"));
        assert!(!keeps("mesh", "x") && !keeps("mesh", "color") && !keeps("mesh", "emissive") && !keeps("model", "name") && !keeps("light", "lux"));
        let a = holds(&[("model|hero#0", &[("src", V::S("goose.glb".into()))]), ("mesh~box#0", &[("shape", V::S("box 1".into()))])]);
        let b = holds(&[("model|hero#0", &[("src", V::S("mayor.glb".into()))]), ("mesh~box#0", &[("shape", V::S("box 1".into()))])]);
        assert_eq!(held(Some(&a), Some(&b)), vec![json!({"part": "model|hero#0", "kind": "model", "cols": ["src"]})]);
        assert!(held(Some(&a), Some(&a)).is_empty());
        // a column gone counts as changed; a part new to the frame is a new row, drawn as it is
        let c = holds(&[("model|hero#0", &[]), ("mesh~ball#0", &[("shape", V::S("sphere".into()))])]);
        assert_eq!(held(Some(&a), Some(&c)), vec![json!({"part": "model|hero#0", "kind": "model", "cols": ["src"]})]);
        assert!(held(None, Some(&a)).is_empty());
    }
}
