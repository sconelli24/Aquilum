//! Board geometry in world units (JSON Canvas pixels at 100 % zoom, y pointing down).

use aquilum_core::canvas::{Node, Side};
use masonry::kurbo::{BezPath, CubicBez, ParamCurve, ParamCurveNearest, Point, Rect, Vec2};
use masonry::peniko::Color;

pub const MIN_WIDTH: f64 = 60.0;
pub const MIN_HEIGHT: f64 = 40.0;
pub const GRID: f64 = 20.0;

/// The six preset colours of JSON Canvas: red, orange, yellow, green, cyan, purple.
const PRESETS: [[u8; 3]; 6] = [[0xe9, 0x31, 0x47], [0xec, 0x75, 0x00], [0xe0, 0xac, 0x00], [0x08, 0xb9, 0x4e], [0x00, 0xbf, 0xbc], [0x78, 0x52, 0xee]];

pub fn color(spec: Option<&str>) -> Option<Color> {
    let spec = spec?.trim();
    if let Ok(index) = spec.parse::<usize>() {
        let [r, g, b] = *PRESETS.get(index.checked_sub(1)?)?;
        return Some(Color::from_rgb8(r, g, b));
    }
    let hex = spec.strip_prefix('#')?;
    let digit = |i: usize| u8::from_str_radix(hex.get(i..i + 1)?, 16).ok();
    let pair = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        3 => Some(Color::from_rgb8(digit(0)? * 17, digit(1)? * 17, digit(2)? * 17)),
        6 => Some(Color::from_rgb8(pair(0)?, pair(2)?, pair(4)?)),
        _ => None,
    }
}

pub fn rect(node: &Node) -> Rect {
    Rect::new(node.x, node.y, node.x + node.width.max(1.0), node.y + node.height.max(1.0))
}

pub fn anchor(rect: Rect, side: Side) -> Point {
    let c = rect.center();
    match side {
        Side::Top => Point::new(c.x, rect.y0),
        Side::Right => Point::new(rect.x1, c.y),
        Side::Bottom => Point::new(c.x, rect.y1),
        Side::Left => Point::new(rect.x0, c.y),
    }
}

fn normal(side: Side) -> Vec2 {
    match side {
        Side::Top => Vec2::new(0.0, -1.0),
        Side::Right => Vec2::new(1.0, 0.0),
        Side::Bottom => Vec2::new(0.0, 1.0),
        Side::Left => Vec2::new(-1.0, 0.0),
    }
}

/// The side of `rect` that faces `toward`, used when an edge names no side.
pub fn facing(rect: Rect, toward: Point) -> Side {
    let d = toward - rect.center();
    let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
    if (d.x / w).abs() >= (d.y / h).abs() {
        if d.x >= 0.0 { Side::Right } else { Side::Left }
    } else if d.y >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

/// An edge as drawn: a cubic leaving each card perpendicular to its side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub cubic: CubicBez,
    pub from_side: Side,
    pub to_side: Side,
}

pub fn curve(from: Rect, from_side: Option<Side>, to: Rect, to_side: Option<Side>) -> Curve {
    let from_side = from_side.unwrap_or_else(|| facing(from, to.center()));
    let to_side = to_side.unwrap_or_else(|| facing(to, from.center()));
    curve_to(anchor(from, from_side), from_side, anchor(to, to_side), Some(to_side))
}

/// A curve from a card side to a free point (while dragging out a new edge) or another side.
pub fn curve_to(start: Point, from_side: Side, end: Point, to_side: Option<Side>) -> Curve {
    let reach = ((end - start).hypot() * 0.4).clamp(40.0, 300.0);
    let to = to_side.unwrap_or_else(|| facing(Rect::from_center_size(end, (1.0, 1.0)), start));
    let c1 = start + normal(from_side) * reach;
    let c2 = match to_side {
        Some(side) => end + normal(side) * reach,
        None => end + (start - end).normalize() * reach * 0.5,
    };
    Curve { cubic: CubicBez::new(start, c1, c2, end), from_side, to_side: to }
}

