//! The page of a `.canvas` tab: owns the open board, the notes shown on its cards and their
//! editors, and keeps them saved.

mod actions;
mod cards;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use aquilum_core::canvas::Canvas;
use masonry::app::RenderRoot;
use masonry::core::{NewWidget, Widget, WidgetId};

use super::App;
use super::embeds::Embeds;
use crate::canvas::CanvasDoc;
use crate::i18n::t_with;
use crate::note::Note;
use crate::ui::canvas::{CanvasView, Prompt};
use crate::ui::editor::TextEditor;
use crate::ui::handle::Handle;
use crate::ui::widgets::Slot;

pub(in crate::app) use actions::CanvasCommand;

/// What a card editor writes to.
#[derive(Clone, Debug, PartialEq)]
enum Bound {
    /// The text of a text card, by node id.
    Text(String),
    /// The body of a note shown on a file card.
    Note(PathBuf),
}

/// A note shown on one or more file cards.
struct CardNote {
    note: Note,
    text: crop::Rope,
    /// Bytes of front matter kept out of the card.
    hidden: usize,
    editors: Vec<Handle<TextEditor>>,
    embeds: Option<Embeds>,
}

#[derive(Default)]
pub(in crate::app) struct CanvasState {
    doc: Option<CanvasDoc>,
    view: Option<Handle<CanvasView>>,
    editors: HashMap<WidgetId, Bound>,
    notes: HashMap<PathBuf, CardNote>,
    embeds: Option<Embeds>,
    prompt: Option<(Handle<TextEditor>, Prompt)>,
    /// Board point and targets of the open context menu.
    menu: Option<(masonry::kurbo::Point, Option<String>, Option<String>)>,
    error: Option<(PathBuf, String)>,
}

impl App {
    pub(in crate::app) fn canvas_shows(&self, path: &Path) -> bool {
        self.canvas.doc.as_ref().is_some_and(|doc| doc.is(path)) || self.canvas.error.as_ref().is_some_and(|(p, _)| p == path)
    }

    pub fn canvas_shown(&self) -> bool {
        self.canvas.view.is_some() && self.popup.is_none() && self.tabs.active().path.as_deref().is_some_and(|p| self.canvas_shows(p))
    }

    pub(in crate::app) fn canvas_title(&self) -> Option<String> {
        self.canvas.doc.as_ref().map(|doc| doc.path().file_stem().unwrap_or_default().to_string_lossy().into_owned())
    }

    fn canvas_key(&self, path: &Path) -> Option<(uuid::Uuid, String)> {
        let root = self.tree.root()?;
        let relative = crate::paths::relative(root, path)?;
        let ui = &self.workspace.core.ui_state;
        let workspace = ui.resolve_workspace(&root.to_string_lossy(), crate::dates::now_ms()).ok()?;
        Some((workspace, relative))
    }

    /// Shows the board of `path`, opening it unless it is already open.
    pub(in crate::app) fn show_canvas(&mut self, root: &mut RenderRoot, path: &Path) {
        if self.canvas_shows(path) && self.canvas.view.is_some() {
            return;
        }
        self.leave_canvas(root);
        let widget = match CanvasDoc::open(path.to_path_buf()) {
            Ok(doc) => {
                self.canvas.doc = Some(doc);
                self.canvas_widget()
            }
            Err(error) => {
                report!("canvas {} not read: {error}", path.display());
                self.canvas.error = Some((path.to_path_buf(), error.to_string()));
                let view = CanvasView::new(Canvas::default(), Vec::new(), None, Some(t_with("canvas.notRead", &[("reason", &error.to_string())])));
                self.canvas.view = Some(Handle::of(&view));
                view.erased()
            }
        };
        if let Some(chrome) = &self.chrome {
            chrome.canvas_page.edit(root, |mut slot| Slot::set_child(&mut slot, widget));
        }
        if let Some(view) = self.canvas.view {
            root.focus_on(Some(view.id()));
        }
    }

