use masonry::core::WidgetId;
use masonry::kurbo::{Insets, Rect};

use super::*;
use crate::interface::{Side, WidthClass, drawer_width};
use crate::ui::keymap::Command;
use crate::ui::tokens::size;
use crate::ui::widgets::Pressed;

pub(super) fn at_width(width: f64) -> Harness {
    let mut h = Harness::new();
    resize(&mut h, width);
    h
}

pub(super) fn resize(h: &mut Harness, width: f64) {
    h.root.handle_window_event(masonry::core::WindowEvent::Resize(PhysicalSize::new(width as u32, H)));
    h.app.window_resized(&mut h.root, Size::new(width, f64::from(H)));
    h.settle();
}

pub(super) fn bounds(h: &Harness, id: WidgetId) -> Rect {
    h.root.get_widget(id).unwrap().ctx().bounding_box()
}

pub(super) fn hidden(h: &Harness, id: WidgetId) -> bool {
    h.root.get_widget(id).unwrap().ctx().is_stashed()
}

fn ids(h: &Harness) -> (WidgetId, WidgetId, WidgetId, WidgetId) {
    let chrome = h.app.chrome.as_ref().unwrap();
    (chrome.drawer, chrome.sidebar.id(), chrome.panel.id(), chrome.scrim)
}

fn press(h: &mut Harness, id: impl Fn(&crate::ui::Chrome) -> WidgetId) {
    let id = id(h.app.chrome.as_ref().unwrap());
    h.act(id, Pressed);
    h.settle();
}

#[test]
fn width_classes_follow_material_breakpoints() {
    let classes: Vec<_> = [360.0, 599.0, 600.0, 839.0, 840.0, 900.0, 1400.0].into_iter().map(WidthClass::of).collect();
    use WidthClass::{Compact, Expanded, Medium};
    assert_eq!(classes, [Compact, Compact, Medium, Medium, Expanded, Expanded, Expanded]);
}

#[test]
fn every_width_shows_its_own_layout() {
    for width in [360.0, 600.0, 900.0, 1400.0] {
        let h = at_width(width);
        let compact = width < 600.0;
        let (drawer, sidebar, panel, scrim) = ids(&h);
        assert_eq!(h.app.compact(), compact, "{width}: width class");
        assert_eq!(hidden(&h, drawer), !compact, "{width}: drawer button");
        assert_eq!(hidden(&h, sidebar), compact, "{width}: file tree");
        assert_eq!(hidden(&h, panel), compact, "{width}: right panel");
        assert!(hidden(&h, scrim), "{width}: scrim without drawer");
        let chrome = h.app.chrome.as_ref().unwrap();
        let (strip, select) = (chrome.tab_strip.id(), chrome.tab_select.id());
        assert_eq!((hidden(&h, strip), hidden(&h, select), hidden(&h, chrome.tab_list)), (compact, !compact, compact), "{width}: on a phone a select field replaces the tab strip");
        let tabs = bounds(&h, if compact { select } else { strip });
        assert!(tabs.x1 <= width && tabs.width() > 0.0, "{width}: tabs {tabs:?}");
        if !compact {
            assert_eq!(bounds(&h, sidebar).x0, size::SIDEBAR_RAIL_WIDTH, "{width}: tree right after the icon rail");
        }
    }
}

#[test]
fn drawer_slides_over_the_note_and_closes_by_tap_outside() {
    let mut h = at_width(360.0);
    let (_, sidebar, _, scrim) = ids(&h);
    press(&mut h, |c| c.drawer);
    assert_eq!(h.app.interface.screen().drawer, Some(Side::Sidebar));
    assert!(!hidden(&h, sidebar) && !hidden(&h, scrim));
    let tree = bounds(&h, sidebar);
    assert_eq!((tree.x0, tree.width()), (size::SIDEBAR_RAIL_WIDTH, drawer_width(360.0, Side::Sidebar)), "drawer at the left edge");
    assert_eq!(bounds(&h, scrim).width(), 360.0, "scrim covers the whole window");
    h.click(350.0, 300.0);
    h.settle();
    assert_eq!(h.app.interface.screen().drawer, None, "tap outside the drawer closes it");
    assert!(hidden(&h, sidebar) && hidden(&h, scrim));
    assert!(h.app.interface.sidebar_open(), "desktop setting untouched");
}

#[test]
fn opening_a_note_or_the_rail_toggle_closes_the_drawer() {
    let mut h = at_width(360.0);
    std::fs::write(h.vault.path().join("b.md"), "второй").unwrap();
    press(&mut h, |c| c.drawer);
    assert!(h.open(h.vault.path().join("b.md")));
    assert_eq!(h.app.interface.screen().drawer, None, "note opened: drawer closed");
    press(&mut h, |c| c.drawer);
    assert!(h.open(h.vault.path().join("b.md")));
    assert_eq!(h.app.interface.screen().drawer, None, "same note: drawer still closed");
    press(&mut h, |c| c.drawer);
    h.app.open_graph(&mut h.root);
    assert_eq!(h.app.interface.screen().drawer, None, "graph opened: drawer closed");
    press(&mut h, |c| c.drawer);
    press(&mut h, |c| c.toggle_sidebar);
    assert_eq!(h.app.interface.screen().drawer, None, "icon rail button closes the drawer");
    assert!(h.app.interface.sidebar_open());
}

