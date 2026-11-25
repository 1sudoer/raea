//! Trees checked by number: a lime fills its crown and keeps its trunk clear, radii thin toward the tips, leaves split
//! evenly into their groups, every leaf kind draws a leaf (a lime's with its notch), and the same request gives the
//! same bytes. `grow_files` (ignored) grows a list of requests to .glb files: RAEA_FOLIAGE_REQ a JSON list of
//! { id, species, height, width, clear, seed, leaves }, RAEA_FOLIAGE_OUT the folder.

use crate::species::ALL;
use crate::{CELLS, GROUPS, Request, atlas, glb, grow};

fn lime() -> Request {
    Request { species: "lime".into(), height: 16.0, width: 10.0, clear: 3.5, seed: 1, leaves: 30_000, prune: 0.0 }
}

#[test]
fn a_lime_fills_its_crown_and_keeps_its_trunk_clear() {
    let req = lime();
    let t = grow(&req).unwrap();
    let top = t.nodes.iter().map(|n| n.p.y).fold(0.0, f32::max);
    // its width as a crown is measured: the spread across it, averaged over bearings (a lobe may reach further)
    let spread = |a: f32| {
        let d = (a.cos(), a.sin());
        let s = t.nodes.iter().map(|n| n.p.x * d.0 + n.p.z * d.1);
        s.clone().fold(f32::MIN, f32::max) - s.fold(f32::MAX, f32::min)
    };
    let across = (0..8).map(|k| spread(std::f32::consts::PI * k as f32 / 8.0)).sum::<f32>() / 8.0;
    let wide = t.nodes.iter().map(|n| n.p.x.hypot(n.p.z)).fold(0.0, f32::max);
    assert!(top > 0.85 * req.height && top < 1.15 * req.height, "top {top}");
    assert!(across > 0.8 * req.width && across < 1.2 * req.width, "spread {across} for a width of {}", req.width);
    assert!(wide < 0.8 * req.width, "half-width {wide}");
    for n in t.nodes.iter().filter(|n| n.p.y < req.clear * 0.9) {
        assert!(n.p.x.hypot(n.p.z) < 0.5, "only the trunk below the crown: {:?}", n.p);
    }
    for m in &t.leaves {
        assert!(m.pos.iter().all(|p| p[1] > req.clear * 0.8), "no leaves on the clear trunk");
    }
    assert!((t.nodes[0].r - 0.02 * req.height).abs() < 1e-3, "the trunk's foot: {}", t.nodes[0].r);
    for n in t.nodes.iter().skip(1) {
        assert!(n.r <= t.nodes[n.parent].r + 1e-6, "radii thin toward the tips");
    }
    println!("lime: spread {across:.1} m, {} nodes, {} bark triangles, {} leaves", t.nodes.len(), t.bark.tris(), t.leaf_count());
}

#[test]
fn leaves_split_evenly_into_their_groups_within_the_budget() {
    let req = Request { leaves: 3_000, ..lime() };
    let t = grow(&req).unwrap();
    let all = t.leaf_count() as f32;
    assert!(all > 0.5 * req.leaves as f32 && all < 1.1 * req.leaves as f32, "{all} leaves for a budget of {}", req.leaves);
    for (k, m) in t.leaves.iter().enumerate() {
        let share = (m.idx.len() / 6) as f32 / all;
        assert!((share - 1.0 / GROUPS as f32).abs() < 0.04, "group {k}: {share}");
    }
}

#[test]
fn every_leaf_kind_draws_a_leaf() {
    for sp in &ALL {
        let img = atlas(sp, 3);
        for cell in [0, CELLS * CELLS - 1] {
            let c = crate::atlas::coverage(&img, cell);
            assert!(c > 0.06 && c < 0.75, "{}: cell {cell} covers {c}", sp.id);
        }
    }
    let lime = crate::atlas::leaf_at(crate::LeafKind::Cordate);
    assert!(lime(0.0, 0.19) < 0.0, "a lime's blade has its notch where the stalk meets it");
    assert!(lime(0.15, 0.3) > 0.0, "and its lobes either side");
    let sp = crate::species("lime").unwrap();
    assert!(crate::atlas::covers(sp, &atlas(sp, 3), 0, 0.0, 0.3), "a lime's sprig has its twig up the middle");
}

