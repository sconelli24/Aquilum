use masonry::app::RenderRoot;
use masonry::kurbo::Point;

use super::FileTree;
use crate::ui::editor::TextEditor;
use crate::ui::scroll::ScrollArea;
use crate::ui::search::Placeholder;
use crate::ui::widgets::Slot;

impl FileTree {
    pub fn filter(&mut self, root: &mut RenderRoot, query: &str) {
        if let Some(toolbar) = self.toolbar {
            toolbar.filter.edit(root, |mut field| TextEditor::reset_text(&mut field, query));
        }
        self.set_filter(root, query);
    }

    pub fn press_outside(&mut self, root: &mut RenderRoot, at: Point) {
        let Some(toolbar) = self.toolbar.filter(|toolbar| root.focused_widget() == Some(toolbar.filter.id())) else { return };
        let pressed = root.get_layer_root(0).find_widget_under_pointer(at).map(|widget| widget.id());
        let inside = pressed.zip(root.get_widget(toolbar.input)).is_some_and(|(id, input)| input.find_widget_by_id(id).is_some());
        if !inside {
            root.focus_on(None);
        }
    }

    /// True when the point is inside the list but not over a row (the empty space below it).
    pub fn is_empty_spot(&self, root: &RenderRoot, at: Point) -> bool {
        let Some(scroll) = self.scroll else { return false };
        if self.model.root().is_none() || self.model.renaming().is_some() {
            return false;
        }
        let Some(under) = root.get_layer_root(0).find_widget_under_pointer(at).map(|widget| widget.id()) else { return false };
        let inside = root.get_widget(scroll.id()).is_some_and(|list| list.find_widget_by_id(under).is_some());
        inside && !self.row_ids.contains(&under)
    }

    pub(super) fn set_filter(&mut self, root: &mut RenderRoot, query: &str) {
        if let Some(toolbar) = self.toolbar {
            toolbar.placeholder.edit(root, |mut placeholder| Placeholder::set_empty(&mut placeholder, query.is_empty()));
            toolbar.clear_slot.edit(root, |mut slot| Slot::set_shown(&mut slot, !query.is_empty()));
        }
        self.model.set_filter(query);
        if !self.model.filtering() {
            self.indexing = None;
        } else if !self.model.indexed() && self.indexing.is_none() {
            self.reindex();
        }
        (self.first, self.offset) = (0, 0.0);
        self.refresh(root);
        if let Some(scroll) = self.scroll {
            scroll.edit(root, |mut scroll| ScrollArea::set_offset(&mut scroll, 0.0));
        }
    }
}
