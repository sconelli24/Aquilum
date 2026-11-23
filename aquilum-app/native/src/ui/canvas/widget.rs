use std::time::Instant;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, CursorIcon, EventCtx, LayoutCtx, MeasureCtx, PaintCtx, PointerEvent, PropertiesMut, PropertiesRef, QueryCtx, RegisterCtx, TextEvent, Update,
    UpdateCtx, Widget, WidgetMut,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Affine, Axis, Point, Size};
use masonry::layout::{LenReq, Length};

use super::{CAMERA_SETTLE, CanvasAction, CanvasView, Card, DETAIL_SCALE, Gesture, LIVE_CARDS, MAX_SCALE, MIN_SCALE, REACH};
use crate::canvas::geometry::{self, grip_at};

impl CanvasView {
    /// Which cards show live content: visible ones when zoomed in enough, nearest to the
    /// centre first, plus the card being edited whatever happens.
    fn live(&self) -> Vec<bool> {
        let view = self.size.to_rect();
        let center = view.center();
        let detailed = self.camera.scale >= DETAIL_SCALE;
        let mut ranked: Vec<(f64, usize)> = Vec::new();
        let mut live = vec![false; self.cards.len()];
        for (i, slot) in self.cards.iter().enumerate() {
            let Some(node) = self.canvas.node(&slot.id) else { continue };
            if self.editing.as_deref() == Some(slot.id.as_str()) {
                live[i] = true;
                continue;
            }
            let rect = self.local_rect(geometry::rect(node));
            if detailed && rect.intersect(view).area() > 0.0 {
                ranked.push(((rect.center() - center).hypot(), i));
            }
        }
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, i) in ranked.into_iter().take(LIVE_CARDS) {
            live[i] = true;
        }
        live
    }
}

impl Widget for CanvasView {
    type Action = CanvasAction;

    fn on_pointer_event(&mut self, ctx: &mut EventCtx<'_>, _props: &mut PropertiesMut<'_>, event: &PointerEvent) {
        self.pointer(ctx, event);
    }

    fn on_text_event(&mut self, ctx: &mut EventCtx<'_>, _props: &mut PropertiesMut<'_>, event: &TextEvent) {
        match event {
            TextEvent::Keyboard(key) => self.key(ctx, key),
            TextEvent::ClipboardPaste(text) if self.editing.is_none() && !self.readonly => self.paste(ctx, text),
            _ => {}
        }
    }

