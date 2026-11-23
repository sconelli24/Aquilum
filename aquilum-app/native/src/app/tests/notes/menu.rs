use masonry::core::WidgetId;
use masonry::kurbo::{Point, Rect};

use super::*;
use crate::app::editor_requests::EditorCommand::{self, *};
use crate::ui::editor::TextEditor;
use crate::ui::keymap::Command;
use crate::ui::tokens::size;

pub(super) const NOTE: &str = "первое слово\n\nвторая строка\n\n";

pub(super) fn caret_at(h: &mut Harness, offset: usize) -> Rect {
    let body = h.app.note_ids.as_ref().unwrap().body;
    body.edit(&mut h.root, |mut editor| TextEditor::select_byte_range(&mut editor, offset, offset));
    let _ = h.root.redraw();
    let editor = body.get(&h.root).unwrap();
    editor.ctx().window_transform().transform_rect_bbox(editor.inner().caret_rect())
}

pub(super) fn press(h: &mut Harness, at: Point, button: PointerButton, count: u8) {
    let pointer = if button == PointerButton::Secondary { MOUSE } else { TOUCH };
    h.press(PointerState { count, ..pointer_at(at.x, at.y, 1.0) }, button, pointer);
}

pub(super) fn menu(h: &Harness) -> Option<(Rect, Vec<EditorCommand>)> {
    let (layer, _, commands) = h.app.editor_menu?;
    Some((h.root.get_widget(layer)?.ctx().bounding_box(), commands.to_vec()))
}

pub(super) fn choose(h: &mut Harness, item: usize) {
    let (layer, ..) = h.app.editor_menu.expect("menu open");
    let mut list = h.root.get_widget(layer).unwrap();
    while list.children().len() == 1 {
        list = list.children()[0];
    }
    let at = list.children()[item].ctx().bounding_box().center();
    press(h, at, PointerButton::Primary, 1);
}

pub(super) fn scrolled(h: &mut Harness, at: Point, dy: f64) {
    h.scroll((at.x, at.y), 1.0, (0.0, dy));
    h.settle();
    h.deliver();
    std::thread::sleep(crate::ui::keymap::gesture::MENU_SETTLE * 2);
    crate::ui::timer::fire(&mut h.root, std::time::Instant::now());
    h.anim(std::time::Duration::from_millis(16));
    h.deliver();
}

pub(super) fn opened(text: &str) -> (Harness, WidgetId) {
    let mut h = Harness::new();
    h.open(h.write_note("Меню.md", text));
    let editor = h.focus_editor();
    (h, editor)
}

#[test]
fn a_touch_selection_brings_up_the_edit_menu_under_it_and_its_items_keep_the_editor_focused() {
    let (mut h, editor) = opened(NOTE);
    let (start, word) = (caret_at(&mut h, 0), caret_at(&mut h, 3));
    h.press(PointerState { count: 2, ..pointer_at(word.center().x, word.center().y, 1.0) }, PointerButton::Primary, MOUSE);
    assert!(menu(&h).is_none(), "on desktop mouse selection does not open the menu, only right click");
    press(&mut h, word.center(), PointerButton::Primary, 2);
    let (frame, commands) = menu(&h).expect("double tap on a word did not open the menu");
    assert_eq!(commands, [Cut, Copy, Paste, SelectAll]);
    let below = word.y1 + size::SELECTION_HANDLE_SIZE;
    assert!(frame.y0 >= below && frame.y0 <= below + size::SPACE_8, "menu {frame:?} is not under the line handles {word:?}");
    assert!((frame.x0 - start.x0).abs() <= size::CARET_WIDTH, "menu {frame:?} does not start at the selection start {start:?}");
    choose(&mut h, 1);
    assert_eq!(h.copied.as_deref(), Some("первое"));
    assert!(menu(&h).is_none(), "menu remained after a choice");
    assert_eq!(h.root.focused_widget(), Some(editor), "pressing a menu item took focus from the editor");

    let from = NOTE.find("вторая").unwrap();
    let (a, b) = (caret_at(&mut h, from), caret_at(&mut h, from + "вторая".len()));
    h.drag_with((a.x0 + 1.0, a.center().y), (b.x0, b.center().y), TOUCH);
    assert!(menu(&h).is_some(), "finger drag did not open the menu");
    choose(&mut h, 0);
    assert_eq!(h.copied.as_deref(), Some("вторая"));
    assert_eq!(h.body_text(), "первое слово\n\n строка\n\n");
}