#[test]
fn right_panel_is_a_drawer_on_a_phone() {
    let mut h = at_width(360.0);
    let (_, _, panel, _) = ids(&h);
    press(&mut h, |c| c.panel_toggle);
    assert_eq!(h.app.interface.screen().drawer, Some(Side::Panel));
    let rect = bounds(&h, panel);
    assert_eq!((rect.x0, rect.x1), (360.0 - drawer_width(360.0, Side::Panel), 360.0), "right drawer at the right edge");
    assert!(h.app.interface.right_open(), "desktop right panel setting untouched");
    press(&mut h, |c| c.scrim);
    assert!(hidden(&h, panel));
}

#[test]
fn back_closes_the_top_element_then_walks_history() {
    let mut h = at_width(360.0);
    std::fs::write(h.vault.path().join("b.md"), "второй").unwrap();
    let (a, b) = (h.vault.path().join("a.md"), h.vault.path().join("b.md"));
    assert!(h.open(a.clone()) && h.open(b));
    press(&mut h, |c| c.drawer);
    h.app.open_search(&mut h.root);
    let back = |h: &mut Harness| {
        h.app.command(&mut h.root, Command::Back).expect("Back handled");
        h.deliver();
        h.settle();
    };
    back(&mut h);
    assert!(h.app.popup.is_none(), "search closes first");
    assert_eq!(h.app.interface.screen().drawer, Some(Side::Sidebar), "drawer still open");
    back(&mut h);
    assert_eq!(h.app.interface.screen().drawer, None, "then the drawer");
    back(&mut h);
    assert_eq!(h.app.tabs.active().path.as_deref(), Some(a.as_path()), "then note history");
    h.app.open_settings(&mut h.root);
    back(&mut h);
    assert!(h.app.popup.is_none(), "settings close on Back");
}

#[test]
fn back_closes_menus_like_their_own_dismissal() {
    let mut h = at_width(360.0);
    let folder = h.vault.path().to_path_buf();
    h.app.open_file_menu(&mut h.root, vec![folder.clone()], folder, true, masonry::kurbo::Point::new(100.0, 100.0), None);
    let [_, _, sort, _] = h.app.tree.toolbar_ids().unwrap();
    for open in [None, Some(sort)] {
        if let Some(button) = open {
            h.app.open_sort_menu(&mut h.root, button);
        }
        let layer = match &h.app.popup {
            Some(Popup::FileMenu { layer, .. } | Popup::SortMenu { layer, .. }) => *layer,
            _ => panic!("menu not open"),
        };
        h.app.command(&mut h.root, Command::Back).expect("Back handled");
        assert!(h.app.popup.is_none(), "menu closed");
        assert!(h.root.get_widget(layer).is_none(), "menu layer removed");
    }
}

#[test]
fn switching_classes_keeps_desktop_preferences() {
    let mut h = Harness::new();
    let (_, sidebar, panel, _) = ids(&h);
    assert!(!hidden(&h, sidebar) && !hidden(&h, panel));
    resize(&mut h, 360.0);
    press(&mut h, |c| c.drawer);
    resize(&mut h, 1000.0);
    assert_eq!(h.app.interface.screen().drawer, None, "class change closes the drawer");
    assert!(!h.app.compact() && !hidden(&h, sidebar) && !hidden(&h, panel), "desktop panels are back");
    assert_eq!(bounds(&h, sidebar).x0, size::SIDEBAR_RAIL_WIDTH);
}

#[test]
fn safe_area_keeps_the_layout_clear_of_system_bars() {
    let mut h = at_width(360.0);
    h.app.fit_window(&mut h.root, h.app.window, Insets::new(0.0, 24.0, 0.0, 48.0));
    h.settle();
    let chrome = h.app.chrome.as_ref().unwrap();
    let (strip, pages) = (bounds(&h, chrome.tab_strip.id()), bounds(&h, chrome.pages.id()));
    assert!(strip.y0 >= 24.0, "header below the status bar: {strip:?}");
    assert_eq!(pages.y1, f64::from(H) - 48.0, "bottom above the navigation bar and keyboard");
    resize(&mut h, 900.0);
    h.app.fit_window(&mut h.root, h.app.window, Insets::new(30.0, 0.0, 0.0, 0.0));
    h.settle();
    let rail = bounds(&h, h.app.chrome.as_ref().unwrap().rail.id());
    assert_eq!(rail.x0, 30.0, "icon rail right of the camera cutout");
}