#[test]
fn the_same_request_gives_the_same_bytes_and_a_glb_with_its_groups() {
    let req = Request { leaves: 4_000, ..lime() };
    let (a, b) = (glb(&req, &grow(&req).unwrap()).unwrap(), glb(&req, &grow(&req).unwrap()).unwrap());
    assert_eq!(a, b);
    let g = raea_sys_glb::parse(&a).unwrap();
    let names: Vec<&str> = raea_sys_glb::arr(&g.json, "materials").iter().filter_map(|m| m["name"].as_str()).collect();
    assert_eq!(names, ["bark", "leaf-0", "leaf-1", "leaf-2", "leaf-3", "leaf-4", "leaf-5", "leaf-6", "leaf-7"]);
    assert_eq!(g.json["meshes"][0]["primitives"].as_array().unwrap().len(), 1 + GROUPS);
}

#[test]
fn every_species_grows() {
    for sp in &ALL {
        let req = Request { species: sp.id.into(), height: 12.0, width: 12.0 / sp.height_per_width, clear: 2.5, seed: 7, leaves: 3_000, prune: 0.0 };
        let t = grow(&req).unwrap();
        assert!(t.nodes.len() > 200 && t.leaf_count() > 500, "{}: {} nodes, {} leaves", sp.id, t.nodes.len(), t.leaf_count());
        // its roots show only in its pit: past it they're under the paving
        let reach = t.bark.pos.iter().filter(|p| p[1] > 0.0 && p[1] < 0.5).map(|p| p[0].hypot(p[2])).fold(0.0, f32::max);
        assert!(reach < t.pit && t.pit >= 1.0, "{}: bark above the ground {reach:.2} m from the trunk, its pit {:.2} m", sp.id, t.pit);
    }
}

