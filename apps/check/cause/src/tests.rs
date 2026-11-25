use super::*;
use raea_core_facts::Level;

// The 28 samples `raea check` takes of the 11-16 Telluride walkaround (55.8 s, 24 fps, phone footage), as its
// blotching measures them: rendered to PNG with no encoding (BEFORE), those PNGs through x264 crf 12, and raea's own
// exports at --quality high and master. Blotching's line is 0.009; x264 moved a sample at most 0.005.
const BEFORE: [Option<f64>; 28] = [Some(0.0), None, Some(0.052), Some(0.062), None, Some(0.062), None, None, Some(0.076), Some(0.009), Some(0.03), Some(0.039), Some(0.068), None, None, None, None, Some(0.077), Some(0.062), None, None, Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0)];
const X264: [Option<f64>; 28] = [Some(0.0), None, Some(0.052), Some(0.062), None, Some(0.059), None, None, Some(0.078), Some(0.008), Some(0.028), Some(0.034), Some(0.071), None, None, None, None, Some(0.078), Some(0.062), None, Some(0.039), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0)];
const HIGH: [Option<f64>; 28] = [Some(0.0), None, Some(0.042), Some(0.024), Some(0.052), Some(0.064), Some(0.05), None, Some(0.126), Some(0.063), Some(0.061), Some(0.122), Some(0.088), Some(0.04), Some(0.029), Some(0.047), Some(0.044), Some(0.096), Some(0.09), None, Some(0.029), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0)];
const MASTER: [Option<f64>; 28] = [Some(0.0), None, Some(0.026), Some(0.002), None, Some(0.03), None, None, Some(0.022), None, None, None, None, None, None, None, None, Some(0.071), Some(0.053), None, None, Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0), Some(0.0)];
const LINE: f64 = 0.009;
const NOISE: f64 = 0.005;

/// What the picture already held is told from what the encode added: a clean encode and raea's master add nothing,
/// its high adds to 14 of 28 samples, and 9 samples hold the footage's own either way
#[test]
fn the_picture_s_own_is_told_from_the_encode_s() {
    let clean = against(&X264, &BEFORE, LINE, NOISE);
    assert!(!clean.by_encode(NOISE), "{clean:?}");
    assert!(clean.added.abs() < 0.002, "{clean:?}");
    let high = against(&HIGH, &BEFORE, LINE, NOISE);
    assert!(high.by_encode(NOISE), "{high:?}");
    assert_eq!((high.worse, high.before, high.of), (14, 9, 28));
    assert_eq!(Against::first(&HIGH, &BEFORE, LINE, NOISE), Some(4));
    let master = against(&MASTER, &BEFORE, LINE, NOISE);
    assert!(!master.by_encode(NOISE), "{master:?}");
    assert_eq!(master.worse, 0);
    // a small, even addition over a clean picture still counts (VideoToolbox over a dark render, 10-25)
    assert!(against(&[Some(0.013); 16], &[Some(0.003); 16], LINE, NOISE).by_encode(NOISE));
}

/// The frame read for each sample is the last before its middle, as the fps filter picks it
#[test]
fn the_sampled_frames_are_the_check_s() {
    assert_eq!(sampled(3, 2.0, 24.0, 1338), vec![23, 71, 119]);
    assert_eq!(sampled(2, 2.0, 30.0, 1000), vec![29, 89]);
    assert_eq!(sampled(2, 2.0, 30000.0 / 1001.0, 1000), vec![29, 89]);
    assert_eq!(sampled(2, 2.0, 24.0, 50), vec![23, 50]);
}

/// Luma as the check reads raea's export: BT.709 weights, full range, cropped to the top-left
#[test]
fn luma_is_bt709_cropped() {
    let px = |r: u8, g: u8, b: u8| [r, g, b, 255];
    let rgba: Vec<u8> = [px(255, 255, 255), px(0, 0, 0), px(255, 0, 0), px(0, 255, 0)].concat();
    assert_eq!(luma(&rgba, 2, 2, 2), vec![255, 0, 54, 182]);
    assert_eq!(luma(&rgba, 2, 1, 2), vec![255, 54]);
}

