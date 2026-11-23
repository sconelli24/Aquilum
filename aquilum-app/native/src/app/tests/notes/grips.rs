use masonry::core::WidgetId;
use masonry::kurbo::{Point, Vec2};

use super::*;
use super::menu::{NOTE, caret_at, choose, menu, opened, press, scrolled};
use crate::app::editor_requests::EditorCommand::*;
use crate::ui::editor::TextEditor;
use crate::ui::keymap::gesture::TOUCH_REACH;
use crate::ui::tokens::size;

fn editor(h: &Harness, id: WidgetId) -> &TextEditor {
    h.root.get_widget(id).unwrap().downcast::<TextEditor>().unwrap().inner()
}

fn grips(h: &Harness, id: WidgetId) -> [Option<Point>; 2] {
    let center = Vec2::splat(TOUCH_REACH + size::SELECTION_HANDLE_SIZE / 2.0);
    editor(h, id).grips_at().map(|at| at.map(|at| at + center))
}

#[test]
fn a_touch_selection_gets_grips_that_move_its_ends_and_bring_the_menu_back() {
    let (mut h, body) = opened(NOTE);
    let (start, end) = (caret_at(&mut h, 0), caret_at(&mut h, "первое".len()));
    let (second, last) = (caret_at(&mut h, "первое ".len()), caret_at(&mut h, "первое слово".len()));
    let word = caret_at(&mut h, 3);
    h.press(PointerState { count: 2, ..pointer_at(word.center().x, word.center().y, 1.0) }, PointerButton::Primary, MOUSE);
    assert_eq!(grips(&h, body), [None, None], "mouse selection produced handles");

    press(&mut h, word.center(), PointerButton::Primary, 2);
    assert_eq!(editor(&h, body).selected(), "первое");
    let [Some(left), Some(right)] = grips(&h, body) else { panic!("finger selection did not show handles") };
    assert!(left.x < start.x0 && right.x > end.x0 && left.y > start.y1 && right.y > end.y1, "handles {left:?} {right:?} are not under the ends {start:?} {end:?}");
    let (frame, _) = menu(&h).expect("menu did not open");
    assert!(frame.y0 >= end.y1 + size::SELECTION_HANDLE_SIZE, "menu {frame:?} covers the handles");

    h.drag_with((right.x, right.y), (right.x + last.x0 - end.x0, right.y + 3.0), TOUCH);
    assert_eq!(editor(&h, body).selected(), "первое слово", "right handle did not extend the selection far enough");
    assert!(menu(&h).is_some(), "menu did not come back after the handle");
    let [Some(left), Some(_)] = grips(&h, body) else { panic!("handles disappeared after the drag") };
    h.drag_with((left.x, left.y), (left.x + second.x0 - start.x0, left.y - 2.0), TOUCH);
    assert_eq!(editor(&h, body).selected(), "слово", "left handle did not move the start");

    h.type_text("Ж");
    assert_eq!(h.body_text(), "первое Ж\n\nвторая строка\n\n");
    assert_eq!(grips(&h, body), [None, None], "handles remained after typing");
}

#[test]
fn select_all_keeps_the_menu_where_it_was_for_touch_and_mouse() {
    let (mut h, body) = opened(&format!("# Заголовок\n\n{NOTE}"));
    let (word, empty) = (caret_at(&mut h, "# Заголовок\n\n".len() + 3), caret_at(&mut h, "# Заголовок\n\n".len() + NOTE.len()));
    press(&mut h, word.center(), PointerButton::Primary, 2);
    let (before, _) = menu(&h).expect("menu did not open");
    choose(&mut h, 3);
    assert_eq!(editor(&h, body).selection().len(), h.body_text().len(), "Select all did not select the note");
    let (after, commands) = menu(&h).expect("menu disappeared after Select all");
    assert_eq!((after, commands), (before, vec![Cut, Copy, Paste, SelectAll]), "menu moved after Select all");
    assert!(grips(&h, body).iter().all(Option::is_some), "no handles at the ends of the note");

    press(&mut h, word.center(), PointerButton::Primary, 1);
    press(&mut h, empty.center(), PointerButton::Secondary, 1);
    let (before, commands) = menu(&h).expect("right click did not open the menu");
    assert_eq!(commands[1], SelectAll);
    choose(&mut h, 1);
    let (after, commands) = menu(&h).expect("right-click menu disappeared after Select all");
    assert_eq!((after.origin(), commands), (before.origin(), vec![Cut, Copy, Paste, SelectAll]), "right-click menu moved");
    assert_eq!(grips(&h, body), [None, None], "mouse produced handles");
}

