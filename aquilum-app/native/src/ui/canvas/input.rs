use aquilum_core::canvas::{Edge, Node, NodeKind};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{EventCtx, KeyboardEvent, PointerEvent, PointerScrollEvent, ScrollDelta};
use masonry::kurbo::{Point, Rect, Vec2};

use super::{CanvasAction, CanvasView, Gesture, REACH};
use crate::canvas::geometry::{self, Grip, grip_at, node_at, snap};
use crate::ui::keymap::gesture::{PINCH_ZOOM, WHEEL_ZOOM};
use crate::ui::keymap::{self, pointer};

/// Size of a card made by double-clicking the board, in world units.
const NEW_CARD: (f64, f64) = (260.0, 120.0);
const DUPLICATE_OFFSET: f64 = 40.0;

impl CanvasView {
    fn edge_at(&self, local: Point) -> Option<usize> {
        let affine = self.affine();
        self.canvas.edges.iter().enumerate().rev().find_map(|(i, edge)| {
            let curve = self.curve(edge)?;
            let screen = geometry::Curve { cubic: affine * curve.cubic, ..curve };
            (screen.distance(local) <= REACH).then_some(i)
        })
    }

    pub(super) fn curve(&self, edge: &Edge) -> Option<geometry::Curve> {
        let from = geometry::rect(self.canvas.node(&edge.from_node)?);
        let to = geometry::rect(self.canvas.node(&edge.to_node)?);
        Some(geometry::curve(from, edge.from_side, to, edge.to_side))
    }

    /// The group whose label (drawn above its frame) is under the pointer.
    fn group_label_at(&self, local: Point) -> Option<usize> {
        let line = (f64::from(crate::ui::theme::ui_font_settings().size) * crate::ui::tokens::number::FONT_LINE_HEIGHT_NORMAL).round();
        self.canvas.nodes.iter().enumerate().rev().find_map(|(i, n)| {
            let rect = self.local_rect(geometry::rect(n));
            let band = Rect::new(rect.x0, rect.y0 - line - 8.0, rect.x1.min(rect.x0 + 320.0), rect.y0);
            (n.kind() == NodeKind::Group && band.contains(local)).then_some(i)
        })
    }

    pub(super) fn dismiss_prompt(&mut self, ctx: &mut EventCtx<'_>) {
        if let Some(asking) = self.asking.take() {
            ctx.remove_child(asking.pod);
            ctx.children_changed();
            ctx.submit_action::<CanvasAction>(CanvasAction::PromptClosed);
        }
    }

    fn submit_edit(&self, ctx: &mut EventCtx<'_>) {
        ctx.submit_action::<CanvasAction>(CanvasAction::Edit(self.canvas.clone()));
    }

    fn editable(node: &Node) -> bool {
        match node.kind() {
            NodeKind::Text => true,
            NodeKind::File => node.file.as_deref().is_some_and(|f| aquilum_core::search::paths::is_note(std::path::Path::new(f))),
            _ => false,
        }
    }

    pub(super) fn start_editing(&mut self, ctx: &mut EventCtx<'_>, id: String) {
        self.selected = vec![id.clone()];
        self.edge = None;
        self.editing = Some(id.clone());
        ctx.submit_action::<CanvasAction>(CanvasAction::Editing(Some(id)));
        ctx.request_layout();
        ctx.request_paint_only();
        ctx.request_post_paint();
    }

    fn finish_editing(&mut self, ctx: &mut EventCtx<'_>) {
        if self.editing.take().is_some() {
            ctx.submit_action::<CanvasAction>(CanvasAction::Editing(None));
            ctx.request_layout();
        }
        ctx.request_focus();
    }

    /// Opens what a card points at: a note in a tab, a link in the browser.
    fn activate(&mut self, ctx: &mut EventCtx<'_>, index: usize, new_tab: bool) {
        let node = &self.canvas.nodes[index];
        match node.kind() {
            NodeKind::Link => {
                if let Some(url) = node.url.clone() {
                    ctx.submit_action::<CanvasAction>(CanvasAction::OpenUrl(url));
                }
            }
            NodeKind::File if !Self::editable(node) || new_tab => {
                if let Some(file) = node.file.clone() {
                    ctx.submit_action::<CanvasAction>(CanvasAction::Open { file, new_tab });
                }
            }
            _ if !self.readonly && Self::editable(node) => {
                let id = node.id.clone();
                self.start_editing(ctx, id);
            }
            _ => {}
        }
    }

