use aquilum_core::canvas::{Node, NodeKind};
use masonry::core::{PaintCtx, render_text};
use masonry::imaging::Painter;
use masonry::kurbo::{Affine, BezPath, Circle, Point, Rect, RoundedRect, Shape, Stroke};
use masonry::peniko::Color;

use super::card::{BORDER, RADIUS};
use super::{CanvasView, Gesture, PANEL_BUTTON, ZoomButton};
use crate::ui::icons;
use crate::canvas::geometry::{self, Grip};
use crate::ui::text::Style;
use crate::ui::theme;
use crate::ui::tokens::{number, size};

/// Smallest gap between background dots, in screen pixels.
const DOT_GAP: f64 = 14.0;
const EDGE_WIDTH: f64 = 2.0;
const ARROW: f64 = 10.0;
const GRIP: f64 = 7.0;
const DOT: f64 = 5.0;
/// Size of the icons in the zoom panel.
const ICON: f64 = 16.0;

/// The short text a tile or label shows for a node.
pub(super) fn title(node: &Node) -> String {
    match node.kind() {
        NodeKind::File => node
            .file
            .as_deref()
            .map(|f| std::path::Path::new(f).file_stem().map_or_else(|| f.to_owned(), |s| s.to_string_lossy().into_owned()))
            .unwrap_or_default(),
        NodeKind::Text => node
            .text
            .as_deref()
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().trim_start_matches(['#', '-', '>', '*', ' ']).trim())
            .find(|l| !l.is_empty())
            .unwrap_or_default()
            .to_owned(),
        NodeKind::Link => node.url.clone().unwrap_or_default(),
        NodeKind::Group => node.label.clone().unwrap_or_default(),
        NodeKind::Other => node.kind.clone(),
    }
}

impl CanvasView {
    fn line_height() -> f64 {
        (f64::from(theme::ui_font_settings().size) * number::FONT_LINE_HEIGHT_NORMAL).round()
    }

    fn label(&mut self, ctx: &mut PaintCtx<'_>, painter: &mut Painter<'_>, key: &str, text: &str, at: Point, width: f64, color: Color) {
        if text.is_empty() || width < 8.0 {
            return;
        }
        let line = self.labels.entry(key.to_owned()).or_default();
        let layout = line.layout(ctx, text, width, Style::ui(Self::line_height()));
        render_text(painter, Affine::translate(at.to_vec2()), layout, &[color.into()], true);
    }

    fn paint_dots(&self, painter: &mut Painter<'_>) {
        let tm = theme::current();
        let mut gap = geometry::GRID * self.camera.scale;
        while gap < DOT_GAP {
            gap *= 5.0;
        }
        let origin = self.local(Point::ZERO);
        let (start_x, start_y) = (origin.x.rem_euclid(gap), origin.y.rem_euclid(gap));
        let mut dots = BezPath::new();
        let mut y = start_y;
        while y < self.size.height {
            let mut x = start_x;
            while x < self.size.width {
                dots.extend(Rect::from_center_size((x, y), (1.5, 1.5)).path_elements(0.1));
                x += gap;
            }
            y += gap;
        }
        painter.fill(&dots, tm.border_default.multiply_alpha(0.7)).draw();
    }

