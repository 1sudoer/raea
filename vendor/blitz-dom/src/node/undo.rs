//! raea: a text field's history, for Cmd+Z and Cmd+Shift+Z (Ctrl+Z and Ctrl+Y elsewhere). Blitz kept none, so undo in
//! any box of raea's window did nothing (a tester, 11-16). Before each change the field's text and selection are kept;
//! a run of typing, or of deleting, with the caret where the last change left it, is one step, as the Mac's own fields
//! group it, and anything else (a paste, a cut, a new line, the caret moved first) starts the next. A value a script
//! sets is a new start: the history goes, as Chrome drops it.

/// The field as it was: its words and its selection, as byte offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snap {
    pub text: String,
    pub anchor: usize,
    pub focus: usize,
}

/// What a change was, for grouping: a run of typing or of deleting is one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Typing,
    Deleting,
}

/// Steps kept: enough for any box in the app, small enough to never matter.
const KEEP: usize = 200;

#[derive(Default, Debug)]
pub struct History {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    /// the run the last change was in, and where it left the caret
    run: Option<(Run, usize)>,
    /// the field as it was when a composition (an input method's marked text) began: its commit is one step from there
    pub composing: Option<Snap>,
}

impl History {
    /// A change happened: `before` is the field before it, `after` where the caret is now, `run` what kind of change.
    pub fn record(&mut self, before: Snap, after: usize, run: Option<Run>) {
        let joins = matches!((run, self.run), (Some(r), Some((last, at))) if r == last && at == before.focus);
        if !joins {
            self.undo.push(before);
            if self.undo.len() > KEEP {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.run = run.map(|r| (r, after));
    }

    /// Cmd+Z: the field before its last step, given the field now (kept for Cmd+Shift+Z).
    pub fn undo(&mut self, now: Snap) -> Option<Snap> {
        let to = self.undo.pop()?;
        self.redo.push(now);
        self.run = None;
        Some(to)
    }

    /// Cmd+Shift+Z: the step Cmd+Z last took back.
    pub fn redo(&mut self, now: Snap) -> Option<Snap> {
        let to = self.redo.pop()?;
        self.undo.push(now);
        self.run = None;
        Some(to)
    }

    /// A value set by script: nothing before it can be put back.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(text: &str) -> Snap {
        Snap { text: text.into(), anchor: text.len(), focus: text.len() }
    }

    #[test]
    fn a_run_of_typing_is_one_step() {
        let mut h = History::default();
        h.record(snap(""), 1, Some(Run::Typing));
        h.record(snap("h"), 2, Some(Run::Typing));
        h.record(snap("hi"), 3, Some(Run::Typing));
        assert_eq!(h.undo(snap("hi!")), Some(snap("")));
        assert_eq!(h.undo(snap("")), None);
    }

    #[test]
    fn deleting_after_typing_is_its_own_step() {
        let mut h = History::default();
        h.record(snap(""), 1, Some(Run::Typing));
        h.record(snap("a"), 2, Some(Run::Typing));
        h.record(snap("ab"), 1, Some(Run::Deleting));
        assert_eq!(h.undo(snap("a")), Some(snap("ab")));
        assert_eq!(h.undo(snap("ab")), Some(snap("")));
    }

    #[test]
    fn typing_somewhere_else_starts_a_new_step() {
        let mut h = History::default();
        h.record(snap(""), 1, Some(Run::Typing));
        // the caret was moved to 0 before the next letter
        h.record(Snap { text: "a".into(), anchor: 0, focus: 0 }, 1, Some(Run::Typing));
        assert_eq!(h.undo(snap("ba")).map(|s| s.text), Some("a".into()));
    }

    #[test]
    fn a_paste_is_a_step_of_its_own() {
        let mut h = History::default();
        h.record(snap(""), 1, Some(Run::Typing));
        h.record(snap("a"), 6, None);
        h.record(snap("ahello"), 7, Some(Run::Typing));
        assert_eq!(h.undo(snap("ahello!")).map(|s| s.text), Some("ahello".into()));
        assert_eq!(h.undo(snap("ahello")).map(|s| s.text), Some("a".into()));
    }

    #[test]
    fn redo_puts_back_what_undo_took_and_a_new_change_drops_it() {
        let mut h = History::default();
        h.record(snap(""), 5, None);
        assert_eq!(h.undo(snap("hello")), Some(snap("")));
        assert_eq!(h.redo(snap("")), Some(snap("hello")));
        assert_eq!(h.undo(snap("hello")), Some(snap("")));
        h.record(snap(""), 1, Some(Run::Typing));
        assert_eq!(h.redo(snap("x")), None);
    }

    #[test]
    fn a_script_value_clears_it() {
        let mut h = History::default();
        h.record(snap(""), 1, Some(Run::Typing));
        h.clear();
        assert_eq!(h.undo(snap("a")), None);
    }
}