    fn canvas_widget(&mut self) -> NewWidget<dyn Widget> {
        let (canvas, path, broken) = match &self.canvas.doc {
            Some(doc) => (doc.canvas().clone(), doc.path().to_path_buf(), doc.broken.clone()),
            None => (Canvas::default(), PathBuf::new(), None),
        };
        if let Some(root) = self.tree.root().map(Path::to_path_buf) {
            self.canvas.embeds = Some(Embeds::new(Arc::clone(&self.workspace.core), root, path.clone(), self.workspace.data_dir.join("remote"), Arc::clone(&self.workspace.wake)));
        }
        let cards = canvas.nodes.iter().filter_map(|node| self.card_for(node)).collect();
        let camera = self
            .canvas_key(&path)
            .and_then(|(workspace, file)| self.workspace.core.ui_state.load_canvas_view(workspace, &file).ok().flatten())
            .map(|v| (v.center_x, v.center_y, v.scale));
        let notice = broken.map(|reason| t_with("canvas.broken", &[("reason", &reason)]));
        let view = CanvasView::new(canvas, cards, camera, notice);
        self.canvas.view = Some(Handle::of(&view));
        view.erased()
    }

    /// Creates a canvas in `folder` (the notes folder for the vault root), opens it and starts
    /// renaming it in the tree.
    pub(in crate::app) fn create_canvas_in(&mut self, root: &mut RenderRoot, folder: &Path) -> bool {
        let folder = match self.tree.root() {
            Some(vault) if vault == folder => crate::vault_files::notes_dir(&self.workspace.core, vault),
            _ => folder.to_path_buf(),
        };
        match crate::file_ops::create_canvas(&self.workspace.core, &folder) {
            Ok(path) => {
                self.tree.paths_changed(root, std::slice::from_ref(&path));
                self.tree.reveal(root, &path);
                let opened = self.open(root, path.clone());
                self.tree.start_rename(root, path);
                opened
            }
            Err(error) => {
                report!("canvas not created: {error}");
                false
            }
        }
    }

    /// Keeps saving to the right files after a rename or a move.
    pub(in crate::app) fn canvas_moved(&mut self, from: &Path, to: &Path) {
        if let Some(doc) = self.canvas.doc.as_mut().filter(|d| d.is(from)) {
            doc.moved(to);
        }
        let moved: Vec<PathBuf> = self.canvas.notes.keys().filter(|p| aquilum_core::search::paths::same_path(p, from)).cloned().collect();
        for path in moved {
            if let Some(mut card) = self.canvas.notes.remove(&path) {
                card.note.path = to.to_path_buf();
                self.canvas.notes.insert(to.to_path_buf(), card);
            }
        }
    }

    /// Saves everything and drops the board.
    pub(in crate::app) fn leave_canvas(&mut self, root: &mut RenderRoot) {
        if self.canvas.view.is_none() && self.canvas.doc.is_none() && self.canvas.error.is_none() {
            return;
        }
        self.flush_canvas();
        self.canvas = CanvasState::default();
        if let Some(chrome) = &self.chrome {
            chrome.canvas_page.edit(root, |mut slot| Slot::clear(&mut slot));
        }
    }

    /// Saves pending edits; files deleted meanwhile stay deleted.
    pub(in crate::app) fn flush_canvas(&mut self) {
        let core = Arc::clone(&self.workspace.core);
        if let Some(doc) = self.canvas.doc.as_mut().filter(|d| d.path().exists()) {
            doc.flush(&core);
        }
        for card in self.canvas.notes.values_mut().filter(|c| c.note.path.exists()) {
            card.note.flush(&core, &card.text);
        }
    }

    pub(in crate::app) fn commit_canvas(&mut self) {
        let core = Arc::clone(&self.workspace.core);
        if let Some(doc) = &mut self.canvas.doc {
            doc.commit_pending(&core);
        }
        for card in self.canvas.notes.values_mut() {
            card.note.commit(&core, &card.text);
        }
    }

