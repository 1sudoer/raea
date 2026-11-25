//! The layout snapshot's rects, taken in one pass with what each element asks of its ancestors kept for the next
//! (blitz-dom's RectMemo), are the rects each element's own getBoundingClientRect answers. The pass had walked to the
//! root two more times for every element, every frame: on the app's own page a third of its main thread (11-16).
use bevy::math::UVec2;
use blitz_dom::{BaseDocument, NodeId, RectMemo};
use raea_sys_page::{Page, config};

fn laid_out(html: &str) -> BaseDocument {
    let mut doc = blitz_html::HtmlDocument::from_html(html, config(Page { size: UVec2::new(800, 600), scale: 1.0 })).into_inner();
    doc.resolve(0.0);
    doc
}

fn elements(doc: &BaseDocument) -> Vec<NodeId> {
    doc.tree().iter().filter(|(_, n)| n.is_element()).map(|(id, _)| id).collect()
}

/// Every element's rect, one at a time and in one pass (in `order`), side by side where they differ.
fn differences(doc: &BaseDocument, order: &[NodeId]) -> Vec<String> {
    let mut memo = RectMemo::default();
    order
        .iter()
        .filter_map(|&id| {
            let (alone, pass) = (doc.get_client_bounding_rect(id), doc.get_client_bounding_rect_in(id, &mut memo));
            (alone != pass).then(|| format!("node {id}: alone {alone:?}, in the pass {pass:?}"))
        })
        .collect()
}

const PAGE: &str = r##"<!doctype html><html><body style="margin:8px">
<div id="hidden" style="display:none"><div><span>under display: none</span><p>a paragraph</p></div></div>
<div style="transform: rotate(12deg); width: 200px; padding: 10px">
  <div style="transform: scale(1.5) translate(4px, 2px)"><p>twice turned</p><span>a span</span></div>
  <div><div><b>deep under a turned box</b></div></div>
</div>
<div style="perspective: 400px">
  <div style="transform-style: preserve-3d; transform: rotateY(30deg)">
    <div style="transform: translateZ(40px)">in a 3D context</div>
  </div>
</div>
<p style="width: 120px">Some text <span>that wraps across <i>more than one</i> line box</span> and a
  <a href="#">link</a> at the end of it.</p>
<table><tbody><tr><td>one</td><td>two</td></tr><tr><td colspan="2">three</td></tr></tbody></table>
<div style="position: relative; height: 80px">
  <div style="position: absolute; left: 30px; top: 20px"><div>out of flow</div></div>
</div>
<div style="height: 60px; overflow: auto"><div style="height: 300px"><div>in a scroller</div></div></div>
<div style="display: contents"><div>under display: contents</div></div>
</body></html>"##;

#[test]
fn every_rect_in_a_pass_is_the_rect_alone() {
    let doc = laid_out(PAGE);
    let ids = elements(&doc);
    assert!(ids.len() > 30, "the page has its elements ({})", ids.len());
    // tree order (a parent asked before its children) and the reverse (a child first: the walk fills its ancestors)
    assert_eq!(differences(&doc, &ids), Vec::<String>::new());
    let reversed: Vec<NodeId> = ids.iter().rev().copied().collect();
    assert_eq!(differences(&doc, &reversed), Vec::<String>::new());
    // and the page's turns and hidden boxes are what the pass met
    let rect = |id: NodeId| doc.get_client_bounding_rect(id).unwrap();
    let id_of = |id: NodeId| doc.get_node(id).and_then(|n| n.element_data()).and_then(|e| e.attr(blitz_dom::local_name!("id")));
    let hidden = ids.iter().copied().find(|&id| id_of(id) == Some("hidden")).unwrap();
    assert_eq!((rect(hidden).width, rect(hidden).height), (0.0, 0.0));
}

/// A page as big as the app's (a few thousand elements, twenty deep): the pass's time against each element alone.
/// `cargo test -p raea-sys-page --test rects --release -- --ignored --nocapture`
#[test]
#[ignore]
fn a_big_page_in_one_pass() {
    let row = (0..20).fold("<span>clip</span>".to_string(), |inner, _| format!("<div>{inner}</div>"));
    let doc = laid_out(&format!("<!doctype html><html><body>{}</body></html>", row.repeat(150)));
    let ids = elements(&doc);
    let time = |f: &dyn Fn()| {
        let t = std::time::Instant::now();
        for _ in 0..20 {
            f();
        }
        t.elapsed().as_secs_f64() * 1000.0 / 20.0
    };
    let alone = time(&|| ids.iter().for_each(|&id| _ = std::hint::black_box(doc.get_client_bounding_rect(id))));
    let pass = time(&|| {
        let mut memo = RectMemo::default();
        ids.iter().for_each(|&id| _ = std::hint::black_box(doc.get_client_bounding_rect_in(id, &mut memo)))
    });
    println!("{} elements: {alone:.2} ms alone, {pass:.2} ms in one pass", ids.len());
    assert_eq!(differences(&doc, &ids), Vec::<String>::new());
}