    /// Pasted text becomes a card under the pointer (or in the middle): a link card for a bare
    /// address, a text card otherwise.
    pub(super) fn paste(&mut self, ctx: &mut EventCtx<'_>, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let at = self.world(self.pointer.unwrap_or(self.size.to_rect().center()));
        let url = !text.contains(char::is_whitespace) && (text.starts_with("http://") || text.starts_with("https://"));
        let node = if url {
            Node::link(text, snap(at.x - 200.0), snap(at.y - 60.0), 400.0, 120.0)
        } else {
            let lines = text.lines().count().clamp(2, 20) as f64;
            Node::text(text, snap(at.x - 200.0), snap(at.y - 30.0), 400.0, snap(lines * 28.0 + 40.0))
        };
        self.selected = vec![node.id.clone()];
        self.edge = None;
        self.canvas.nodes.push(node);
        self.submit_edit(ctx);
        ctx.request_layout();
        ctx.request_paint_only();
        ctx.request_post_paint();
        ctx.set_handled();
    }

    fn create_card(&mut self, ctx: &mut EventCtx<'_>, world: Point) {
        let (w, h) = NEW_CARD;
        let node = Node::text("", snap(world.x - w / 2.0), snap(world.y - h / 2.0), w, h);
        let id = node.id.clone();
        self.canvas.nodes.push(node);
        self.submit_edit(ctx);
        self.start_editing(ctx, id);
    }