    pub(in crate::app) fn canvas_deadline(&self) -> Option<Instant> {
        let doc = self.canvas.doc.as_ref().and_then(CanvasDoc::deadline);
        self.canvas.notes.values().filter_map(|c| c.note.deadline()).chain(doc).min()
    }

    pub(in crate::app) fn canvas_tick(&mut self, root: &mut RenderRoot, now: Instant) {
        let core = Arc::clone(&self.workspace.core);
        let before = self.canvas.doc.as_ref().and_then(|d| d.save_error().map(str::to_owned));
        let replaced = self.canvas.doc.as_mut().and_then(|doc| doc.tick(&core, now).cloned());
        if let Some(canvas) = replaced {
            self.canvas_replaced(root, canvas);
        }
        let after = self.canvas.doc.as_ref().and_then(|d| d.save_error().map(str::to_owned));
        if after != before
            && let (Some(message), Some(view)) = (after, self.canvas.view)
        {
            view.edit(root, |mut v| CanvasView::flash(&mut v, message));
        }
        let mut reset = Vec::new();
        for (path, card) in &mut self.canvas.notes {
            card.note.tick(&core, &card.text, now);
            if let Some(text) = card.note.take_replaced() {
                card.text = text;
                reset.push(path.clone());
            }
        }
        for path in reset {
            self.reset_card_note(root, &path, None);
        }
    }

    /// The board changed under us (undo, a reload, a conflict): show it as it is now.
    fn canvas_replaced(&mut self, root: &mut RenderRoot, canvas: Canvas) {
        let Some(view) = self.canvas.view else { return };
        let notice = self.canvas.doc.as_ref().and_then(|d| d.broken.clone()).map(|reason| t_with("canvas.broken", &[("reason", &reason)]));
        view.edit(root, |mut v| {
            CanvasView::set_canvas(&mut v, canvas.clone());
            CanvasView::set_notice(&mut v, notice);
        });
        self.sync_canvas_cards(root, &canvas, true);
    }

    /// Records an edit made on the board and builds cards for nodes that have none yet.
    pub(in crate::app) fn commit_canvas_edit(&mut self, root: &mut RenderRoot, canvas: Canvas) {
        let Some(doc) = &mut self.canvas.doc else { return };
        if !doc.commit(canvas.clone()) {
            return;
        }
        self.sync_canvas_cards(root, &canvas, false);
    }

