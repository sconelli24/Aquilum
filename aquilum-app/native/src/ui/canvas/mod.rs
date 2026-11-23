//! The board of a `.canvas` file: an infinite plane with cards, groups and edges.
//!
//! Cards with content (Markdown, images) are child [`Card`] widgets laid out at their world size
//! and scaled by a per-card transform, so the rendered Markdown is the same editor the note page
//! uses. Groups, edges, link cards and far-away (stashed) cards are drawn by the board itself.

mod card;
mod input;
mod paint;
#[cfg(test)]
mod tests;
mod view;
mod widget;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use aquilum_core::canvas::{Canvas, Side};
use masonry::core::{Widget, WidgetPod};
use masonry::kurbo::{Affine, Point, Rect, Size, Vec2};

use crate::graph::camera::Camera;
use crate::ui::text::Line;

pub use card::Card;

/// The one-line editor of a prompt, framed like a card.
pub fn prompt_field(editor: masonry::core::NewWidget<crate::ui::editor::TextEditor>) -> masonry::core::NewWidget<dyn Widget> {
    Card::field(editor.erased()).erased()
}

/// Zoom range of the board.
pub const MIN_SCALE: f64 = 0.05;
pub const MAX_SCALE: f64 = 4.0;
/// Below this zoom cards are drawn as plain tiles instead of live content.
pub const DETAIL_SCALE: f64 = 0.3;
/// Live content widgets kept at once; the farthest from the centre become tiles.
pub const LIVE_CARDS: usize = 60;
const CAMERA_SETTLE: Duration = Duration::from_millis(400);
/// Pointer reach for grips, connection dots and edges, in screen pixels.
const REACH: f64 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasAction {
    /// A finished structural edit: the whole board after it.
    Edit(Canvas),
    Undo { redo: bool },
    /// The settled camera: world centre and zoom.
    Camera { x: f64, y: f64, scale: f64 },
    /// The node whose card is being edited, or none.
    Editing(Option<String>),
    /// Open a file card's file (`new_tab` with a modifier).
    Open { file: String, new_tab: bool },
    OpenUrl(String),
    /// A context menu was asked for at `at` (window coordinates) over `world`.
    Menu { at: Point, world: Point, node: Option<String>, edge: Option<String> },
    /// Ask for a line of text in place: a group or edge label, a link's address, a note name.
    Prompt { target: Prompt, text: String },
    /// The prompt was dismissed by clicking elsewhere.
    PromptClosed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prompt {
    GroupLabel(String),
    EdgeLabel(String),
    LinkUrl(String),
    /// A note to add as a card at this board point.
    Note(Point),
}

struct Asking {
    target: Prompt,
    pod: WidgetPod<dyn Widget>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    Pan { last: Point, travelled: f64, slop: f64 },
    Move { start: Point, travelled: f64, slop: f64, moved: bool, click_edit: bool },
    Resize { node: usize, grip: crate::canvas::geometry::Grip, start: Point, rect: Rect },
    Connect { from: usize, side: Side, end: Point },
    Marquee { start: Point, end: Point },
}

struct Slot {
    id: String,
    pod: WidgetPod<Card>,
    shown: bool,
    pushed: Option<Affine>,
    editing: bool,
    color: Option<Option<String>>,
}

pub struct CanvasView {
    canvas: Canvas,
    cards: Vec<Slot>,
    camera: Camera,
    stored: Option<(f64, f64, f64)>,
    fitted: bool,
    size: Size,
    selected: Vec<String>,
    edge: Option<String>,
    editing: Option<String>,
    gesture: Option<Gesture>,
    /// Node positions when a move started, for every node that moves along.
    origins: Vec<(usize, f64, f64)>,
    pointer: Option<Point>,
    moved_at: Option<Instant>,
    labels: HashMap<String, Line>,
    asking: Option<Asking>,
    notice: Option<String>,
    /// A passing message, gone with the next click.
    flash: Option<String>,
    hint: Line,
    notice_line: Line,
    readonly: bool,
}

impl CanvasView {
    fn affine(&self) -> Affine {
        let s = self.camera.scale;
        Affine::translate((self.size.width / 2.0 - self.camera.center_x * s, self.size.height / 2.0 + self.camera.center_y * s)) * Affine::scale(s)
    }

    /// Board coordinates of a point in the widget.
    fn world(&self, local: Point) -> Point {
        let s = self.camera.scale;
        Point::new((local.x - self.size.width / 2.0) / s + self.camera.center_x, (local.y - self.size.height / 2.0) / s - self.camera.center_y)
    }

    fn local(&self, world: Point) -> Point {
        self.affine() * world
    }

    fn local_rect(&self, world: Rect) -> Rect {
        self.affine().transform_rect_bbox(world)
    }

    /// Centre of the view in board coordinates.
    pub fn center(&self) -> Point {
        Point::new(self.camera.center_x, -self.camera.center_y)
    }

    fn zoom_at(&mut self, factor: f64, local: Point) {
        self.camera.zoom_by(factor, local.x - self.size.width / 2.0, local.y - self.size.height / 2.0);
        self.camera.limit(MIN_SCALE, MAX_SCALE);
    }

    fn pan(&mut self, delta: Vec2) {
        self.camera.pan_by(delta.x, delta.y);
    }

    fn fit(&mut self, bounds: Option<Rect>, glide: bool) {
        let Some(bounds) = bounds.filter(|_| self.size.width > 0.0 && self.size.height > 0.0) else { return };
        let bounds = bounds.inflate(60.0, 60.0);
        let flipped = crate::graph::Bounds { min_x: bounds.x0, min_y: -bounds.y1, max_x: bounds.x1, max_y: -bounds.y0 };
        // Never zoom past 100 % just to fill the view with a small board.
        let fit = (self.size.width / bounds.width().max(1.0)).min(self.size.height / bounds.height().max(1.0)).clamp(MIN_SCALE, 1.0);
        if glide {
            self.camera.glide_to(flipped, self.size.width, self.size.height);
            self.camera.limit(MIN_SCALE, fit.max(self.camera.scale));
        } else {
            let c = bounds.center();
            self.camera.move_to(c.x, -c.y, fit);
        }
    }

    fn bounds(&self, only: Option<&[String]>) -> Option<Rect> {
        self.canvas
            .nodes
            .iter()
            .filter(|n| only.is_none_or(|ids| ids.contains(&n.id)))
            .map(crate::canvas::geometry::rect)
            .reduce(|a, b| a.union(b))
    }

    fn moved(&mut self) {
        self.moved_at = Some(Instant::now());
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.canvas.nodes.iter().position(|n| n.id == id)
    }
}
