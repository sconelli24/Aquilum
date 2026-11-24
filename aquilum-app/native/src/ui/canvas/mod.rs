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

/// Buttons of the floating zoom panel, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ZoomButton {
    In,
    Out,
    Reset,
    Fit,
}

impl ZoomButton {
    const ALL: [ZoomButton; 4] = [ZoomButton::In, ZoomButton::Out, ZoomButton::Reset, ZoomButton::Fit];
}

/// Side of a panel button and the gap to the board's corner, in screen pixels.
const PANEL_BUTTON: f64 = 32.0;
const PANEL_MARGIN: f64 = 12.0;
/// Zoom step of the panel's + and − buttons.
const PANEL_STEP: f64 = 1.25;

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
    /// Space is held: a left drag pans the board, as in Obsidian.
    space: bool,
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

    /// The panel's frame and the square of each button, in the widget's coordinates.
    fn panel(&self) -> (Rect, [Rect; 4]) {
        let x1 = self.size.width - PANEL_MARGIN;
        let y0 = PANEL_MARGIN;
        let buttons = std::array::from_fn(|i| {
            let top = y0 + PANEL_BUTTON * i as f64;
            Rect::new(x1 - PANEL_BUTTON, top, x1, top + PANEL_BUTTON)
        });
        (Rect::new(x1 - PANEL_BUTTON, y0, x1, y0 + PANEL_BUTTON * 4.0), buttons)
    }

    fn panel_button_at(&self, local: Point) -> Option<ZoomButton> {
        let (_, buttons) = self.panel();
        ZoomButton::ALL.into_iter().zip(buttons).find_map(|(button, rect)| rect.contains(local).then_some(button))
    }

    /// Does what a panel button says; the view zooms around its centre.
    fn press_zoom_button(&mut self, button: ZoomButton) {
        let centre = Point::new(self.size.width / 2.0, self.size.height / 2.0);
        match button {
            ZoomButton::In => self.zoom_at(PANEL_STEP, centre),
            ZoomButton::Out => self.zoom_at(1.0 / PANEL_STEP, centre),
            ZoomButton::Reset => self.zoom_at(1.0 / self.camera.scale, centre),
            ZoomButton::Fit => {
                let bounds = self.bounds(None);
                self.fit(bounds, true);
            }
        }
        self.moved();
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
