use std::path::{Path, PathBuf};
use std::sync::Arc;

use aquilum_core::canvas::{Canvas, Node, NodeKind};
use masonry::app::RenderRoot;
use masonry::core::{NewWidget, Widget, WidgetId};

use super::{Bound, CardNote};
use crate::app::App;
use crate::app::embeds::Embeds;
use crate::canvas::geometry;
use crate::note::Note;
use crate::ui::canvas::Card;
use crate::ui::editor::{Edit, Smart, TextEditor};
use crate::ui::handle::Handle;
use crate::ui::scroll::ScrollArea;
use crate::ui::style;

fn is_image(file: &str) -> bool {
    matches!(crate::images::kind(file), Some(crate::images::MediaKind::Image))
}

impl App {
    fn smart(&self) -> Smart {
        Smart::from_config(&self.workspace.core.settings.get_config())
    }

    fn resolver_for(embeds: &Option<Embeds>) -> Option<Arc<dyn crate::ui::editor::resolve::Resolver>> {
        embeds.clone().map(|e| Arc::new(e) as Arc<dyn crate::ui::editor::resolve::Resolver>)
    }

    fn scrolled(editor: NewWidget<TextEditor>) -> NewWidget<dyn Widget> {
        NewWidget::new(ScrollArea::new(editor).fill(true)).erased()
    }

    /// The live card of a node, or `None` for nodes the board draws itself (links, groups,
    /// files that are neither notes nor images).
    pub(super) fn card_for(&mut self, node: &Node) -> Option<(String, NewWidget<Card>)> {
        let color = geometry::color(node.color.as_deref());
        let content = match node.kind() {
            NodeKind::Text => {
                let editor = TextEditor::new(node.text.as_deref().unwrap_or_default())
                    .styled(style::editor_text)
                    .markdown()
                    .with_resolver(Self::resolver_for(&self.canvas.embeds))
                    .with_wake(Arc::clone(&self.workspace.wake))
                    .with_smart(self.smart());
                let editor = NewWidget::new(editor).with_class(style::EDITOR_TEXT);
                self.canvas.editors.insert(editor.id(), Bound::Text(node.id.clone()));
                Self::scrolled(editor)
            }
            NodeKind::File => {
                let file = node.file.as_deref()?;
                let root = self.tree.root()?.to_path_buf();
                let path = crate::paths::absolute(&root, file);
                if aquilum_core::search::paths::is_note(&path) {
                    let editor = self.note_card_editor(&path)?;
                    Self::scrolled(editor)
                } else if is_image(file) {
                    let editor = TextEditor::new(&format!("![[{file}]]")).styled(style::editor_text).markdown().with_resolver(Self::resolver_for(&self.canvas.embeds));
                    NewWidget::new(editor).with_class(style::EDITOR_TEXT).erased()
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some((node.id.clone(), Card::new(Some(content), color)))
    }

    fn note_card_editor(&mut self, path: &Path) -> Option<NewWidget<TextEditor>> {
        if !self.canvas.notes.contains_key(path) {
            let (note, text) = match Note::open(path.to_path_buf()) {
                Ok(opened) => opened,
                Err(error) => {
                    report!("canvas card {} not read: {error}", path.display());
                    return None;
                }
            };
            let hidden = crate::frontmatter::bounds_of(&crate::rope::Text(&text)).map_or(0, |found| found.body);
            let embeds = self.tree.root().map(|root| Embeds::new(Arc::clone(&self.workspace.core), root.to_path_buf(), path.to_path_buf(), self.workspace.data_dir.join("remote"), Arc::clone(&self.workspace.wake)));
            self.canvas.notes.insert(path.to_path_buf(), CardNote { note, text, hidden, editors: Vec::new(), embeds });
        }
        let smart = self.smart();
        let wake = Arc::clone(&self.workspace.wake);
        let card = self.canvas.notes.get_mut(path)?;
        let mut body = card.text.clone();
        body.delete(..card.hidden.min(body.byte_len()));
        let editor = TextEditor::entire(body)
            .styled(style::editor_text)
            .markdown()
            .report_edits()
            .with_resolver(Self::resolver_for(&card.embeds))
            .with_wake(wake)
            .with_smart(smart);
        let editor = NewWidget::new(editor).with_class(style::EDITOR_TEXT);
        card.editors.push(Handle::of(&editor));
        self.canvas.editors.insert(editor.id(), Bound::Note(path.to_path_buf()));
        Some(editor)
    }

    /// Typing in a note card: track the change for history, keep the text, save soon, and
    /// mirror it to other cards of the same note.
    pub(super) fn note_card_edited(&mut self, root: &mut RenderRoot, editor: WidgetId, path: &PathBuf, edits: &[Edit]) {
        let Some(rope) = root.get_widget(editor).and_then(|w| w.downcast::<TextEditor>().map(|e| e.inner().rope())) else { return };
        let Some(card) = self.canvas.notes.get_mut(path) else { return };
        for edit in edits {
            card.note.touch(card.hidden + edit.range.start..card.hidden + edit.range.end, edit.inserted);
        }
        let mut text = rope;
        if card.hidden > 0 {
            text.insert(0, card.text.byte_slice(..crate::rope::floor(&card.text, card.hidden)).to_string());
        }
        card.text = text;
        card.note.edit();
        self.reset_card_note(root, path, Some(editor));
    }

    /// Shows a note's current text in its card editors, except `keep` (the one typed into).
    pub(super) fn reset_card_note(&mut self, root: &mut RenderRoot, path: &Path, keep: Option<WidgetId>) {
        let Some(card) = self.canvas.notes.get(path) else { return };
        let mut body = card.text.clone();
        body.delete(..card.hidden.min(body.byte_len()));
        for editor in card.editors.iter().filter(|h| Some(h.id()) != keep) {
            let body = body.clone();
            editor.edit(root, |mut e| TextEditor::reset_rope(&mut e, body));
        }
    }

    /// Saves and forgets notes no card shows any more.
    pub(super) fn drop_unused_notes(&mut self, canvas: &Canvas) {
        let Some(root) = self.tree.root().map(Path::to_path_buf) else { return };
        let used: Vec<PathBuf> = canvas.files().map(|f| crate::paths::absolute(&root, f)).collect();
        let unused: Vec<PathBuf> = self.canvas.notes.keys().filter(|p| !used.iter().any(|u| aquilum_core::search::paths::same_path(u, p))).cloned().collect();
        let core = Arc::clone(&self.workspace.core);
        for path in unused {
            if let Some(mut card) = self.canvas.notes.remove(&path) {
                card.note.flush(&core, &card.text);
                let ids: Vec<WidgetId> = card.editors.iter().map(|h| h.id()).collect();
                self.canvas.editors.retain(|id, _| !ids.contains(id));
            }
        }
    }
}