    pub(super) fn paint_board(&mut self, ctx: &mut PaintCtx<'_>, painter: &mut Painter<'_>) {
        let tm = theme::current();
        painter.fill(self.size.to_rect(), tm.graph_backdrop).draw();
        self.paint_dots(painter);
        let view = self.size.to_rect();
        let line = Self::line_height();
        let affine = self.affine();
        let nodes = self.canvas.nodes.clone();

        for node in nodes.iter().filter(|n| n.kind() == NodeKind::Group) {
            let rect = self.local_rect(geometry::rect(node));
            if rect.intersect(view).area() <= 0.0 && !rect.inflate(0.0, line).contains(rect.origin()) {
                continue;
            }
            let color = geometry::color(node.color.as_deref());
            let shape = RoundedRect::from_rect(rect, RADIUS * self.camera.scale);
            painter.fill(shape, color.unwrap_or(tm.text_muted).multiply_alpha(0.06)).draw();
            painter.stroke(shape, &Stroke::new(1.5), color.unwrap_or(tm.border_emphasis)).draw();
            let label = node.label.clone().unwrap_or_default();
            self.label(ctx, painter, &node.id, &label, Point::new(rect.x0, rect.y0 - line - size::SPACE_4), rect.width(), tm.text_secondary);
        }

        let edges = self.canvas.edges.clone();
        // Like Obsidian, the edge under the pointer is highlighted unless a card or a drag is in the way.
        let hovered_edge = match (self.pointer, &self.gesture) {
            (Some(p), None) if !self.space && geometry::node_at(&self.canvas.nodes, self.world(p)).is_none() => self.edge_at(p).map(|i| self.canvas.edges[i].id.clone()),
            _ => None,
        };
        for edge in &edges {
            let Some(curve) = self.curve(edge) else { continue };
            let screen = geometry::Curve { cubic: affine * curve.cubic, ..curve };
            let selected = self.edge.as_deref() == Some(edge.id.as_str());
            let hovered = !selected && hovered_edge.as_deref() == Some(edge.id.as_str());
            let color = if selected {
                tm.border_accent
            } else if hovered {
                tm.border_accent.multiply_alpha(0.7)
            } else {
                geometry::color(edge.color.as_deref()).unwrap_or(tm.text_muted)
            };
            painter.stroke(&screen.path(), &Stroke::new(if selected || hovered { EDGE_WIDTH + 1.0 } else { EDGE_WIDTH }), color).draw();
            if edge.to_arrow() {
                painter.fill(&screen.arrow(true, ARROW), color).draw();
            }
            if edge.from_arrow() {
                painter.fill(&screen.arrow(false, ARROW), color).draw();
            }
            if let Some(label) = edge.label.as_deref().filter(|l| !l.is_empty()) {
                let middle = screen.middle();
                let width = 240.0;
                let key = format!("edge:{}", edge.id);
                let text_width = {
                    let (fcx, lcx) = ctx.text_contexts();
                    self.labels.entry(key.clone()).or_default().width(fcx, lcx, label, Style::ui(line)).min(width)
                };
                let pill = Rect::from_center_size(middle, (text_width + size::SPACE_8 * 2.0, line + size::SPACE_4 * 2.0));
                painter.fill(RoundedRect::from_rect(pill, size::ROUNDED_SM), tm.graph_backdrop).draw();
                self.label(ctx, painter, &key, label, Point::new(pill.x0 + size::SPACE_8, pill.y0 + size::SPACE_4), width, tm.text_secondary);
            }
        }

        // Cards without live content: links, unknown types and everything when zoomed far out.
        let shown: std::collections::HashSet<&str> = self.cards.iter().filter(|s| s.shown).map(|s| s.id.as_str()).collect();
        let tiles: Vec<Node> = nodes.iter().filter(|n| n.kind() != NodeKind::Group && !shown.contains(n.id.as_str())).cloned().collect();
        drop(shown);
        for node in &tiles {
            let rect = self.local_rect(geometry::rect(node));
            if rect.intersect(view).area() <= 0.0 {
                continue;
            }
            let color = geometry::color(node.color.as_deref());
            let radius = RADIUS * self.camera.scale;
            let shape = RoundedRect::from_rect(rect, radius);
            painter.fill(shape, tm.bg_surface).draw();
            if let Some(color) = color {
                painter.fill(shape, color.multiply_alpha(0.08)).draw();
            }
            painter.stroke(shape, &Stroke::new((BORDER * self.camera.scale).max(1.0)), color.unwrap_or(tm.border_default)).draw();
            if rect.height() >= line + size::SPACE_4 {
                let text = title(node);
                let pad = size::SPACE_8.min(rect.width() / 4.0);
                let primary = if node.kind() == NodeKind::Link { tm.text_link } else { tm.text_primary };
                self.label(ctx, painter, &node.id, &text, Point::new(rect.x0 + pad, rect.y0 + pad.min(rect.height() / 4.0)), rect.width() - pad * 2.0, primary);
                if node.kind() == NodeKind::Link && rect.height() >= line * 2.5 {
                    let key = format!("host:{}", node.id);
                    let host = node.url.as_deref().and_then(|u| url_host(u)).unwrap_or_default();
                    self.label(ctx, painter, &key, &host, Point::new(rect.x0 + pad, rect.y0 + pad + line), rect.width() - pad * 2.0, tm.text_muted);
                }
            }
        }

        // File names above live file cards, as Obsidian shows them.
        for node in nodes.iter().filter(|n| n.kind() == NodeKind::File) {
            let rect = self.local_rect(geometry::rect(node));
            if rect.intersect(view.inflate(0.0, line)).area() <= 0.0 || self.camera.scale < super::DETAIL_SCALE {
                continue;
            }
            let key = format!("file:{}", node.id);
            self.label(ctx, painter, &key, &title(node), Point::new(rect.x0, rect.y0 - line - size::SPACE_4), rect.width(), tm.text_muted);
        }
    }

