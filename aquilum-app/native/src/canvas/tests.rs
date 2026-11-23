use aquilum_core::canvas::{Node, Side};
use masonry::kurbo::{Point, Rect, Vec2};

use super::geometry::*;

#[test]
fn colours_accept_presets_and_hex() {
    assert!(color(Some("1")).is_some());
    assert!(color(Some("6")).is_some());
    assert!(color(Some("7")).is_none());
    assert!(color(Some("0")).is_none());
    assert_eq!(color(Some("#fff")), color(Some("#ffffff")));
    assert!(color(Some("#12345")).is_none());
    assert!(color(None).is_none());
}

#[test]
fn edges_without_sides_face_each_other() {
    let left = Rect::new(0.0, 0.0, 100.0, 100.0);
    let right = Rect::new(300.0, 0.0, 400.0, 100.0);
    let curve = curve(left, None, right, None);
    assert_eq!((curve.from_side, curve.to_side), (Side::Right, Side::Left));
    assert_eq!(curve.cubic.p0, Point::new(100.0, 50.0));
    assert_eq!(curve.cubic.p3, Point::new(300.0, 50.0));
    assert!(curve.distance(Point::new(200.0, 50.0)) < 1.0);
    assert!(curve.distance(Point::new(200.0, 200.0)) > 100.0);
    let below = Rect::new(0.0, 300.0, 100.0, 400.0);
    assert_eq!(facing(left, below.center()), Side::Bottom);
}

#[test]
fn resizing_keeps_a_minimum_size() {
    let rect = Rect::new(0.0, 0.0, 200.0, 100.0);
    assert_eq!(Grip::SE.resize(rect, Vec2::new(50.0, 20.0)), Rect::new(0.0, 0.0, 250.0, 120.0));
    assert_eq!(Grip::NW.resize(rect, Vec2::new(10.0, 10.0)), Rect::new(10.0, 10.0, 200.0, 100.0));
    let squashed = Grip::E.resize(rect, Vec2::new(-500.0, 0.0));
    assert_eq!(squashed.width(), MIN_WIDTH);
    assert_eq!(grip_at(rect, Point::new(199.0, 99.0), 6.0), Some(Grip::SE));
    assert_eq!(side_dot(rect, Point::new(100.0, -1.0), 6.0), Some(Side::Top));
}

#[test]
fn cards_win_over_the_groups_around_them() {
    let nodes = vec![Node::group(None, 0.0, 0.0, 500.0, 500.0), Node::text("a", 10.0, 10.0, 100.0, 100.0), Node::text("b", 50.0, 50.0, 100.0, 100.0)];
    assert_eq!(node_at(&nodes, Point::new(60.0, 60.0)), Some(2));
    assert_eq!(node_at(&nodes, Point::new(20.0, 20.0)), Some(1));
    assert_eq!(node_at(&nodes, Point::new(400.0, 400.0)), Some(0));
    assert_eq!(node_at(&nodes, Point::new(600.0, 400.0)), None);
    assert_eq!(snap(29.0), 20.0);
}
