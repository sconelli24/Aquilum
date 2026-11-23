use super::*;
use crate::app::tests::adaptive::resize;

#[test]
fn focus_mode_sits_out_a_narrow_screen_and_comes_back() {
    let mut h = Harness::new();
    h.open(h.vault.path().join("a.md"));
    h.settle();
    h.app.toggle_focus_mode(&mut h.root);
    h.settle();
    let button_hidden = |h: &Harness| h.root.get_widget(h.app.note_ids.as_ref().unwrap().focus).unwrap().ctx().is_stashed();
    assert!(h.app.interface.focus_mode() && !button_hidden(&h));

    resize(&mut h, 360.0);
    assert!(!h.app.interface.focus_mode(), "focus mode is active on a narrow screen");
    assert!(button_hidden(&h), "focus button visible on a narrow screen");
    h.app.toggle_focus_mode(&mut h.root);

    resize(&mut h, 1000.0);
    assert!(h.app.interface.focus_mode(), "focus mode did not return on a wide screen");
    assert!(!button_hidden(&h), "focus button did not return on a wide screen");
}

#[test]
#[ignore]
fn real_tree_after_focus_mode() {
    let out = std::path::PathBuf::from(std::env::var("AQ_OUT").unwrap());
    let vault = std::path::PathBuf::from(std::env::var("AQ_VAULT").unwrap());
    let mut h = Harness::with(Some(&vault), 1.0);
    for _ in 0..100 {
        h.app.on_core_events(&mut h.root);
        let _ = h.root.redraw();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let shot = |h: &mut Harness, name: &str| {
        for _ in 0..20 {
            h.app.on_core_events(&mut h.root);
            h.deliver();
        }
        let (layers, _) = h.root.redraw();
        let mut r = crate::render::Renderer::new(2);
        r.render(&layers, W as u16, H as u16, 1.0, theme::background()).unwrap();
        std::fs::write(out.join(name), r.png()).unwrap();
    };
    shot(&mut h, "tree-before.png");
    h.app.toggle_focus_mode(&mut h.root);
    shot(&mut h, "tree-focus.png");
    h.app.toggle_focus_mode(&mut h.root);
    shot(&mut h, "tree-after.png");
}

#[test]
fn tree_rows_survive_focus_mode() {
    let vault = crate::test_dir::tempdir();
    for i in 0..1000 {
        std::fs::write(vault.path().join(format!("Заметка {i:04}.md")), "текст").unwrap();
    }
    let mut h = Harness::with(Some(vault.path()), 1.0);
    h.open(vault.path().join("Заметка 0900.md"));
    h.deliver();
    let offset = |h: &Harness| scroll_offset(h, h.app.tree.scroll_id().unwrap());
    let before = (offset(&h), h.app.tree.built_rows());
    h.app.toggle_focus_mode(&mut h.root);
    h.deliver();
    h.app.toggle_focus_mode(&mut h.root);
    h.deliver();
    let after = (offset(&h), h.app.tree.built_rows());
    assert_eq!(before, after);
}

#[test]
fn focus_mode_removes_the_rail_and_puts_settings_in_the_top_left_corner() {
    let mut h = Harness::new();
    h.open(h.vault.path().join("a.md"));
    h.settle();
    let (rail, settings, strip, toggle) = {
        let chrome = h.app.chrome.as_ref().unwrap();
        (chrome.rail.id(), chrome.rail_settings, chrome.tab_strip.id(), chrome.toggle_sidebar)
    };
    let bounds = |h: &Harness, id| h.root.get_widget(id).unwrap().ctx().bounding_box();
    let (before, tabs) = (bounds(&h, toggle), bounds(&h, strip));
    assert!(bounds(&h, rail).width() > 0.0, "left icon rail not visible before focus mode");
    h.app.toggle_focus_mode(&mut h.root);
    h.settle();
    assert_eq!(bounds(&h, rail).width(), 0.0, "icon rail stayed in focus mode");
    let corner = bounds(&h, settings);
    assert_eq!((corner.x0, corner.y0, corner.width(), corner.height()), (before.x0, before.y0, before.width(), before.height()), "settings not in place of the left sidebar button");
    assert!(bounds(&h, strip).x0 >= corner.x1, "tabs overlap the settings button");
    h.click(corner.center().x, corner.center().y);
    assert!(h.section().is_some(), "corner button did not open settings");
    h.escape();
    h.app.toggle_focus_mode(&mut h.root);
    h.settle();
    assert!(bounds(&h, rail).width() > 0.0, "icon rail did not come back");
    assert!(h.root.get_widget(settings).unwrap().ctx().is_stashed(), "corner settings button remained after leaving the mode");
    assert_eq!(bounds(&h, strip).x0, tabs.x0, "tabs shifted after leaving the mode");
}

#[test]
fn note_search_button_sits_left_of_the_menu_opens_the_find_bar_and_hides_in_focus_mode() {
    let mut h = Harness::new();
    h.open(h.vault.path().join("a.md"));
    h.settle();
    let (search, more) = {
        let ids = h.app.note_ids.as_ref().unwrap();
        (ids.search, ids.more)
    };
    let bounds = |h: &Harness, id| h.root.get_widget(id).unwrap().ctx().bounding_box();
    assert!(bounds(&h, search).x1 <= bounds(&h, more).x0, "search not left of the ellipsis");
    assert!(h.app.find.is_none());
    let center = bounds(&h, search).center();
    h.click(center.x, center.y);
    assert!(h.app.find.is_some(), "button did not open in-note search");
    h.app.toggle_focus_mode(&mut h.root);
    h.settle();
    assert!(h.root.get_widget(search).unwrap().ctx().is_stashed(), "search button remained in focus mode");
    h.app.toggle_focus_mode(&mut h.root);
    h.settle();
    assert!(!h.root.get_widget(search).unwrap().ctx().is_stashed(), "search button did not return after focus mode");
}