    fn on_anim_frame(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, interval: u64) {
        let busy = self.camera.advance(interval as f64 / 1e9);
        if busy {
            self.camera.limit(MIN_SCALE, MAX_SCALE);
            ctx.request_layout();
            ctx.request_paint_only();
        }
        if let Some(at) = self.moved_at
            && Instant::now().duration_since(at) >= CAMERA_SETTLE
        {
            self.moved_at = None;
            let (x, y, scale) = self.camera();
            ctx.submit_action::<CanvasAction>(CanvasAction::Camera { x, y, scale });
        }
        if busy || self.moved_at.is_some() {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::FontsChanged = event {
            self.labels.clear();
            self.hint.clear();
            self.notice_line.clear();
            ctx.request_paint_only();
        }
    }

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for slot in &mut self.cards {
            ctx.register_child(&mut slot.pod);
        }
        if let Some(asking) = &mut self.asking {
            ctx.register_child(&mut asking.pod);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx<'_>, _props: &PropertiesRef<'_>, axis: Axis, _len_req: LenReq, _cross: Option<Length>) -> Length {
        ctx.context_size().length(axis).unwrap_or(Length::ZERO)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        self.size = size;
        ctx.set_clip_path(size.to_rect());
        if !self.fitted && size.width > 0.0 && size.height > 0.0 {
            match self.stored.take() {
                Some((x, y, scale)) => self.camera.move_to(x, -y, scale.clamp(MIN_SCALE, MAX_SCALE)),
                None => match self.bounds(None) {
                    Some(bounds) => self.fit(Some(bounds), false),
                    None => self.camera.move_to(0.0, 0.0, 1.0),
                },
            }
            self.fitted = true;
        }
        let affine = self.affine();
        let live = self.live();
        let editing = self.editing.clone();
        for (slot, live) in self.cards.iter_mut().zip(live) {
            if slot.shown != live {
                ctx.set_stashed(&mut slot.pod, !live);
                slot.shown = live;
            }
            let want = editing.as_deref() == Some(slot.id.as_str());
            if slot.editing != want {
                slot.editing = want;
                ctx.mutate_child_later(&mut slot.pod, move |mut card: WidgetMut<'_, Card>| Card::set_editing(&mut card, want));
            }
            if !live {
                continue;
            }
            let Some(node) = self.canvas.node(&slot.id) else { continue };
            if slot.color.as_ref() != Some(&node.color) {
                slot.color = Some(node.color.clone());
                let color = geometry::color(node.color.as_deref());
                ctx.mutate_child_later(&mut slot.pod, move |mut card: WidgetMut<'_, Card>| Card::set_color(&mut card, color));
            }
            ctx.run_layout(&mut slot.pod, Size::new(node.width.max(1.0), node.height.max(1.0)));
            ctx.place_child(&mut slot.pod, Point::ZERO);
            let transform = affine * Affine::translate((node.x, node.y));
            if slot.pushed != Some(transform) {
                slot.pushed = Some(transform);
                ctx.mutate_child_later(&mut slot.pod, move |mut card: WidgetMut<'_, Card>| card.set_transform(transform));
            }
        }
        if let Some(rect) = self.prompt_rect()
            && let Some(asking) = &mut self.asking
        {
            ctx.run_layout(&mut asking.pod, rect.size());
            ctx.place_child(&mut asking.pod, rect.origin());
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        self.paint_board(ctx, painter);
    }

    fn post_paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        self.paint_overlay(ctx, painter);
    }

    fn get_cursor(&self, _ctx: &QueryCtx<'_>, pos: Point) -> CursorIcon {
        match self.gesture {
            Some(Gesture::Pan { .. }) => return CursorIcon::Grabbing,
            Some(Gesture::Move { moved: true, .. }) => return CursorIcon::Move,
            Some(Gesture::Resize { grip, .. }) => return grip.cursor(),
            Some(Gesture::Connect { .. }) => return CursorIcon::Crosshair,
            _ => {}
        }
        if let [id] = self.selected.as_slice()
            && let Some(node) = self.canvas.node(id)
            && let Some(grip) = grip_at(self.local_rect(geometry::rect(node)), pos, REACH)
        {
            return grip.cursor();
        }
        if self.dot_at(pos).is_some() { CursorIcon::Crosshair } else { CursorIcon::Default }
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn accessibility_role(&self) -> Role {
        Role::Canvas
    }

    fn accessibility(&mut self, _ctx: &mut AccessCtx<'_>, _props: &PropertiesRef<'_>, node: &mut Node) {
        node.set_label(crate::i18n::t("canvas.title"));
    }

    fn children_ids(&self) -> ChildrenIds {
        let prompt = self.asking.iter().map(|a| a.pod.id());
        ChildrenIds::from_slice(&self.cards.iter().map(|slot| slot.pod.id()).chain(prompt).collect::<Vec<_>>())
    }
}

impl CanvasView {
    /// Side dot under the pointer on the hovered or selected card, for a new edge.
    pub(super) fn dot_at(&self, local: Point) -> Option<(usize, aquilum_core::canvas::Side)> {
        if self.readonly || self.editing.is_some() {
            return None;
        }
        let world = self.world(local);
        let reach = REACH / self.camera.scale;
        self.canvas.nodes.iter().enumerate().rev().find_map(|(i, node)| {
            let rect = geometry::rect(node);
            if !rect.inflate(reach, reach).contains(world) {
                return None;
            }
            geometry::side_dot(self.local_rect(rect), local, REACH).map(|side| (i, side))
        })
    }
}
