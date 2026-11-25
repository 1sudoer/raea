//! raea gen's picture steps on a host without a made.app key: fal, then OpenRouter (route.rs Keys).
use serde_json::json;

use crate::flow::Step;
use crate::route::{Backend, Keys, Policy, route};

fn on(made: bool, fal: bool, openrouter: bool) -> Policy {
    Policy { keys: Keys { made, fal, openrouter }, ..Default::default() }
}

#[test]
fn pictures_go_to_fal_then_openrouter_without_a_made_key() {
    let make = Step::new("image.generate", json!({"prompt": "a lighthouse in fog"}));
    let edit = Step::new("image.edit", json!({"prompt": "make it night", "image": "https://example.com/a.png"}));
    // made's key here: made, as before (and with no keys known at all, the default)
    assert!(matches!(route(&make, &on(true, true, true)).unwrap().0, Backend::Made(_)));
    assert!(matches!(route(&make, &Policy::default()).unwrap().0, Backend::Made(_)));
    // no made key: fal's nano-banana-2, its edit endpoint for an edit, said in the note
    let (b, note) = route(&make, &on(false, true, true)).unwrap();
    assert_eq!(b, Backend::Fal("fal-ai/nano-banana-2".into()));
    assert!(note.unwrap().contains("no made.app key"));
    assert_eq!(route(&edit, &on(false, true, false)).unwrap().0, Backend::Fal("fal-ai/nano-banana-2/edit".into()));
    // no made or fal key: OpenRouter's Gemini image model, for both
    assert_eq!(route(&make, &on(false, false, true)).unwrap().0, Backend::OpenRouter("google/gemini-3.1-flash-image".into()));
    assert_eq!(route(&edit, &on(false, false, true)).unwrap().0, Backend::OpenRouter("google/gemini-3.1-flash-image".into()));
    // none of the three: made, which then asks for its key when it runs
    assert!(matches!(route(&make, &on(false, false, false)).unwrap().0, Backend::Made(_)));
}

#[test]
fn only_picture_steps_and_unnamed_models_fall_back() {
    let none = on(false, true, true);
    // a video step stays on made (fal and OpenRouter take its key only when the step names them)
    let video = Step::new("video.generate", json!({"prompt": "waves"}));
    assert!(matches!(route(&video, &none).unwrap().0, Backend::Made(_)));
    // a step that names its model goes there whatever keys are here
    let mut named = Step::new("image.generate", json!({"prompt": "x"}));
    named.model = Some("made:nano_banana_2".into());
    assert_eq!(route(&named, &none).unwrap().0, Backend::Made("nano_banana_2".into()));
}
