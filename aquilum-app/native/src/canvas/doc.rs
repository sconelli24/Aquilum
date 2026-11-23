use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use aquilum_core::Core;
use aquilum_core::canvas::{Canvas, NodeKind};
use aquilum_core::files::error::FileCommandError;

use crate::note::Note;

const UNDO_DEPTH: usize = 100;

/// The board model on top of a [`Note`] that owns the file: the note's debounced saving,
/// conflict copies and reloads work on the JSON text unchanged.
pub struct CanvasDoc {
    note: Note,
    canvas: Canvas,
    json: String,
    /// Set when the file on disk is not valid JSON Canvas; the board is then read-only so a
    /// save never replaces a file we could not read.
    pub broken: Option<String>,
    undo: Vec<Canvas>,
    redo: Vec<Canvas>,
}

/// Byte range that differs between two texts and the length of its replacement.
fn changed_region(old: &str, new: &str) -> (std::ops::Range<usize>, usize) {
    let prefix = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old.bytes().rev().zip(new.bytes().rev()).take(room).take_while(|(a, b)| a == b).count();
    (prefix..old.len() - suffix, new.len() - suffix - prefix)
}

impl CanvasDoc {
    pub fn open(path: PathBuf) -> Result<CanvasDoc, FileCommandError> {
        let (note, rope) = Note::open(path)?;
        let json = rope.to_string();
        let mut doc = CanvasDoc { note, canvas: Canvas::default(), json: String::new(), broken: None, undo: Vec::new(), redo: Vec::new() };
        doc.adopt(json);
        Ok(doc)
    }

    fn adopt(&mut self, json: String) {
        match Canvas::parse(&json) {
            Ok(canvas) => {
                self.canvas = canvas;
                self.broken = None;
            }
            Err(error) => self.broken = Some(error.to_string()),
        }
        self.json = json;
    }

    pub fn moved(&mut self, to: &Path) {
        self.note.path = to.to_path_buf();
    }

    pub fn path(&self) -> &Path {
        &self.note.path
    }

    pub fn is(&self, path: &Path) -> bool {
        self.note.is(path)
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn save_error(&self) -> Option<&str> {
        self.note.save_error.as_deref()
    }

    fn write(&mut self) {
        if self.broken.is_some() {
            return;
        }
        let json = self.canvas.to_json();
        if json == self.json {
            return;
        }
        let (range, inserted) = changed_region(&self.json, &json);
        self.note.touch(range, inserted);
        self.note.edit();
        self.json = json;
    }

    /// Replaces the board with an edited copy, as one undo step.
    pub fn commit(&mut self, canvas: Canvas) -> bool {
        if self.broken.is_some() || canvas == self.canvas {
            return false;
        }
        let before = std::mem::replace(&mut self.canvas, canvas);
        self.undo.push(before);
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.write();
        true
    }

    /// Text typed into a text card. Typing has its own undo in the editor, so this is no step.
    pub fn set_text(&mut self, id: &str, text: &str) -> bool {
        let Some(node) = self.canvas.node_mut(id).filter(|n| n.kind() == NodeKind::Text) else { return false };
        if node.text.as_deref() == Some(text) {
            return false;
        }
        node.text = Some(text.to_owned());
        self.write();
        true
    }

    /// Steps back (or forward) through structural edits. Text cards keep the text they have
    /// now: their typing is undone in the editor, not here.
    pub fn undo(&mut self, redo: bool) -> bool {
        let (from, to) = if redo { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        let Some(mut target) = from.pop() else { return false };
        for node in &mut target.nodes {
            if node.kind() == NodeKind::Text
                && let Some(current) = self.canvas.node(&node.id)
            {
                node.text = current.text.clone();
            }
        }
        to.push(std::mem::replace(&mut self.canvas, target));
        self.write();
        true
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.note.deadline()
    }

    /// Saves when due. Returns the board as it is on disk when a conflict replaced ours.
    pub fn tick(&mut self, core: &Arc<Core>, now: Instant) -> Option<&Canvas> {
        let rope = crop::Rope::from(self.json.as_str());
        self.note.tick(core, &rope, now);
        let replaced = self.note.take_replaced()?;
        self.adopt(replaced.to_string());
        Some(&self.canvas)
    }

    pub fn commit_pending(&mut self, core: &Arc<Core>) {
        self.note.commit(core, &crop::Rope::from(self.json.as_str()));
    }

    pub fn flush(&mut self, core: &Arc<Core>) {
        self.note.flush(core, &crop::Rope::from(self.json.as_str()));
    }

    /// Picks up a change made outside the app. `None` when nothing changed or ours is unsaved.
    pub fn reload(&mut self) -> Option<&Canvas> {
        let rope = self.note.reload()?;
        self.adopt(rope.to_string());
        Some(&self.canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aquilum_core::canvas::Node;

    #[test]
    fn changed_region_spans_only_the_difference() {
        assert_eq!(changed_region("abcdef", "abXYef"), (2..4, 2));
        assert_eq!(changed_region("abc", "abc"), (3..3, 0));
        assert_eq!(changed_region("abc", "abcd"), (3..3, 1));
        assert_eq!(changed_region("aaaa", "aa"), (2..4, 0));
        assert_eq!(changed_region("", "xyz"), (0..0, 3));
    }

    #[test]
    fn undo_restores_structure_but_keeps_typed_text() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Board.canvas");
        std::fs::write(&path, aquilum_core::canvas::EMPTY).unwrap();
        let mut doc = CanvasDoc::open(path).unwrap();
        let mut next = doc.canvas().clone();
        next.nodes.push(Node::text("one", 0.0, 0.0, 250.0, 60.0));
        let id = next.nodes[0].id.clone();
        assert!(doc.commit(next));
        let mut moved = doc.canvas().clone();
        moved.nodes[0].x = 100.0;
        assert!(doc.commit(moved));
        assert!(doc.set_text(&id, "two"));
        assert!(doc.undo(false));
        assert_eq!(doc.canvas().nodes[0].x, 0.0);
        assert_eq!(doc.canvas().nodes[0].text.as_deref(), Some("two"));
        assert!(doc.undo(true));
        assert_eq!(doc.canvas().nodes[0].x, 100.0);
        assert!(doc.undo(false) && doc.undo(false));
        assert!(doc.canvas().nodes.is_empty());
        assert!(!doc.undo(false));
    }

    #[test]
    fn a_broken_file_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Bad.canvas");
        std::fs::write(&path, "{not json").unwrap();
        let mut doc = CanvasDoc::open(path).unwrap();
        assert!(doc.broken.is_some());
        let mut next = doc.canvas().clone();
        next.nodes.push(Node::text("x", 0.0, 0.0, 10.0, 10.0));
        assert!(!doc.commit(next));
    }
}
