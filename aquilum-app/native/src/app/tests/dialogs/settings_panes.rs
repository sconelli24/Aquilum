use masonry::core::WidgetId;
use masonry::kurbo::Insets;

use super::super::adaptive::{at_width, bounds, hidden, resize};
use super::*;
use crate::ui::editor::TextEditor;
use crate::ui::keymap::Command;
use crate::ui::settings::{Binding, SettingsDialog};
use crate::ui::tokens::size;

const LIST: [bool; 4] = [true, false, false, true];
const DETAIL: [bool; 4] = [false, true, true, false];
const BOTH: [bool; 4] = [true, true, false, true];

fn dialog(h: &Harness) -> &SettingsDialog {
    match &h.app.popup {
        Some(Popup::Settings { dialog, .. }) => dialog,
        _ => panic!("settings not open"),
    }
}

fn opened(width: f64) -> Harness {
    let mut h = at_width(width);
    h.app.open_settings(&mut h.root);
    h.settle();
    h
}

fn shown(h: &Harness) -> [bool; 4] {
    let d = dialog(h);
    [d.list.id(), d.content.id(), d.back, d.close].map(|id| !hidden(h, id))
}

fn press(h: &mut Harness, id: WidgetId) {
    let center = bounds(h, id).center();
    h.click(center.x, center.y);
    h.settle();
}

fn back(h: &mut Harness) {
    h.app.command(&mut h.root, Command::Back).expect("Back handled");
    h.deliver();
    h.settle();
}

#[test]
fn phone_settings_open_on_the_section_list_and_return_to_it() {
    let mut h = opened(360.0);
    assert_eq!(shown(&h), LIST, "section list first");
    let panes = bounds(&h, dialog(&h).panes.id());
    assert_eq!((panes.x0, panes.y0, panes.x1, panes.y1), (0.0, 0.0, 360.0, f64::from(H)), "full screen");
    let row = bounds(&h, h.nav(Section::Editor));
    let wide = 360.0 - 2.0 * size::SPACE_8 - size::SCROLLBAR_WIDTH;
    assert_eq!((row.height(), row.width()), (size::SIZE_48, wide), "large row at full width, except the scrollbar");

    h.open_section(Section::Editor);
    assert_eq!((h.section(), shown(&h)), (Some(Section::Editor), DETAIL), "section instead of the list");
    assert_eq!(dialog(&h).title.get(&h.root).unwrap().inner().text(), Section::Editor.label());
    assert_eq!(bounds(&h, dialog(&h).content.id()).width(), 360.0);

    let arrow = dialog(&h).back;
    press(&mut h, arrow);
    assert_eq!((h.section(), shown(&h)), (Some(Section::Editor), LIST), "arrow in the header goes to the list");

    h.open_section(Section::History);
    assert_eq!((h.section(), shown(&h)), (Some(Section::History), DETAIL));
    back(&mut h);
    assert_eq!((h.section(), shown(&h)), (Some(Section::History), LIST), "Back from a section goes to the list");
    back(&mut h);
    assert!(h.app.popup.is_none(), "Back from the list closes settings");
}

#[test]
fn phone_settings_close_sub_closes_directly_from_detail() {
    let mut h = opened(360.0);
    h.open_section(Section::Editor);
    let close_sub = dialog(&h).close_sub;
    assert!(!hidden(&h, close_sub), "close button visible in section details");
    press(&mut h, close_sub);
    assert!(h.app.popup.is_none(), "pressing close in a section closes settings immediately");
}

#[test]
fn phone_settings_stay_inside_the_safe_area() {
    let mut h = opened(360.0);
    h.app.fit_window(&mut h.root, h.app.window, Insets::new(0.0, 24.0, 0.0, 48.0));
    h.settle();
    assert!(bounds(&h, dialog(&h).close).y0 >= 24.0, "header below the status bar");
    assert_eq!(bounds(&h, dialog(&h).list.id()).y1, f64::from(H) - 48.0, "list above the navigation bar");
    h.open_section(Section::Editor);
    assert!(bounds(&h, dialog(&h).back).y0 >= 24.0);
    assert_eq!(bounds(&h, dialog(&h).content.id()).y1, f64::from(H) - 48.0);
}

#[test]
fn wide_settings_keep_both_columns() {
    let mut h = opened(1000.0);
    assert_eq!(shown(&h), BOTH);
    let d = dialog(&h);
    let (panes, list, content) = (bounds(&h, d.panes.id()), bounds(&h, d.list.id()), bounds(&h, d.content.id()));
    assert_eq!((panes.x0, panes.y0, panes.x1, panes.y1), (50.0, 45.0, 950.0, 555.0), "card centered, as before");
    assert_eq!((list.width(), content.x0), (size::SETTINGS_NAV_WIDTH, list.x1 + size::BORDER_1));
    assert_eq!(bounds(&h, h.nav(Section::Editor)).height(), size::SIZE_32);
    h.open_section(Section::Editor);
    assert_eq!((h.section(), shown(&h)), (Some(Section::Editor), BOTH));
    back(&mut h);
    assert!(h.app.popup.is_none(), "in a wide window Back closes settings immediately");
}

#[test]
fn width_change_keeps_the_section_and_typed_text() {
    let mut h = opened(360.0);
    h.open_section(Section::Mcp);
    let port = h.binding("port field", |b| matches!(b, Binding::Number { number, .. } if number.commit));
    let before = h.config().mcp.port;
    press(&mut h, port);
    h.key(Key::Named(NamedKey::End), Modifiers::empty());
    h.type_text("1");
    let text = |h: &Harness| h.root.get_widget(port).and_then(|w| w.downcast::<TextEditor>()).unwrap().inner().text();
    let typed = format!("{before}1");
    assert_eq!(text(&h), typed);

    resize(&mut h, 1000.0);
    assert_eq!((h.section(), shown(&h)), (Some(Section::Mcp), BOTH), "two columns with the same section");
    assert_eq!((text(&h), h.root.focused_widget()), (typed.clone(), Some(port)), "typed text and focus preserved");

    resize(&mut h, 360.0);
    assert_eq!((h.section(), shown(&h)), (Some(Section::Mcp), DETAIL), "the section again, not the list");
    assert_eq!(text(&h), typed);
    assert_eq!(h.config().mcp.port, before, "port is applied only on Enter");

    h.app.rebuild(&mut h.root);
    h.settle();
    assert_eq!((h.section(), shown(&h)), (Some(Section::Mcp), DETAIL), "rebuilding the window does not return to the list");
}