fn finding(check: &str, frame: Option<i64>, says: &str) -> Finding {
    let f = Finding::new(check, Level::Error, says, "raea check");
    match frame {
        Some(n) => f.frames(n, n + 1),
        None => f,
    }
}

/// A finding is carried when the same rule finds the same thing at the same moment, whatever its numbers say
#[test]
fn a_carried_finding_is_the_same_rule_at_the_same_moment() {
    let fps = 24.0;
    assert!(alike(&finding("blotching", Some(96), "worst 13%"), &finding("blotching", Some(96), "worst 15%"), fps));
    assert!(alike(&finding("clipping", Some(624), "9 runs"), &finding("clipping", Some(640), "6 runs"), fps));
    assert!(!alike(&finding("clipping", Some(624), "9 runs"), &finding("clipping", Some(1104), "11 runs"), fps));
    assert!(!alike(&finding("clipping", Some(624), "x"), &finding("dropout", Some(624), "x"), fps));
    assert!(alike(&finding("brand", None, "a"), &finding("brand", None, "b"), fps));
    assert_eq!(piece("walkaround-v2"), "walkaround");
    assert_eq!(piece("edit-video"), "edit-video");
    assert_eq!(piece("telluride-v12"), "telluride");
}

/// A project whose record holds two renders of one composition and a check of the first: the second's check finds
/// that one, its blotching carried and its clipping new; a take under assets/ is a source, an export isn't
#[test]
fn the_earlier_check_is_of_the_composition_s_other_export() {
    let dir = std::env::temp_dir().join(format!("raea-cause-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in [".raea/record", "out", "assets/footage", "compositions"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    std::fs::write(dir.join(".raea/project"), "[project]\n\tid = pj_test\n").unwrap();
    for f in ["out/a-v1.mp4", "out/a-v2.mp4", "assets/footage/take.mov", "compositions/a.tsx", "compositions/a-v2.tsx"] {
        std::fs::write(dir.join(f), b"").unwrap();
    }
    let entry = |id: &str, by: &str, of: &str, out: Option<&str>, kind: &str, findings: Vec<Finding>| record::Entry { id: id.into(), at: 0, by: by.into(), of: of.into(), out: out.map(Into::into), kind: kind.into(), commit: None, findings, despite: vec![] };
    let entries = [
        entry("20261116-170223-a", "raea render", "compositions/a.tsx", Some("out/a-v1.mp4"), "video", vec![]),
        entry("20261116-170252-a-v1", "raea check", "out/a-v1.mp4", None, "check", vec![finding("blotching", Some(96), "worst 13%")]),
        entry("20261116-171601-a-v2", "raea render", "compositions/a-v2.tsx", Some("out/a-v2.mp4"), "video", vec![]),
    ];
    for e in &entries {
        std::fs::write(dir.join(format!(".raea/record/{}.json", e.id)), serde_json::to_vec(e).unwrap()).unwrap();
    }
    let v2 = dir.join("out/a-v2.mp4");
    assert_eq!(composition_of(&v2).map(|(_, c)| c.ends_with("compositions/a-v2.tsx")), Some(true));
    let before = earlier(&v2).expect("the check of a-v1");
    assert_eq!(before.of, "out/a-v1.mp4");
    let mut now = vec![finding("blotching", Some(96), "worst 15%"), finding("clipping", Some(624), "9 runs")];
    carried(&mut now, &before, 24.0);
    assert!(now[0].says.contains("also in out/a-v1.mp4 (20261116-170252-a-v1)"), "{}", now[0].says);
    assert_eq!(now[1].says, "9 runs");
    // nothing was rendered before a-v1, so its check has no earlier one, even with a-v2's check in the record
    assert!(earlier(&dir.join("out/a-v1.mp4")).is_none());
    assert!(is_source(&dir.join("assets/footage/take.mov")));
    assert!(!is_source(&v2));
    let _ = std::fs::remove_dir_all(&dir);
}
