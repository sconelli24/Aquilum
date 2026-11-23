use aquilum_core::canvas::Canvas;
use masonry::core::{NewWidget, WidgetMut};

use super::{Card, CanvasView, Slot};
use crate::graph::camera::Camera;

impl CanvasView {
    /// `camera` is a stored (centre x, centre y, zoom) to restore instead of fitting the board.
    pub fn new(canvas: Canvas, cards: Vec<(String, NewWidget<Card>)>, camera: Option<(f64, f64, f64)>, notice: Option<String>) -> NewWidget<Self> {
        let readonly = notice.is_some();
        let cards = cards.into_iter().map(|(id, card)| Slot { id, pod: card.to_pod(), shown: true, pushed: None, editing: false, color: None }).collect();
        NewWidget::new(CanvasView {
            canvas,
            cards,
            camera: Camera::default(),
            stored: camera,
            fitted: false,
            size: Default::default(),
            selected: Vec::new(),
            edge: None,
            editing: None,
            gesture: None,
            origins: Vec::new(),
            pointer: None,
            moved_at: None,
            labels: Default::default(),
            asking: None,
            notice,
            flash: None,
            hint: Default::default(),
            notice_line: Default::default(),
            readonly,
        })
    }

    #[cfg(test)]
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    #[cfg(test)]
    pub fn editing(&self) -> Option<&str> {
        self.editing.as_deref()
    }


    pub fn camera(&self) -> (f64, f64, f64) {
        let c = self.center();
        (c.x, c.y, self.camera.scale)
    }

    pub fn card_ids(&self) -> impl Iterator<Item = &str> {
        self.cards.iter().map(|c| c.id.as_str())
    }

    /// Replaces the board (undo, reload, a conflict). Selection keeps the nodes that remain.
    pub fn set_canvas(this: &mut WidgetMut<'_, Self>, canvas: Canvas) {
        let widget = &mut *this.widget;
        widget.selected.retain(|id| canvas.node(id).is_some());
        widget.edge = widget.edge.take().filter(|id| canvas.edges.iter().any(|e| &e.id == id));
        widget.canvas = canvas;
        widget.gesture = None;
        for slot in &mut widget.cards {
            slot.pushed = None;
        }
        this.ctx.request_layout();
        this.ctx.request_paint_only();
    }

    /// Text typed into a card, mirrored so later edits of the board carry it.
    pub fn set_text(this: &mut WidgetMut<'_, Self>, id: &str, text: &str) {
        if let Some(node) = this.widget.canvas.node_mut(id) {
            node.text = Some(text.to_owned());
        }
    }

    pub fn flash(this: &mut WidgetMut<'_, Self>, message: String) {
        this.widget.flash = Some(message);
        this.ctx.request_paint_only();
    }

    pub fn set_notice(this: &mut WidgetMut<'_, Self>, notice: Option<String>) {
        this.widget.readonly = notice.is_some();
        this.widget.notice = notice;
        this.ctx.request_paint_only();
    }

    /// Drops cards whose node is gone and adds new ones.
    pub fn sync_cards(this: &mut WidgetMut<'_, Self>, added: Vec<(String, NewWidget<Card>)>) {
        let canvas = &this.widget.canvas;
        let (keep, gone): (Vec<Slot>, Vec<Slot>) = std::mem::take(&mut this.widget.cards).into_iter().partition(|slot| canvas.node(&slot.id).is_some());
        this.widget.cards = keep;
        for slot in gone {
            this.ctx.remove_child(slot.pod);
        }
        for (id, card) in added {
            if let Some(old) = this.widget.cards.iter().position(|s| s.id == id) {
                let slot = this.widget.cards.remove(old);
                this.ctx.remove_child(slot.pod);
            }
            // Its edit mode and transform are applied in layout once it is registered.
            this.widget.cards.push(Slot { id, pod: card.to_pod(), shown: true, pushed: None, editing: false, color: None });
        }
        this.ctx.children_changed();
        this.ctx.request_layout();
    }

    /// Leaves edit mode: the card editor gave up (Escape).
    pub fn stop_editing(this: &mut WidgetMut<'_, Self>) {
        if this.widget.editing.take().is_some() {
            this.ctx.request_layout();
            this.ctx.request_paint_only();
        }
    }

    /// Recentres on a node and starts editing it, as after creating it.
    pub fn begin_edit(this: &mut WidgetMut<'_, Self>, id: &str) {
        this.widget.editing = Some(id.to_owned());
        this.widget.selected = vec![id.to_owned()];
        this.ctx.request_layout();
    }

    pub fn fit_all(this: &mut WidgetMut<'_, Self>) {
        let bounds = this.widget.bounds(None);
        this.widget.fit(bounds, true);
        this.widget.moved();
        this.ctx.request_anim_frame();
    }
}

