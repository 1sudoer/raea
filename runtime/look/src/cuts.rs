//! The paint look's worlds on the camera row (a <Look cuts>): split from lib.rs, which is at its 500-line limit.
use super::*;

/// The paint look's worlds (a <Look cuts>: raea-render's paint.rs, paint_world.wesl): each of up to four cuts' centre,
/// radius, style, scale and amount on the camera row, one value a frame. No columns while no look has cuts.
pub fn paint_cut_rows(looks: &[Option<&Map<String, Value>>], p: &mut Program, row: &mut BTreeMap<String, V>, first: &Map<String, Value>) {
    if !looks.iter().flatten().any(|l| matches!(l.get("cuts"), Some(Value::Array(_)))) {
        return;
    }
    for i in 0..4 {
        for (k, or) in [("x", 0.0), ("y", 0.0), ("z", 0.0), ("r", 0.0), ("style", 0.0), ("scale", 0.1), ("amount", 1.0)] {
            let vals: Vec<f64> = looks
                .iter()
                .map(|l| {
                    let l = l.filter(|l| is_paint(l)).unwrap_or(first);
                    l.get("cuts").and_then(Value::as_array).and_then(|a| a.get(i)).and_then(Value::as_object).and_then(|c| num(c, k)).unwrap_or(or)
                })
                .collect();
            column(p, row, "cam", &format!("paint_cut{i}_{k}"), &vals);
        }
    }
}

/// Each cut's two colours, fixed for a look (paint_cut_rows has its keyed values).
pub fn cut_colours(row: &mut BTreeMap<String, V>, first: &Map<String, Value>) {
    if let Some(Value::Array(cuts)) = first.get("cuts") {
        for (i, c) in cuts.iter().take(4).enumerate() {
            for k in ["a", "b"] {
                if let Some(h) = hex(c.get(k)) {
                    row.insert(format!("paint_cut{i}_{k}"), V::S(h));
                }
            }
        }
    }
}

/// a material's `paint` (under a paint look): its breakup, patches of noise in their own colour (raea-render's paint.rs)
pub fn paint_columns(row: &mut BTreeMap<String, V>, props: &Map<String, Value>) {
    let Some(g) = props.get("paint").and_then(Value::as_object) else { return };
    // its world (a <Look cuts>): alone, it leaves the breakup off
    if let Some(w) = num(g, "world") {
        row.insert("paint_world".into(), V::N(w));
        if g.keys().all(|k| k == "world") {
            return;
        }
    }
    row.insert("paint_breakup".into(), V::N(n(g, "breakup", 0.5)));
    row.insert("paint_breakup_scale".into(), V::N(n(g, "scale", 1.0)));
    if let Some(h) = hex(g.get("color")) {
        row.insert("paint_patch".into(), V::S(h));
        row.insert("paint_patch_amount".into(), V::N(n(g, "amount", 0.6)));
    }
}