#[test]
fn right_click_and_a_double_tap_on_empty_space_offer_paste_and_inserts() {
    let (mut h, _) = opened(NOTE);
    let caret = [Paste, SelectAll, InsertBook, InsertTable, InsertFrontmatter];
    let (empty, word, second) = (caret_at(&mut h, NOTE.len()), caret_at(&mut h, 3), caret_at(&mut h, NOTE.find("вторая").unwrap() + 4));
    press(&mut h, empty.center(), PointerButton::Secondary, 1);
    let (frame, commands) = menu(&h).expect("right click did not open the menu");
    assert_eq!(commands, caret);
    assert!(frame.y0 >= empty.center().y && frame.y0 <= empty.center().y + size::SPACE_8, "menu {frame:?} is not at the click point {empty:?}");
    choose(&mut h, 1);
    let body = h.app.note_ids.as_ref().unwrap().body;
    assert_eq!(body.get(&h.root).unwrap().inner().selection(), 0..NOTE.len(), "Select all did not select the note");

    press(&mut h, second.center(), PointerButton::Primary, 1);
    press(&mut h, second.center(), PointerButton::Primary, 2);
    assert_eq!(menu(&h).map(|(_, commands)| commands.len()), Some(4), "double tap on a word did not open the selection menu");
    press(&mut h, word.center(), PointerButton::Secondary, 1);
    assert_eq!(menu(&h).map(|(_, commands)| commands), Some(caret.to_vec()), "right click outside the selection did not clear it");
    press(&mut h, empty.center(), PointerButton::Primary, 1);
    assert!(menu(&h).is_none(), "tap did not close the menu");
    press(&mut h, empty.center(), PointerButton::Primary, 2);
    assert_eq!(menu(&h).map(|(_, commands)| commands), Some(caret.to_vec()), "long press on an empty spot did not offer paste");
}

#[test]
fn the_edit_menu_closes_on_tap_escape_back_focus_loss_selection_change_and_typing() {
    let (mut h, editor) = opened(NOTE);
    let (word, elsewhere) = (caret_at(&mut h, 3), caret_at(&mut h, NOTE.find("слово").unwrap() + 4));
    let closers: [(&str, &dyn Fn(&mut Harness)); 6] = [
        ("tap elsewhere", &|h| press(h, elsewhere.center(), PointerButton::Primary, 1)),
        ("Escape", &|h| _ = h.escape()),
        ("Back", &|h| _ = h.app.command(&mut h.root, Command::Back)),
        ("window focus loss", &|h| _ = h.root.handle_text_event(TextEvent::WindowFocusChange(false))),
        ("selection change", &|h| _ = h.key(Key::Named(NamedKey::ArrowRight), Modifiers::SHIFT)),
        ("typing", &|h| h.type_text("Ж")),
    ];
    for (what, close) in closers {
        h.root.handle_text_event(TextEvent::WindowFocusChange(true));
        h.root.focus_on(Some(editor));
        press(&mut h, word.center(), PointerButton::Primary, 2);
        assert!(menu(&h).is_some(), "{what}: menu did not open");
        let before = h.body_text();
        close(&mut h);
        h.deliver();
        assert!(menu(&h).is_none(), "{what}: menu did not close");
        if what != "typing" {
            assert_eq!(h.body_text(), before, "{what}: text changed");
        }
    }
    assert_eq!(h.body_text(), "Ж слово\n\nвторая строка\n\n");
}