impl CanvasView {
    /// Where the prompt sits on screen: over what it edits, never narrower than a short line.
    pub(super) fn prompt_rect(&self) -> Option<masonry::kurbo::Rect> {
        use masonry::kurbo::{Point, Rect};
        let target = &self.asking.as_ref()?.target;
        let line = (f64::from(crate::ui::theme::ui_font_settings().size) * crate::ui::tokens::number::FONT_LINE_HEIGHT_NORMAL).round() + 12.0;
        let (at, width) = match target {
            super::Prompt::GroupLabel(id) => {
                let rect = self.local_rect(crate::canvas::geometry::rect(self.canvas.node(id)?));
                (Point::new(rect.x0, rect.y0 - line - 4.0), rect.width())
            }
            super::Prompt::LinkUrl(id) => {
                let rect = self.local_rect(crate::canvas::geometry::rect(self.canvas.node(id)?));
                (Point::new(rect.x0, rect.y0 - line - 4.0), rect.width())
            }
            super::Prompt::EdgeLabel(id) => {
                let edge = self.canvas.edges.iter().find(|e| &e.id == id)?;
                let middle = self.affine() * self.curve(edge)?.middle();
                (Point::new(middle.x - 120.0, middle.y - line / 2.0), 240.0)
            }
            super::Prompt::Note(world) => (self.local(*world), 280.0),
        };
        Some(Rect::from_origin_size(at, (width.clamp(220.0, 480.0), line)))
    }

    /// Shows a one-line editor for `target`; the app reads its `Entered` / `Cancelled`.
    pub fn set_prompt(this: &mut WidgetMut<'_, Self>, target: super::Prompt, editor: NewWidget<dyn masonry::core::Widget>) {
        if let Some(old) = this.widget.asking.take() {
            this.ctx.remove_child(old.pod);
        }
        this.widget.asking = Some(super::Asking { target, pod: editor.to_pod() });
        this.ctx.children_changed();
        this.ctx.request_layout();
    }

    pub fn close_prompt(this: &mut WidgetMut<'_, Self>) {
        if let Some(old) = this.widget.asking.take() {
            this.ctx.remove_child(old.pod);
            this.ctx.children_changed();
            this.ctx.request_layout();
        }
    }

    fn changed(this: &mut WidgetMut<'_, Self>) -> Canvas {
        this.ctx.request_layout();
        this.ctx.request_paint_only();
        this.widget.canvas.clone()
    }

    /// Adds nodes, selects them and returns the board for the app to commit.
    pub fn add_nodes(this: &mut WidgetMut<'_, Self>, nodes: Vec<aquilum_core::canvas::Node>) -> Canvas {
        this.widget.selected = nodes.iter().map(|n| n.id.clone()).collect();
        this.widget.edge = None;
        // Groups go first so they stay behind the cards already on the board.
        for node in nodes {
            if node.kind() == aquilum_core::canvas::NodeKind::Group {
                this.widget.canvas.nodes.insert(0, node);
            } else {
                this.widget.canvas.nodes.push(node);
            }
        }
        Self::changed(this)
    }

    /// Colours the selected nodes, or the selected edge. `None` when nothing is selected.
    pub fn set_color(this: &mut WidgetMut<'_, Self>, color: Option<String>) -> Option<Canvas> {
        let widget = &mut *this.widget;
        if let Some(edge) = widget.edge.clone() {
            widget.canvas.edges.iter_mut().filter(|e| e.id == edge).for_each(|e| e.color = color.clone());
        } else if !widget.selected.is_empty() {
            let selected = widget.selected.clone();
            widget.canvas.nodes.iter_mut().filter(|n| selected.contains(&n.id)).for_each(|n| n.color = color.clone());
        } else {
            return None;
        }
        Some(Self::changed(this))
    }

    /// Applies the prompt's answer to what it was editing.
    pub fn answer(this: &mut WidgetMut<'_, Self>, target: &super::Prompt, text: &str) -> Option<Canvas> {
        let text = text.trim();
        let value = (!text.is_empty()).then(|| text.to_owned());
        let canvas = &mut this.widget.canvas;
        match target {
            super::Prompt::GroupLabel(id) => canvas.node_mut(id)?.label = value,
            super::Prompt::LinkUrl(id) => canvas.node_mut(id)?.url = Some(value.unwrap_or_default()),
            super::Prompt::EdgeLabel(id) => canvas.edges.iter_mut().find(|e| &e.id == id)?.label = value,
            super::Prompt::Note(_) => return None,
        }
        Some(Self::changed(this))
    }

    pub fn delete_selection(this: &mut WidgetMut<'_, Self>) -> Option<Canvas> {
        let widget = &mut *this.widget;
        if let Some(edge) = widget.edge.take() {
            widget.canvas.edges.retain(|e| e.id != edge);
        } else if !widget.selected.is_empty() {
            let gone = std::mem::take(&mut widget.selected);
            widget.canvas.remove_nodes(&gone);
        } else {
            return None;
        }
        Some(Self::changed(this))
    }

    /// Board rectangle around the selection, for wrapping it in a group.
    pub fn selection_bounds(&self) -> Option<masonry::kurbo::Rect> {
        if self.selected.is_empty() { None } else { self.bounds(Some(&self.selected)) }
    }
}