    /// Nodes that move with the selection: the selected ones and everything inside selected groups.
    fn movers(&self) -> Vec<(usize, f64, f64)> {
        let groups: Vec<Rect> = self.canvas.nodes.iter().filter(|n| n.kind() == NodeKind::Group && self.selected.contains(&n.id)).map(geometry::rect).collect();
        self.canvas
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| self.selected.contains(&n.id) || groups.iter().any(|g| g.contains_rect(geometry::rect(n))))
            .map(|(i, n)| (i, n.x, n.y))
            .collect()
    }

    pub(super) fn pointer(&mut self, ctx: &mut EventCtx<'_>, event: &PointerEvent) {
        match event {
            PointerEvent::Down(down) if pointer::context(down.button) => {
                let local = ctx.local_position(down.state.position);
                let world = self.world(local);
                let node = node_at(&self.canvas.nodes, world).map(|i| self.canvas.nodes[i].id.clone());
                let edge = if node.is_none() { self.edge_at(local).map(|i| self.canvas.edges[i].id.clone()) } else { None };
                if let Some(id) = &node
                    && !self.selected.contains(id)
                {
                    self.selected = vec![id.clone()];
                }
                if edge.is_some() {
                    self.selected.clear();
                    self.edge = edge.clone();
                }
                self.dismiss_prompt(ctx);
                ctx.request_focus();
                ctx.request_paint_only();
                ctx.request_post_paint();
                ctx.submit_action::<CanvasAction>(CanvasAction::Menu { at: ctx.to_window(local), world, node, edge });
                ctx.set_handled();
            }
            PointerEvent::Down(down) if pointer::press(down.button) => {
                let local = ctx.local_position(down.state.position);
                self.press(ctx, local, down.state.count, down.state.modifiers.shift(), keymap::action(down.state.modifiers), pointer::drag_slop(down.pointer.pointer_type));
                ctx.capture_pointer();
                ctx.set_handled();
            }
            PointerEvent::Move(update) => {
                let local = ctx.local_position(update.current.position);
                self.drag(ctx, local, update.current.modifiers.alt());
            }
            PointerEvent::Up(up) => {
                let local = ctx.local_position(up.state.position);
                self.release(ctx, local);
            }
            PointerEvent::Cancel(_) => {
                self.gesture = None;
                ctx.request_paint_only();
                ctx.request_post_paint();
            }
            PointerEvent::Leave(_) => {
                if self.pointer.take().is_some() {
                    ctx.request_paint_only();
                    ctx.request_post_paint();
                }
            }
            PointerEvent::Scroll(PointerScrollEvent { delta, state, .. }) => {
                let local = ctx.local_position(state.position);
                let scale = ctx.scale_factor().max(0.1);
                match delta {
                    ScrollDelta::LineDelta(_, y) => self.zoom_at((f64::from(*y) * WHEEL_ZOOM).exp(), local),
                    ScrollDelta::PixelDelta(_) if pointer::zoom(state.modifiers) => {
                        self.zoom_at((crate::ui::scroll::pixels(delta, scale, 0.0).y * PINCH_ZOOM).exp(), local);
                    }
                    ScrollDelta::PixelDelta(_) => self.pan(crate::ui::scroll::pixels(delta, scale, 0.0)),
                    _ => return,
                }
                self.moved();
                ctx.request_layout();
                ctx.request_anim_frame();
                ctx.set_handled();
            }
            PointerEvent::Gesture(gesture) => {
                if let masonry::core::PointerGesture::Pinch(delta) = gesture.gesture {
                    let local = ctx.local_position(gesture.state.position);
                    self.zoom_at((1.0 + f64::from(delta)).max(0.01), local);
                    self.moved();
                    ctx.request_layout();
                    ctx.request_anim_frame();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn press(&mut self, ctx: &mut EventCtx<'_>, local: Point, count: u8, shift: bool, action: bool, slop: f64) {
        if self.editing.is_some() {
            self.finish_editing(ctx);
        }
        self.dismiss_prompt(ctx);
        self.flash = None;
        ctx.request_focus();
        ctx.request_paint_only();
        ctx.request_post_paint();
        let world = self.world(local);
        let hit = node_at(&self.canvas.nodes, world);
        if count >= 2 {
            if !self.readonly {
                if let Some(group) = self.group_label_at(local) {
                    let target = super::Prompt::GroupLabel(self.canvas.nodes[group].id.clone());
                    let text = self.canvas.nodes[group].label.clone().unwrap_or_default();
                    ctx.submit_action::<CanvasAction>(CanvasAction::Prompt { target, text });
                    return;
                }
                if hit.is_none_or(|i| self.canvas.nodes[i].kind() == NodeKind::Group)
                    && let Some(edge) = self.edge_at(local)
                {
                    let edge = &self.canvas.edges[edge];
                    ctx.submit_action::<CanvasAction>(CanvasAction::Prompt { target: super::Prompt::EdgeLabel(edge.id.clone()), text: edge.label.clone().unwrap_or_default() });
                    return;
                }
            }
            match hit {
                Some(index) => self.activate(ctx, index, action),
                None if !self.readonly && self.edge_at(local).is_none() => self.create_card(ctx, world),
                None => {}
            }
            return;
        }
        if !self.readonly {
            if let [id] = self.selected.as_slice()
                && let Some(index) = self.index(id)
                && let Some(grip) = grip_at(self.local_rect(geometry::rect(&self.canvas.nodes[index])), local, REACH)
            {
                let rect = geometry::rect(&self.canvas.nodes[index]);
                self.gesture = Some(Gesture::Resize { node: index, grip, start: world, rect });
                return;
            }
            if let Some((from, side)) = self.dot_at(local) {
                self.gesture = Some(Gesture::Connect { from, side, end: world });
                return;
            }
        }
        if let Some(index) = hit {
            let id = self.canvas.nodes[index].id.clone();
            let was_only = self.selected == [id.clone()];
            self.edge = None;
            if shift {
                match self.selected.iter().position(|s| *s == id) {
                    Some(at) => _ = self.selected.remove(at),
                    None => self.selected.push(id),
                }
            } else if !self.selected.contains(&id) {
                self.selected = vec![id];
            }
            let click_edit = was_only && !shift && !self.readonly && Self::editable(&self.canvas.nodes[index]);
            self.origins = self.movers();
            self.gesture = Some(Gesture::Move { start: world, travelled: 0.0, slop, moved: false, click_edit });
            return;
        }
        if let Some(edge) = self.edge_at(local) {
            self.selected.clear();
            self.edge = Some(self.canvas.edges[edge].id.clone());
            return;
        }
        self.edge = None;
        if shift {
            self.gesture = Some(Gesture::Marquee { start: world, end: world });
        } else {
            self.selected.clear();
            self.gesture = Some(Gesture::Pan { last: local, travelled: 0.0, slop });
        }
    }

    fn drag(&mut self, ctx: &mut EventCtx<'_>, local: Point, free: bool) {
        let world = self.world(local);
        let previous = self.pointer.replace(local);
        let mut pan = None;
        match &mut self.gesture {
            None => {
                if previous.is_none_or(|p| (p - local).hypot() > 0.5) {
                    ctx.request_paint_only();
                    ctx.request_post_paint();
                }
            }
            Some(Gesture::Pan { last, travelled, slop }) => {
                let delta = local - *last;
                *last = local;
                *travelled += delta.hypot();
                if *travelled >= *slop {
                    pan = Some(delta);
                }
            }
            Some(Gesture::Move { start, travelled, slop, moved, .. }) => {
                let delta = world - *start;
                *travelled = delta.hypot() * self.camera.scale;
                if *travelled < *slop && !*moved || self.readonly {
                    return;
                }
                *moved = true;
                let anchor = self.origins.first().map(|&(_, x, y)| Vec2::new(x, y)).unwrap_or_default();
                // Snap the first mover to the grid and move the rest by the same amount.
                let shift = if free { delta } else { Vec2::new(snap(anchor.x + delta.x) - anchor.x, snap(anchor.y + delta.y) - anchor.y) };
                for &(i, x, y) in &self.origins {
                    if let Some(node) = self.canvas.nodes.get_mut(i) {
                        node.x = x + shift.x;
                        node.y = y + shift.y;
                    }
                }
                ctx.request_layout();
                ctx.request_paint_only();
                ctx.request_post_paint();
            }
            Some(Gesture::Resize { node, grip, start, rect }) => {
                let resized = grip.resize(*rect, world - *start);
                let resized = if free { resized } else { snap_rect(resized, *grip) };
                if let Some(node) = self.canvas.nodes.get_mut(*node) {
                    (node.x, node.y, node.width, node.height) = (resized.x0, resized.y0, resized.width(), resized.height());
                }
                ctx.request_layout();
                ctx.request_paint_only();
                ctx.request_post_paint();
            }
            Some(Gesture::Connect { end, .. }) | Some(Gesture::Marquee { end, .. }) => {
                *end = world;
                ctx.request_paint_only();
                ctx.request_post_paint();
            }
        }
        if let Some(delta) = pan {
            self.pan(delta);
            self.moved();
            ctx.request_layout();
            ctx.request_anim_frame();
        }
    }

    fn release(&mut self, ctx: &mut EventCtx<'_>, local: Point) {
        let Some(gesture) = self.gesture.take() else { return };
        let world = self.world(local);
        match gesture {
            Gesture::Pan { .. } => {}
            Gesture::Move { moved: true, .. } => self.submit_edit(ctx),
            Gesture::Move { click_edit: true, .. } => {
                if let [id] = self.selected.as_slice() {
                    let id = id.clone();
                    self.start_editing(ctx, id);
                }
            }
            Gesture::Move { .. } => {}
            Gesture::Resize { rect, node, .. } => {
                if self.canvas.nodes.get(node).is_some_and(|n| geometry::rect(n) != rect) {
                    self.submit_edit(ctx);
                }
            }
            Gesture::Connect { from, side, .. } => {
                if let Some(to) = node_at(&self.canvas.nodes, world).filter(|&to| to != from) {
                    let target = geometry::rect(&self.canvas.nodes[to]);
                    let to_side = geometry::side_dot(self.local_rect(target), local, REACH * 2.0).unwrap_or_else(|| geometry::facing(target, geometry::anchor(geometry::rect(&self.canvas.nodes[from]), side)));
                    let edge = Edge::new(&self.canvas.nodes[from].id, Some(side), &self.canvas.nodes[to].id, Some(to_side));
                    self.edge = Some(edge.id.clone());
                    self.selected.clear();
                    self.canvas.edges.push(edge);
                    self.submit_edit(ctx);
                }
            }
            Gesture::Marquee { start, end } => {
                let area = Rect::from_points(start, end);
                self.selected = self.canvas.nodes.iter().filter(|n| area.intersect(geometry::rect(n)).area() > 0.0).map(|n| n.id.clone()).collect();
            }
        }
        ctx.request_layout();
        ctx.request_paint_only();
        ctx.request_post_paint();
    }

    pub(super) fn key(&mut self, ctx: &mut EventCtx<'_>, key: &KeyboardEvent) {
        if key.state != KeyState::Down {
            return;
        }
        let action = keymap::action(key.modifiers);
        let shift = key.modifiers.shift();
        if self.editing.is_some() {
            if key.key == Key::Named(NamedKey::Escape) {
                self.finish_editing(ctx);
                ctx.set_handled();
            }
            return;
        }
        let character = match &key.key {
            Key::Character(c) => Some(c.to_lowercase()),
            _ => None,
        };
        match (&key.key, character.as_deref()) {
            (Key::Named(NamedKey::Escape), _) => {
                self.selected.clear();
                self.edge = None;
            }
            (Key::Named(NamedKey::Delete | NamedKey::Backspace), _) if !self.readonly => self.delete_selected(ctx),
            (Key::Named(NamedKey::Enter), _) => {
                if let [id] = self.selected.as_slice()
                    && let Some(index) = self.index(id)
                {
                    self.activate(ctx, index, false);
                }
            }
            (_, Some("z")) if action => ctx.submit_action::<CanvasAction>(CanvasAction::Undo { redo: shift }),
            (_, Some("y")) if action => ctx.submit_action::<CanvasAction>(CanvasAction::Undo { redo: true }),
            (_, Some("a")) if action => self.selected = self.canvas.nodes.iter().map(|n| n.id.clone()).collect(),
            (_, Some("d")) if action && !self.readonly => self.duplicate(ctx),
            (_, Some("1" | "!")) if shift => {
                let bounds = self.bounds(None);
                self.fit(bounds, true);
                self.moved();
                ctx.request_anim_frame();
            }
            (_, Some("2" | "@")) if shift => {
                let bounds = self.bounds(Some(&self.selected.clone()));
                self.fit(bounds, true);
                self.moved();
                ctx.request_anim_frame();
            }
            _ => return,
        }
        ctx.request_paint_only();
        ctx.request_post_paint();
        ctx.set_handled();
    }

    pub(super) fn delete_selected(&mut self, ctx: &mut EventCtx<'_>) {
        if let Some(edge) = self.edge.take() {
            self.canvas.edges.retain(|e| e.id != edge);
        } else if !self.selected.is_empty() {
            let gone = std::mem::take(&mut self.selected);
            self.canvas.remove_nodes(&gone);
        } else {
            return;
        }
        self.submit_edit(ctx);
        ctx.request_layout();
    }

    fn duplicate(&mut self, ctx: &mut EventCtx<'_>) {
        if self.selected.is_empty() {
            return;
        }
        let mut renamed = std::collections::HashMap::new();
        let copies: Vec<Node> = self
            .canvas
            .nodes
            .iter()
            .filter(|n| self.selected.contains(&n.id))
            .map(|n| {
                let mut copy = n.clone();
                copy.id = aquilum_core::canvas::new_id();
                copy.x += DUPLICATE_OFFSET;
                copy.y += DUPLICATE_OFFSET;
                renamed.insert(n.id.clone(), copy.id.clone());
                copy
            })
            .collect();
        let edges: Vec<Edge> = self
            .canvas
            .edges
            .iter()
            .filter_map(|e| {
                let mut copy = e.clone();
                copy.from_node = renamed.get(&e.from_node)?.clone();
                copy.to_node = renamed.get(&e.to_node)?.clone();
                copy.id = aquilum_core::canvas::new_id();
                Some(copy)
            })
            .collect();
        self.selected = copies.iter().map(|n| n.id.clone()).collect();
        self.canvas.nodes.extend(copies);
        self.canvas.edges.extend(edges);
        self.submit_edit(ctx);
        ctx.request_layout();
    }
}

/// Snaps the edges a grip moves to the grid.
fn snap_rect(rect: Rect, grip: Grip) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
    if matches!(grip, Grip::N | Grip::NE | Grip::NW) {
        y0 = snap(y0).min(y1 - geometry::MIN_HEIGHT);
    }
    if matches!(grip, Grip::S | Grip::SE | Grip::SW) {
        y1 = snap(y1).max(y0 + geometry::MIN_HEIGHT);
    }
    if matches!(grip, Grip::W | Grip::NW | Grip::SW) {
        x0 = snap(x0).min(x1 - geometry::MIN_WIDTH);
    }
    if matches!(grip, Grip::E | Grip::NE | Grip::SE) {
        x1 = snap(x1).max(x0 + geometry::MIN_WIDTH);
    }
    Rect::new(x0, y0, x1, y1)
}