    /// Text cards are rebuilt when `refresh` is set, so a restored board shows restored text.
    fn sync_canvas_cards(&mut self, root: &mut RenderRoot, canvas: &Canvas, refresh: bool) {
        let Some(view) = self.canvas.view else { return };
        let existing: Vec<String> = view.get(root).map(|v| v.inner().card_ids().map(str::to_owned).collect()).unwrap_or_default();
        let gone: Vec<WidgetId> = self
            .canvas
            .editors
            .iter()
            .filter(|(_, bound)| match bound {
                Bound::Text(id) => canvas.node(id).is_none() || refresh,
                Bound::Note(_) => false,
            })
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.canvas.editors.remove(&id);
        }
        let added: Vec<_> = canvas
            .nodes
            .iter()
            .filter(|n| !existing.contains(&n.id) || (refresh && n.kind() == aquilum_core::canvas::NodeKind::Text))
            .filter_map(|n| self.card_for(n))
            .collect();
        view.edit(root, |mut v| CanvasView::sync_cards(&mut v, added));
        self.drop_unused_notes(canvas);
    }

    /// Picks up files changed outside the app.
    pub(in crate::app) fn canvas_files_changed(&mut self, root: &mut RenderRoot, touched: &[PathBuf], rescan: bool) {
        let reloaded = self
            .canvas
            .doc
            .as_mut()
            .filter(|doc| rescan || touched.iter().any(|p| doc.is(p)))
            .and_then(|doc| doc.reload().cloned());
        if let Some(canvas) = reloaded {
            self.canvas_replaced(root, canvas);
        }
        let paths: Vec<PathBuf> = self.canvas.notes.keys().filter(|p| rescan || touched.iter().any(|t| aquilum_core::search::paths::same_path(t, p))).cloned().collect();
        for path in paths {
            let Some(card) = self.canvas.notes.get_mut(&path) else { continue };
            if let Some(text) = card.note.reload() {
                card.text = text;
                self.reset_card_note(root, &path, None);
            }
        }
        for embeds in self.canvas.notes.values().filter_map(|c| c.embeds.as_ref()).chain(self.canvas.embeds.as_ref()) {
            embeds.forget_paths(touched);
        }
    }

    /// Embed results and link clicks from the card editors.
    pub(in crate::app) fn poll_canvas_embeds(&mut self, root: &mut RenderRoot) -> bool {
        let mut title = false;
        let mut requests = Vec::new();
        let mut refresh: Vec<WidgetId> = Vec::new();
        if let Some(embeds) = &self.canvas.embeds {
            requests.extend(embeds.take_requests());
            if embeds.take_changed() {
                refresh.extend(self.canvas.editors.iter().filter(|(_, b)| matches!(b, Bound::Text(_))).map(|(id, _)| *id));
                // The "add note" field waits for the same background search results.
                refresh.extend(self.canvas.prompt.as_ref().map(|(handle, _)| handle.id()));
            }
        }
        for card in self.canvas.notes.values() {
            if let Some(embeds) = &card.embeds {
                requests.extend(embeds.take_requests());
                if embeds.take_changed() {
                    refresh.extend(card.editors.iter().map(|h| h.id()));
                }
            }
        }
        for id in refresh {
            if root.has_widget(id) {
                root.edit_widget(id, |mut w| TextEditor::refresh_embeds(&mut w.downcast()));
            }
        }
        for request in requests {
            title |= self.on_editor_request(root, request);
        }
        title
    }

    pub(in crate::app) fn canvas_focus_editor(&self, root: &mut RenderRoot, node: &str) {
        let editor = self.canvas.editors.iter().find_map(|(id, bound)| match bound {
            Bound::Text(n) if n == node => Some(*id),
            Bound::Note(path) => {
                let doc = self.canvas.doc.as_ref()?;
                let file = doc.canvas().node(node)?.file.as_deref()?;
                let root_path = self.tree.root()?;
                let target = crate::paths::absolute(root_path, file);
                (aquilum_core::search::paths::same_path(&target, path) && self.canvas.notes.get(path)?.editors.iter().any(|h| h.id() == *id)).then_some(*id)
            }
            _ => None,
        });
        if let Some(editor) = editor {
            root.focus_on(Some(editor));
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_view(&self) -> Option<WidgetId> {
        self.canvas.view.map(|v| v.id())
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_prompt(&self) -> Option<WidgetId> {
        self.canvas.prompt.as_ref().map(|(h, _)| h.id())
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_editing(&self, root: &RenderRoot) -> Option<String> {
        self.canvas.view.and_then(|v| v.get(root)).and_then(|v| v.inner().editing().map(str::to_owned))
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_board(&self, root: &RenderRoot) -> Option<Canvas> {
        self.canvas.view.and_then(|v| v.get(root)).map(|v| v.inner().canvas().clone())
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_editor(&self, node: &str) -> Option<WidgetId> {
        self.canvas.editors.iter().find(|(_, b)| matches!(b, Bound::Text(n) if n == node)).map(|(id, _)| *id)
    }

    #[cfg(test)]
    pub(in crate::app) fn canvas_note_editor(&self, path: &Path) -> Option<WidgetId> {
        self.canvas.notes.iter().find(|(p, _)| aquilum_core::search::paths::same_path(p, path)).and_then(|(_, c)| c.editors.first()).map(|h| h.id())
    }
}
