use blitz_traits::events::{BlitzImeEvent, DomEvent};

use crate::BaseDocument;

pub(crate) fn handle_ime_event<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    event: BlitzImeEvent,
    dispatch_event: F,
) {
    if let Some(node_id) = doc.focus_node_id {
        // raea: a commit is cut to the room maxlength leaves, as Chrome cuts an insertion
        let event = match event {
            BlitzImeEvent::Commit(text) => match doc.nodes[node_id].text_input_room() {
                Some(room) => BlitzImeEvent::Commit(text.chars().take(room).collect()),
                None => BlitzImeEvent::Commit(text),
            },
            e => e,
        };
        let node = &mut doc.nodes[node_id];
        let text_input_data = node
            .data
            .downcast_element_mut()
            .and_then(|el| el.text_input_data_mut());
        if let Some(input_data) = text_input_data {
            // raea: the field before a composition began, so its commit is one step from there (node/undo.rs)
            let commit = matches!(event, BlitzImeEvent::Commit(_));
            if !input_data.editor.is_composing() && input_data.history.composing.is_none() {
                input_data.history.composing = Some(input_data.snap());
            }
            let generated_event = input_data.apply_ime_event(
                &mut doc.font_ctx.lock().unwrap(),
                &mut doc.layout_ctx,
                event,
            );
            if commit || !input_data.editor.is_composing() {
                if let Some(before) = input_data.history.composing.take() {
                    input_data.note_change(before, Some(crate::node::undo::Run::Typing));
                }
            }

            if let Some(generated_event) = generated_event {
                doc.apply_generated_text_input_event(node_id, generated_event, dispatch_event);
            }

            #[cfg(feature = "tracing")]
            tracing::debug!(node_id = ?node_id, "Sent ime event");
        }
    }
}