    pub(super) fn paint_overlay(&mut self, ctx: &mut PaintCtx<'_>, painter: &mut Painter<'_>) {
        let tm = theme::current();
        let accent = tm.border_accent;
        let selected: Vec<Rect> = self.selected.iter().filter_map(|id| self.canvas.node(id)).map(|n| self.local_rect(geometry::rect(n))).collect();
        for rect in &selected {
            let shape = RoundedRect::from_rect(rect.inflate(2.0, 2.0), RADIUS * self.camera.scale + 2.0);
            painter.stroke(shape, &Stroke::new(2.0), accent).draw();
        }
        if let ([rect], false, None) = (selected.as_slice(), self.readonly, &self.editing) {
            for grip in Grip::ALL {
                let square = Rect::from_center_size(grip.point(*rect), (GRIP, GRIP));
                painter.fill(square, tm.editor_bg).draw();
                painter.stroke(square, &Stroke::new(1.5), accent).draw();
            }
        }

        // Connection dots on the card under the pointer.
        if !self.readonly
            && self.editing.is_none()
            && matches!(self.gesture, None | Some(Gesture::Connect { .. }))
            && let Some(pointer) = self.pointer
            && let Some(index) = geometry::node_at(&self.canvas.nodes, self.world(pointer))
            && self.canvas.nodes[index].kind() != NodeKind::Group
        {
            let rect = self.local_rect(geometry::rect(&self.canvas.nodes[index]));
            for side in [aquilum_core::canvas::Side::Top, aquilum_core::canvas::Side::Right, aquilum_core::canvas::Side::Bottom, aquilum_core::canvas::Side::Left] {
                let dot = Circle::new(geometry::anchor(rect, side), DOT);
                painter.fill(dot, tm.editor_bg).draw();
                painter.stroke(dot, &Stroke::new(1.5), accent).draw();
            }
        }

        match self.gesture {
            Some(Gesture::Connect { from, side, end }) => {
                if let Some(node) = self.canvas.nodes.get(from) {
                    let start = geometry::anchor(geometry::rect(node), side);
                    let curve = geometry::curve_to(start, side, end, None);
                    let screen = geometry::Curve { cubic: self.affine() * curve.cubic, ..curve };
                    painter.stroke(&screen.path(), &Stroke::new(EDGE_WIDTH).with_dashes(0.0, [6.0, 4.0]), accent).draw();
                    painter.fill(&screen.arrow(true, ARROW), accent).draw();
                }
            }
            Some(Gesture::Marquee { start, end }) => {
                let rect = Rect::from_points(self.local(start), self.local(end));
                painter.fill(rect, accent.multiply_alpha(0.08)).draw();
                painter.stroke(rect, &Stroke::new(1.0), accent).draw();
            }
            _ => {}
        }

        // Floating zoom panel, top right, as in Obsidian.
        let (frame, buttons) = self.panel();
        let shape = RoundedRect::from_rect(frame, size::ROUNDED_SM);
        painter.fill(shape, tm.bg_surface).draw();
        let over = self.pointer.filter(|_| self.gesture.is_none()).and_then(|p| self.panel_button_at(p));
        for (button, rect) in ZoomButton::ALL.into_iter().zip(buttons) {
            if over == Some(button) {
                painter.fill(RoundedRect::from_rect(rect, size::ROUNDED_SM), tm.border_default.multiply_alpha(0.5)).draw();
            }
            let icon = match button {
                ZoomButton::In => icons::PLUS,
                ZoomButton::Out => icons::MINUS,
                ZoomButton::Reset => icons::ROTATE_CCW,
                ZoomButton::Fit => icons::MAXIMIZE,
            };
            icon.draw(painter, Point::new(rect.x0 + (PANEL_BUTTON - ICON) / 2.0, rect.y0 + (PANEL_BUTTON - ICON) / 2.0), ICON, 2.0, tm.text_secondary);
        }
        painter.stroke(shape, &Stroke::new(1.0), tm.border_default).draw();

        let line = Self::line_height();
        if self.canvas.nodes.is_empty() && !self.readonly {
            let text = crate::i18n::t("canvas.emptyHint");
            let width = (self.size.width - size::SPACE_24 * 2.0).max(1.0);
            let layout = self.hint.layout(ctx, &text, width, Style::ui(line));
            let at = Point::new((self.size.width - f64::from(layout.width())) / 2.0, (self.size.height - line) / 2.0);
            render_text(painter, Affine::translate(at.to_vec2()), layout, &[tm.text_muted.into()], true);
        }
        if let Some(notice) = self.notice.clone().or_else(|| self.flash.clone()) {
            let width = (self.size.width - size::SPACE_24 * 2.0).max(1.0);
            let layout = self.notice_line.layout(ctx, &notice, width, Style::ui(line));
            let text_width = f64::from(layout.width());
            let pill = Rect::new((self.size.width - text_width) / 2.0 - size::SPACE_12, size::SPACE_12, (self.size.width + text_width) / 2.0 + size::SPACE_12, size::SPACE_12 + line + size::SPACE_8);
            painter.fill(RoundedRect::from_rect(pill, size::ROUNDED_SM), tm.graph_status_bg).draw();
            render_text(painter, Affine::translate((pill.x0 + size::SPACE_12, pill.y0 + size::SPACE_4)), layout, &[tm.graph_status_fg.into()], true);
        }
    }
}

fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty()).map(str::to_owned)
}
