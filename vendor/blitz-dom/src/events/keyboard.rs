use crate::events::focus::generate_focus_events;
use crate::{BaseDocument, node::GeneratedTextInputEvent, util::ACTION_MOD};
use blitz_traits::node_id::NodeId;
use blitz_traits::{
    SmolStr,
    events::{BlitzInputEvent, BlitzKeyEvent, DomEvent, DomEventData},
};
use keyboard_types::{Key, Modifiers, NamedKey};
use markup5ever::local_name;

pub(super) enum KeyboardOrTextInputEvent {
    KeyPress(BlitzKeyEvent),
    AppleStandardKeyBinding(SmolStr),
}

pub(crate) fn handle_key_or_input_event<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: NodeId,
    event: KeyboardOrTextInputEvent,
    mut dispatch_event: F,
) {
    if let KeyboardOrTextInputEvent::KeyPress(event) = &event {
        if event.key == Key::Named(NamedKey::Tab) {
            let backwards = event.modifiers.contains(Modifiers::SHIFT);
            let old_focus = doc.focus_node_id;
            generate_focus_events(
                doc,
                &mut |doc| {
                    if backwards {
                        doc.focus_prev_node();
                    } else {
                        doc.focus_next_node();
                    }
                },
                &mut dispatch_event,
            );
            if doc.focus_node_id != old_focus {
                doc.shell_provider.request_redraw();
            }
            return;
        }

        // Handle copy (Ctrl+C/Cmd+C) for text selection when no text input is focused
        if event.state.is_pressed() {
            let action_mod = event.modifiers.contains(ACTION_MOD);
            if action_mod {
                if let Key::Character(c) = &event.key {
                    if c.to_lowercase() == "c" {
                        // Check if we have a text selection (and no focused text input)
                        let has_focused_text_input = doc.focus_node_id.is_some_and(|id| {
                            doc.get_node(id)
                                .and_then(|n| n.element_data())
                                .is_some_and(|e| e.text_input_data().is_some())
                        });

                        if !has_focused_text_input {
                            if let Some(text) = doc.get_selected_text() {
                                let _ = doc.shell_provider.set_clipboard_text(text);
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(node_id) = doc.focus_node_id {
        if target != node_id {
            return;
        }
        // raea: a typed character a full field (its maxlength) has no room for isn't inserted
        if let KeyboardOrTextInputEvent::KeyPress(e) = &event
            && let Key::Character(_) = &e.key
            && !e.modifiers.contains(ACTION_MOD)
            && doc.nodes[node_id].text_input_room() == Some(0)
        {
            return;
        }

        let node = &mut doc.nodes[node_id];
        let Some(element_data) = node.element_data_mut() else {
            return;
        };

        if let Some(input_data) = element_data.text_input_data_mut() {
            // raea: Cmd+Z and Cmd+Shift+Z (Ctrl+Z and Ctrl+Y elsewhere) step the field's history (node/undo.rs)
            if let Some(redo) = history_step(&event) {
                let generated =
                    input_data.step_history(&mut doc.font_ctx.lock().unwrap(), &mut doc.layout_ctx, redo);
                if let Some(generated) = generated {
                    doc.apply_generated_text_input_event(node_id, generated, dispatch_event);
                }
                return;
            }
            let before = input_data.snap();
            let run = run_of(&event);
            let generated_event = match event {
                KeyboardOrTextInputEvent::KeyPress(blitz_key_event) => input_data
                    .apply_keypress_event(
                        &mut doc.font_ctx.lock().unwrap(),
                        &mut doc.layout_ctx,
                        &*doc.shell_provider,
                        blitz_key_event,
                    ),
                KeyboardOrTextInputEvent::AppleStandardKeyBinding(command) => input_data
                    .apply_apple_standard_keybinding(
                        &mut doc.font_ctx.lock().unwrap(),
                        &mut doc.layout_ctx,
                        &*doc.shell_provider,
                        &command,
                    ),
            };
            input_data.note_change(before, run);

            if let Some(generated_event) = generated_event {
                doc.apply_generated_text_input_event(node_id, generated_event, dispatch_event);
            }
        }
    }
}

/// raea: Some(redo?) for the keys that step a field's history: Cmd+Z, Cmd+Shift+Z and Cmd+Y (Ctrl elsewhere), and
/// the Mac's undo: and redo: commands
fn history_step(event: &KeyboardOrTextInputEvent) -> Option<bool> {
    match event {
        KeyboardOrTextInputEvent::KeyPress(e) if e.state.is_pressed() && e.modifiers.contains(ACTION_MOD) => match &e.key {
            Key::Character(c) if c.eq_ignore_ascii_case("z") => Some(e.modifiers.contains(Modifiers::SHIFT)),
            Key::Character(c) if c.eq_ignore_ascii_case("y") => Some(true),
            _ => None,
        },
        KeyboardOrTextInputEvent::AppleStandardKeyBinding(c) if c.as_str() == "undo:" => Some(false),
        KeyboardOrTextInputEvent::AppleStandardKeyBinding(c) if c.as_str() == "redo:" => Some(true),
        _ => None,
    }
}

/// raea: what a key does to the words, for grouping steps: typing or deleting runs join; anything else stands alone
fn run_of(event: &KeyboardOrTextInputEvent) -> Option<crate::node::undo::Run> {
    use crate::node::undo::Run;
    match event {
        KeyboardOrTextInputEvent::KeyPress(e) if !e.modifiers.contains(ACTION_MOD) => match &e.key {
            Key::Character(c) if c.as_str() != "\n" => Some(Run::Typing),
            Key::Named(NamedKey::Backspace | NamedKey::Delete) => Some(Run::Deleting),
            _ => None,
        },
        KeyboardOrTextInputEvent::AppleStandardKeyBinding(c) if c.as_str().starts_with("delete") => Some(Run::Deleting),
        _ => None,
    }
}

impl BaseDocument {
    pub(crate) fn apply_generated_text_input_event<F: FnMut(DomEvent)>(
        &mut self,
        node_id: NodeId,
        event: GeneratedTextInputEvent,
        mut dispatch_event: F,
    ) {
        let node = &mut self.nodes[node_id];
        let element_data = node
            .element_data_mut()
            .expect("apply_generated_text_input_event called on a node that is not an element");
        let input_data = element_data
            .text_input_data_mut()
            .expect("apply_generated_text_input_event called on a node that is not a text input");

        match event {
            GeneratedTextInputEvent::Input => {
                input_data.sync_mask(&mut self.font_ctx.lock().unwrap(), &mut self.layout_ctx);
                let value = input_data.editor.raw_text().to_string();
                dispatch_event(DomEvent::new(
                    node_id,
                    DomEventData::Input(BlitzInputEvent { value }),
                ));
                self.shell_provider.request_redraw();
            }
            GeneratedTextInputEvent::Select | GeneratedTextInputEvent::PreEditChange => {
                self.shell_provider.request_redraw();
            }
            GeneratedTextInputEvent::Submit => {
                // TODO: Generate submit event that can be handled by script
                implicit_form_submission(self, node_id);
            }
        }
    }
}

/// https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#field-that-blocks-implicit-submission
fn implicit_form_submission(doc: &BaseDocument, text_target: NodeId) {
    let Some(form_owner_id) = doc.controls_to_form.get(&text_target) else {
        return;
    };
    if doc
        .controls_to_form
        .iter()
        .filter(|(_control_id, form_id)| *form_id == form_owner_id)
        .filter_map(|(control_id, _)| doc.nodes[*control_id].element_data())
        .filter(|element_data| {
            element_data.attr(local_name!("type")).is_some_and(|t| {
                matches!(
                    t,
                    "text"
                        | "search"
                        | "email"
                        | "url"
                        | "tel"
                        | "password"
                        | "date"
                        | "month"
                        | "week"
                        | "time"
                        | "datetime-local"
                        | "number"
                )
            })
        })
        .count()
        > 1
    {
        return;
    }

    doc.submit_form(*form_owner_id, *form_owner_id);
}