impl Curve {
    pub fn path(&self) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(self.cubic.p0);
        path.curve_to(self.cubic.p1, self.cubic.p2, self.cubic.p3);
        path
    }

    pub fn middle(&self) -> Point {
        self.cubic.eval(0.5)
    }

    pub fn distance(&self, p: Point) -> f64 {
        self.cubic.nearest(p, 0.5).distance_sq.sqrt()
    }

    /// A filled triangle at the end (`at_end`) or the start, pointing along the curve.
    pub fn arrow(&self, at_end: bool, size: f64) -> BezPath {
        let (tip, from) = if at_end { (self.cubic.p3, self.cubic.eval(0.97)) } else { (self.cubic.p0, self.cubic.eval(0.03)) };
        let dir = (tip - from).normalize();
        let dir = if dir.x.is_finite() { dir } else { Vec2::new(1.0, 0.0) };
        let side = Vec2::new(-dir.y, dir.x) * (size * 0.5);
        let base = tip - dir * size;
        let mut path = BezPath::new();
        path.move_to(tip);
        path.line_to(base + side);
        path.line_to(base - side);
        path.close_path();
        path
    }
}

pub fn snap(value: f64) -> f64 {
    (value / GRID).round() * GRID
}

/// Where a pointer grabbed a selected card's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Grip {
    pub const ALL: [Grip; 8] = [Grip::NW, Grip::N, Grip::NE, Grip::E, Grip::SE, Grip::S, Grip::SW, Grip::W];

    pub fn point(self, rect: Rect) -> Point {
        let c = rect.center();
        match self {
            Grip::N => Point::new(c.x, rect.y0),
            Grip::S => Point::new(c.x, rect.y1),
            Grip::E => Point::new(rect.x1, c.y),
            Grip::W => Point::new(rect.x0, c.y),
            Grip::NE => Point::new(rect.x1, rect.y0),
            Grip::NW => Point::new(rect.x0, rect.y0),
            Grip::SE => Point::new(rect.x1, rect.y1),
            Grip::SW => Point::new(rect.x0, rect.y1),
        }
    }

    pub fn cursor(self) -> masonry::core::CursorIcon {
        use masonry::core::CursorIcon as C;
        match self {
            Grip::N | Grip::S => C::NsResize,
            Grip::E | Grip::W => C::EwResize,
            Grip::NE | Grip::SW => C::NeswResize,
            Grip::NW | Grip::SE => C::NwseResize,
        }
    }

    /// The rectangle after dragging this grip by `delta`, never smaller than the minimum.
    pub fn resize(self, rect: Rect, delta: Vec2) -> Rect {
        let (mut x0, mut y0, mut x1, mut y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
        match self {
            Grip::N | Grip::NE | Grip::NW => y0 = (y0 + delta.y).min(y1 - MIN_HEIGHT),
            Grip::S | Grip::SE | Grip::SW => y1 = (y1 + delta.y).max(y0 + MIN_HEIGHT),
            _ => {}
        }
        match self {
            Grip::W | Grip::NW | Grip::SW => x0 = (x0 + delta.x).min(x1 - MIN_WIDTH),
            Grip::E | Grip::NE | Grip::SE => x1 = (x1 + delta.x).max(x0 + MIN_WIDTH),
            _ => {}
        }
        Rect::new(x0, y0, x1, y1)
    }
}

pub fn grip_at(rect: Rect, p: Point, reach: f64) -> Option<Grip> {
    Grip::ALL.into_iter().find(|g| (g.point(rect) - p).hypot() <= reach)
}

/// The connection dot on a side under the pointer, for dragging out a new edge.
pub fn side_dot(rect: Rect, p: Point, reach: f64) -> Option<Side> {
    [Side::Top, Side::Right, Side::Bottom, Side::Left].into_iter().find(|&s| (anchor(rect, s) - p).hypot() <= reach)
}

/// Topmost node under `p`: cards above groups, later nodes above earlier ones.
pub fn node_at(nodes: &[Node], p: Point) -> Option<usize> {
    let hit = |group: bool| nodes.iter().enumerate().rev().find(|(_, n)| (n.kind() == aquilum_core::canvas::NodeKind::Group) == group && rect(n).contains(p)).map(|(i, _)| i);
    hit(false).or_else(|| hit(true))
}
