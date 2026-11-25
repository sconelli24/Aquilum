//! What the editor does for Cornell notes: Enter at the end of a cue goes on to its notes, and a new row can be added.

use super::*;
use crate::ui::editor::document::Lane;

impl TextEditor {
    /// Enter at the end of a cue goes on to the notes of its row, so the question is one line unless Shift+Enter is used. Returns
    /// whether the key was taken.
    pub(super) fn cornell_enter(&mut self) -> bool {
        let range = self.selection();
        if !range.is_empty() || self.single_line || !self.doc.cornell_note() {
            return false;
        }
        let i = self.doc.at_offset(range.start);
        let p = self.doc.paragraph(i);
        if range.start != p.end() || self.doc.cornell_kind(i) != Some(Lane::Cue) {
            return false;
        }
        let count = self.doc.line_count();
        // The cue goes on while the next line has text.
        if i + 1 < count && !self.doc.line(i + 1).trim().is_empty() {
            return false;
        }
        // The notes start after one blank line: go there, or make room if the row has none yet.
        if i + 2 < count && self.doc.cornell_kind(i + 2).is_none() {
            let notes = self.doc.paragraph(i + 2).end();
            self.move_to((notes, Affinity::Downstream), false);
            return true;
        }
        self.replace(range.start..range.start, "\n\n", Kind::Other);
        true
    }

    /// A new row: a cue with room for its notes, before the summary if there is one and at the end otherwise. The caret is left in the cue.
    pub fn insert_cornell_row(this: &mut WidgetMut<'_, Self>) {
        let widget = &mut *this.widget;
        if !widget.doc.cornell_note() {
            return;
        }
        let at = (0..widget.doc.line_count()).find(|&i| widget.doc.cornell_kind(i) == Some(Lane::Summary)).map_or_else(|| widget.doc.len(), |i| widget.doc.paragraph(i).start);
        let before = widget.doc.slice(at.saturating_sub(2)..at);
        let lead = if at == 0 || before.ends_with("\n\n") { "" } else if before.ends_with('\n') { "\n" } else { "\n\n" };
        let row = format!("{lead}::question \n\n\n\n");
        let caret = at + lead.len() + "::question ".len();
        widget.dispatch(vec![(at..at, row)], Some((caret, caret)), Kind::Other);
        widget.follow_caret = true;
        Self::edited(this);
    }
}