#[test]
fn grips_follow_scrolling_and_hide_outside_the_page() {
    let (mut h, body) = opened(&format!("{NOTE}{}", "абзац\n\n".repeat(80)));
    let word = caret_at(&mut h, 3);
    press(&mut h, word.center(), PointerButton::Primary, 2);
    let [Some(before), _] = grips(&h, body) else { panic!("no handles") };
    let (below, offset) = (Point::new(word.x0, word.y1 + 200.0), note_offset(&h));
    scrolled(&mut h, below, -40.0);
    let moved = note_offset(&h) - offset;
    let [Some(after), _] = grips(&h, body) else { panic!("handles disappeared on scroll") };
    assert!(moved > 10.0 && (before.y - after.y - moved).abs() < 1.0, "handle {after:?} did not move with the text by {moved} from {before:?}");
    scrolled(&mut h, below, -2000.0);
    assert_eq!(grips(&h, body), [None, None], "handles visible when the selection left the page");
    scrolled(&mut h, below, 4000.0);
    assert!(grips(&h, body).iter().all(Option::is_some), "handles did not come back with the selection");
}

#[test]
fn title_and_single_line_grips_take_the_finger_over_other_widgets() {
    let (mut h, _) = opened(NOTE);
    let title = h.app.note_ids.as_ref().unwrap().title.id();
    let frame = h.root.get_widget(title).unwrap().ctx().bounding_box();
    press(&mut h, Point::new(frame.x0 + 12.0, frame.center().y), PointerButton::Primary, 2);
    assert_eq!(editor(&h, title).selected(), "Меню");
    let [Some(left), Some(right)] = grips(&h, title) else { panic!("no handles in the title") };
    h.drag_with((right.x, right.y), ((left.x + right.x) / 2.0, right.y), TOUCH);
    let shrunk = editor(&h, title).selected();
    assert!(!shrunk.is_empty() && shrunk.len() < "Меню".len() && "Меню".starts_with(&shrunk), "title handle did not shrink the selection: {shrunk:?}");
    assert_eq!(h.root.focused_widget(), Some(title), "handle took focus away from the title");
    assert_eq!(h.body_text(), NOTE);

    h.app.open_find(&mut h.root);
    h.type_text("слово тут");
    let field = h.app.find.as_ref().unwrap().field.id();
    let frame = h.root.get_widget(field).unwrap().ctx().bounding_box();
    press(&mut h, Point::new(frame.x0 + 8.0, frame.center().y), PointerButton::Primary, 2);
    assert_eq!(editor(&h, field).selected(), "слово");
    let [Some(_), Some(right)] = grips(&h, field) else { panic!("no handles in the search field") };
    h.drag_with((right.x, right.y), (frame.x1 - 2.0, right.y), TOUCH);
    assert_eq!(editor(&h, field).selected(), "слово тут", "search field handle did not extend the selection far enough");
}

#[test]
fn a_dragged_grip_holds_its_line_until_the_finger_leaves_it_by_half_a_line() {
    let (mut h, body) = opened(NOTE);
    let (end, next) = (caret_at(&mut h, "первое".len()), caret_at(&mut h, NOTE.find("вторая").unwrap() + 2));
    let word = caret_at(&mut h, 3);
    press(&mut h, word.center(), PointerButton::Primary, 2);
    let [_, Some(right)] = grips(&h, body) else { panic!("no right handle") };
    h.drag_with((right.x, right.y), (right.x, right.y + end.height() * 0.4), TOUCH);
    assert_eq!(editor(&h, body).selected(), "первое", "handle slipped off the line from finger jitter");
    let [_, Some(right)] = grips(&h, body) else { panic!("handle disappeared") };
    h.drag_with((right.x, right.y), (right.x + next.x0 - end.x0, right.y + next.center().y + next.height() * 0.25 - end.center().y), TOUCH);
    assert!(editor(&h, body).selected().starts_with("первое слово\n\nв"), "handle did not move to the line under the finger: {:?}", editor(&h, body).selected());
}

#[cfg(target_os = "linux")]
#[test]
fn a_mouse_selection_fills_the_primary_selection_and_a_middle_click_pastes_it() {
    let (mut h, _) = opened(NOTE);
    let (word, end) = (caret_at(&mut h, 3), caret_at(&mut h, NOTE.len()));
    h.press(PointerState { count: 2, ..pointer_at(word.center().x, word.center().y, 1.0) }, PointerButton::Primary, MOUSE);
    assert_eq!(crate::clipboard::selection().as_deref(), Some("первое"));
    h.press(pointer_at(end.center().x, end.center().y, 1.0), PointerButton::Auxiliary, MOUSE);
    assert_eq!(h.body_text(), format!("{NOTE}первое"));
}
