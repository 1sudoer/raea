//! Where a picture sits in CapCut's terms (docs/capcut.feature CC2a). The manifest is read by raea-app-layers, the same
//! for every editor, and so is where a picture is drawn and what of it is kept (raea_app_layers::plan::picture); this
//! is CapCut's own.

use raea_app_layers::model::Clip;
use raea_app_layers::plan::{self, Picture};
use raea_io_capcut::Place;

/// Where a picture of `mw` x `mh` drawn as `p` sits in CapCut's terms on a `w` x `h` canvas: the part of it kept as
/// its crop, which CapCut fits inside the canvas (scale 1), then scaled to the size raea drew it, centred where raea
/// drew that part.
pub fn place(p: &Picture, (mw, mh): (f64, f64), (w, h): (f64, f64), opacity: f64) -> Place {
    let [dx, dy, dw, dh] = p.drawn;
    let [l, t, r, b] = p.kept;
    let (cw, ch) = ((r - l).max(1e-6), (b - t).max(1e-6));
    let fit = (w / (mw * cw)).min(h / (mh * ch));
    let (x, y) = (dx + dw * (l + r) / 2.0, dy + dh * (t + b) / 2.0);
    Place { x, y, scale: dw / (mw * fit), opacity, crop: p.kept }
}

/// A footage clip's keys in CapCut's terms (frames from its first, each its place there; none when it holds), cut
/// throughout as `first` keeps it (its first frame's): CapCut's crop holds, so its moves, scaling and fades are keyed
/// around that cut. Thinned to what a straight line between can't give, to 0.15 px and half a percent of opacity; and
/// whether what is cut changes over its run, which CapCut can't show.
pub fn keys(c: &Clip, fit: &str, first: [f64; 4], (mw, mh): (f64, f64), canvas: (f64, f64)) -> (Vec<(i64, Place)>, bool) {
    let mut changes = false;
    let all: Vec<(i64, [f64; 4])> = c
        .keys
        .iter()
        .map(|k| {
            let (p, _) = plan::picture(&k.r#box, &k.clip, fit, &k.position, (mw, mh), canvas);
            changes |= plan::apart(&p.kept, &first) > 1e-3;
            let at = place(&Picture { drawn: p.drawn, kept: first }, (mw, mh), canvas, k.opacity);
            (k.frame, [at.x, at.y, at.scale, at.opacity])
        })
        .collect();
    if all.is_empty() {
        return (Vec::new(), false);
    }
    // a scale step a 0.15 px change in the width of what's kept, as CapCut fits it inside the canvas
    let (cw, ch) = ((first[2] - first[0]).max(1e-6), (first[3] - first[1]).max(1e-6));
    let fitted = mw * cw * (canvas.0 / (mw * cw)).min(canvas.1 / (mh * ch));
    let kept = plan::thin(&all, &[0.15, 0.15, 0.15 / fitted, 0.005]);
    let keys = kept.into_iter().map(|(f, [x, y, scale, opacity])| (f, Place { x, y, scale, opacity, crop: first })).collect();
    (keys, changes)
}
