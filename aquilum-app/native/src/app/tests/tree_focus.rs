use super::tree_filter::{bounds, field, names, start};
use super::*;
use crate::test_dir::{TOUCH, pointer_at};
use crate::ui::tokens::size;
use masonry::core::pointer::PointerButtons;
use masonry::core::{PointerButton, PointerButtonEvent, PointerEvent, PointerUpdate};
use masonry::kurbo::Point;

fn finger(h: &mut Harness, at: Point) {
    let state = |buttons| masonry::core::PointerState { buttons, ..pointer_at(at.x, at.y, 1.0) };
    let button = |state| PointerButtonEvent { pointer: TOUCH, button: Some(PointerButton::Primary), state };
    h.root.handle_pointer_event(PointerEvent::Move(PointerUpdate { pointer: TOUCH, current: state(PointerButtons::new()), coalesced: vec![], predicted: vec![] }));
    h.app.pointer_event(&mut h.root, PointerEvent::Down(button(state(PointerButton::Primary.into()))));
    h.root.handle_pointer_event(PointerEvent::Up(button(state(PointerButtons::new()))));
    h.root.handle_pointer_event(PointerEvent::Leave(TOUCH));
}

fn hit(h: &mut Harness, at: Point, touch: bool) {
    if touch {
        finger(h, at);
        h.deliver();
    } else {
        h.click(at.x, at.y);
    }
}

fn probes(h: &Harness) -> Vec<(&'static str, Point)> {
    let [new_note, new_folder, _, _] = h.app.tree.toolbar_ids().unwrap();
    let (new_note, new_folder) = (bounds(h, new_note), bounds(h, new_folder));
    let input = bounds(h, field(h));
    let scroll = bounds(h, h.app.tree.scroll_id().unwrap());
    vec![
        ("padding above the buttons", Point::new(new_note.center().x, new_note.y0 - size::SPACE_8 / 2.0)),
        ("gap between the buttons", Point::new((new_note.x1 + new_folder.x0) / 2.0, new_note.center().y)),
        ("gap between the buttons and the field", Point::new(input.center().x, (new_note.y1 + input.y0 - size::INPUT_XS_PADDING_Y) / 2.0)),
        ("padding below the field", Point::new(input.center().x, input.y1 + size::INPUT_XS_PADDING_Y + size::SPACE_8 / 2.0)),
        ("strip left of the field", Point::new(scroll.x0 + 2.0, input.center().y)),
        ("strip right of the field", Point::new(scroll.x1 - 2.0, input.center().y)),
        ("tree row", bounds(h, h.app.tree.row_id("Кофе").unwrap()).center()),
        ("empty area of the tree", Point::new(scroll.center().x, scroll.y1 - 20.0)),
        ("right part of the window", Point::new(700.0, 300.0)),
    ]
}

#[test]
fn a_click_or_a_tap_elsewhere_drops_the_focus_and_keeps_the_filter() {
    for touch in [false, true] {
        let (_vault, mut h) = start();
        let field = field(&h);
        h.app.tree.filter(&mut h.root, "коф");
        assert!(h.wait_for(|h| names(h) == ["Проекты", "Кофемолка", "Кофе"]));
        for (what, at) in probes(&h) {
            let center = bounds(&h, field).center();
            hit(&mut h, center, touch);
            assert_eq!(h.root.focused_widget(), Some(field), "{what}: field did not get focus (touch: {touch})");
            hit(&mut h, at, touch);
            assert_ne!(h.root.focused_widget(), Some(field), "{what}: focus stayed in the field (touch: {touch})");
            assert_eq!(h.app.tree.filter_text(&h.root), "коф", "{what}: search text disappeared");
            assert!(!names(&h).is_empty() && names(&h).len() < 4, "{what}: filter was reset: {:?}", names(&h));
        }
    }
}

#[test]
fn a_press_inside_the_search_box_keeps_the_focus() {
    for touch in [false, true] {
        let (_vault, mut h) = start();
        let field = field(&h);
        let center = bounds(&h, field).center();
        hit(&mut h, center, touch);
        let input = bounds(&h, field);
        let edge = Point::new(input.center().x, input.y0 - size::INPUT_XS_PADDING_Y / 2.0);
        hit(&mut h, edge, touch);
        assert_eq!(h.root.focused_widget(), Some(field), "press on padding inside the search block removed focus (touch: {touch})");
        hit(&mut h, input.center(), touch);
        assert_eq!(h.root.focused_widget(), Some(field), "press on the field itself removed focus (touch: {touch})");
    }
}

#[test]
fn losing_the_focus_hides_the_keyboard_and_getting_it_shows_it() {
    let (_vault, mut h) = start();
    let field = field(&h);
    let seen = |h: &Harness, wanted: fn(&RenderRootSignal) -> bool| h.signals.borrow().iter().any(wanted);
    h.signals.borrow_mut().clear();
    let center = bounds(&h, field).center();
    finger(&mut h, center);
    let _ = h.root.redraw();
    assert!(seen(&h, |signal| matches!(signal, RenderRootSignal::StartIme)), "tap on the field did not bring up the keyboard");
    let row = bounds(&h, h.app.tree.row_id("Кофе").unwrap()).center();
    h.signals.borrow_mut().clear();
    finger(&mut h, row);
    let _ = h.root.redraw();
    assert!(seen(&h, |signal| matches!(signal, RenderRootSignal::EndIme)), "tap outside the field did not hide the keyboard");
}