#[test]
fn the_edit_menu_stays_with_the_selection_while_scrolling_and_works_in_the_title() {
    let (mut h, _) = opened(&format!("{NOTE}{}", "абзац\n\n".repeat(60)));
    let word = caret_at(&mut h, 3);
    press(&mut h, word.center(), PointerButton::Primary, 2);
    let (before, offset) = (menu(&h).expect("menu did not open").0, note_offset(&h));
    let below = Point::new(word.x0, word.y1 + 200.0);
    scrolled(&mut h, below, -60.0);
    let moved = note_offset(&h) - offset;
    let (after, _) = menu(&h).expect("menu did not return to the selection after scrolling");
    assert!(moved > 10.0, "page did not scroll: {moved}");
    assert!((before.y0 - after.y0 - moved).abs() < 1.0, "menu {after:?} did not move with the selection by {moved} from {before:?}");
    choose(&mut h, 1);
    scrolled(&mut h, below, 60.0);
    assert!(menu(&h).is_none(), "closed menu came back on scroll");

    let title = h.app.note_ids.as_ref().unwrap().title;
    let frame = title.get(&h.root).unwrap().ctx().bounding_box();
    press(&mut h, Point::new(frame.x0 + 12.0, frame.center().y), PointerButton::Primary, 2);
    assert_eq!(menu(&h).map(|(_, commands)| commands), Some(vec![Cut, Copy, Paste, SelectAll]), "double tap on the title did not open the menu");
    choose(&mut h, 1);
    assert_eq!(h.copied.as_deref(), Some("Меню"));
    assert_eq!(h.root.focused_widget(), Some(title.id()), "menu took focus away from the title");
}

#[test]
fn near_the_bottom_right_the_edit_menu_opens_above_and_stays_on_screen() {
    let mut h = super::super::adaptive::at_width(360.0);
    let bars = masonry::kurbo::Insets::new(0.0, 24.0, 0.0, 48.0);
    h.app.fit_window(&mut h.root, h.app.window, bars);
    h.open(h.write_note("Длинная.md", &"слово ".repeat(3000)));
    let body = h.app.note_ids.as_ref().unwrap().body;
    h.root.focus_on(Some(body.id()));
    let right = body.get(&h.root).unwrap().ctx().bounding_box().x1;
    let y = f64::from(H) - bars.y1 - 30.0;
    press(&mut h, Point::new(right - 30.0, y), PointerButton::Primary, 2);
    let (frame, _) = menu(&h).expect("double tap near the bottom edge did not open the menu");
    let editor = body.get(&h.root).unwrap();
    let line = editor.ctx().window_transform().transform_rect_bbox(editor.inner().caret_rect());
    assert!(frame.y1 <= line.y0, "menu {frame:?} did not rise above the selection {line:?} at the edge");
    assert!(frame.x0 >= size::SPACE_8 && frame.x1 <= 360.0 - size::SPACE_8 + 0.5, "menu {frame:?} went off the screen");
    assert!(frame.y0 >= bars.y0 + size::SPACE_8, "menu {frame:?} went under the status bar");
}

#[test]
fn keyboard_menu_actions_cut_copy_and_select_all_in_the_focused_field() {
    let (mut h, editor) = opened(NOTE);
    let body = h.app.note_ids.as_ref().unwrap().body;
    body.edit(&mut h.root, |mut e| TextEditor::select_byte_range(&mut e, "первое ".len(), "первое слово".len()));
    h.app.text_command(&mut h.root, editor, Command::Copy);
    h.deliver();
    assert_eq!(h.copied.as_deref(), Some("слово"));
    h.app.text_command(&mut h.root, editor, Command::Cut);
    h.deliver();
    assert_eq!(h.body_text(), "первое \n\nвторая строка\n\n");
    h.app.text_command(&mut h.root, editor, Command::SelectAll);
    assert_eq!(body.get(&h.root).unwrap().inner().selection(), 0..h.body_text().len());
}