#[test]
#[ignore]
fn grow_files() {
    let req = std::env::var("RAEA_FOLIAGE_REQ").expect("RAEA_FOLIAGE_REQ");
    let out = std::path::PathBuf::from(std::env::var("RAEA_FOLIAGE_OUT").expect("RAEA_FOLIAGE_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(req).unwrap()).unwrap();
    for r in list {
        let req = Request {
            species: r["species"].as_str().unwrap().into(),
            height: r["height"].as_f64().unwrap() as f32,
            width: r["width"].as_f64().unwrap() as f32,
            clear: r["clear"].as_f64().unwrap() as f32,
            seed: r["seed"].as_u64().unwrap_or(1),
            leaves: r["leaves"].as_u64().unwrap_or(40_000) as u32,
            prune: r["prune"].as_f64().unwrap_or(0.0) as f32,
        };
        let t0 = std::time::Instant::now();
        let t = grow(&req).unwrap();
        let ms = t0.elapsed().as_millis();
        let bytes = glb(&req, &t).unwrap();
        let id = r["id"].as_str().unwrap();
        std::fs::write(out.join(format!("{id}.glb")), &bytes).unwrap();
        atlas(crate::species(&req.species).unwrap(), req.seed).save(out.join(format!("{id}-atlas.png"))).unwrap();
        // the pit the ground draws round it
        std::fs::write(out.join(format!("{id}.json")), serde_json::json!({ "pit": (t.pit * 100.0).round() / 100.0 }).to_string()).unwrap();
        println!("{id}: pit {:.2} m, {} nodes, {} bark triangles, {} leaves, grown in {ms} ms, {} KB", t.pit, t.nodes.len(), t.bark.tris(), t.leaf_count(), bytes.len() / 1024);
    }
}

#[test]
fn a_pruned_level_drops_its_twigs_and_keeps_its_leaves() {
    let full = Request { leaves: 4_000, ..lime() };
    let far = Request { prune: 0.03, ..full.clone() };
    let (a, b) = (grow(&full).unwrap(), grow(&far).unwrap());
    assert!(b.bark.tris() * 4 < a.bark.tris(), "pruned bark {} against {}", b.bark.tris(), a.bark.tris());
    assert_eq!(a.leaf_count(), b.leaf_count(), "the same leaves");
}

#[test]
fn a_trunk_flares_at_the_ground_and_its_roots_sink_into_it() {
    let sp = crate::species("lime").unwrap();
    let near = Request { leaves: 2_000, ..lime() };
    let far = Request { prune: 0.03, ..near.clone() };
    let (a, b) = (grow(&near).unwrap(), grow(&far).unwrap());
    let r = a.nodes[0].r;
    let out = |p: &[f32; 3]| p[0].hypot(p[2]);
    // the flare at the ground, on the level without roots: its ring averages the species' flare
    let ring: Vec<f32> = b.bark.pos.iter().filter(|p| p[1].abs() < 1e-4 && out(p) > 1.05 * r).map(out).collect();
    let mean = ring.iter().sum::<f32>() / ring.len().max(1) as f32;
    assert!(!ring.is_empty() && (mean / r - sp.flare).abs() < 0.12 * sp.flare, "flare {} against {}", mean / r, sp.flare);
    assert!(b.bark.pos.iter().any(|p| p[1] < -0.2), "the foot goes on under the ground");
    // at breast height it's the trunk again
    let c = a.nodes.iter().filter(|n| n.p.y < near.clear).min_by(|x, y| (x.p.y - 1.3).abs().total_cmp(&(y.p.y - 1.3).abs())).unwrap().p;
    let wide = a.bark.pos.iter().filter(|p| (p[1] - c.y).abs() < 0.05).map(|p| (p[0] - c.x).hypot(p[2] - c.z)).filter(|&d| d < 3.0 * r).fold(0.0, f32::max);
    assert!(wide < 1.05 * r, "at 1.3 m the bark is {} of the trunk", wide / r);
    // the roots: only on the near level, showing in most of their sectors by the trunk, every end under the soil
    assert!(b.bark.pos.iter().filter(|p| p[1] < 0.5).all(|p| out(p) < 2.0 * sp.flare * r), "no roots on a pruned level");
    let n = sp.roots as usize;
    let mut seen = vec![false; n];
    for p in a.bark.pos.iter().filter(|p| p[1] > 0.0 && p[1] < 0.5 && out(p) > 1.3 * sp.flare * r && out(p) < 4.0 * r) {
        seen[((-p[2]).atan2(p[0]).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * n as f32) as usize % n] = true;
    }
    assert!(seen.iter().filter(|s| **s).count() * 2 >= n, "roots show in {seen:?}");
    let mut far_out: Vec<&[f32; 3]> = a.bark.pos.iter().filter(|p| p[1] < 0.6 && out(p) > 2.0 * sp.flare * r).collect();
    far_out.sort_by(|x, y| out(y).total_cmp(&out(x)));
    let tips = &far_out[..far_out.len() / 20];
    assert!(!tips.is_empty() && tips.iter().all(|p| p[1] < 0.0), "a root's end is under the ground: {:?}", tips.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
    println!("lime foot: flare {:.2}, {} of {n} sectors with a root showing, reach {:.2} m", mean / r, seen.iter().filter(|s| **s).count(), out(far_out[0]));
}

/// Side views of every species (three seeds, two bearings) as masks for the project's crown check:
/// RAEA_FOLIAGE_REQ=list.json RAEA_FOLIAGE_OUT=dir cargo test -p raea-sys-foliage -- --ignored silhouettes
#[test]
#[ignore]
fn silhouettes() {
    let req = std::env::var("RAEA_FOLIAGE_REQ").expect("RAEA_FOLIAGE_REQ");
    let out = std::path::PathBuf::from(std::env::var("RAEA_FOLIAGE_OUT").expect("RAEA_FOLIAGE_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(req).unwrap()).unwrap();
    for r in list {
        for seed in 1..=3u64 {
            let req = Request { species: r["species"].as_str().unwrap().into(), height: r["height"].as_f64().unwrap() as f32, width: r["width"].as_f64().unwrap() as f32, clear: r["clear"].as_f64().unwrap() as f32, seed, leaves: 40_000, prune: 0.0 };
            let t = grow(&req).unwrap();
            for (k, b) in [0.0f32, 1.57].iter().enumerate() {
                crate::silhouette(&req, &t, 600, *b).unwrap().save(out.join(format!("{}-{seed}{}.png", req.species, ["a", "b"][k]))).unwrap();
            }
        }
    }
}
