use super::*;
use crate::app::MENU_COLOR;
use crate::ui::file_tree::FileTree;
use crate::ui::menu::MenuChoice;
use masonry::core::WidgetId;
use masonry::kurbo::{Point, Rect};

fn rect(h: &Harness, id: WidgetId) -> Rect {
    h.root.get_widget(id).unwrap().ctx().border_box()
}

fn open_menu(h: &mut Harness, folder: &std::path::Path) -> WidgetId {
    h.app.open_file_menu(&mut h.root, vec![folder.to_path_buf()], folder.to_path_buf(), true, Point::new(300.0, 200.0), None);
    let Some(Popup::FileMenu { layer, .. }) = &h.app.popup else { panic!("folder menu not open") };
    let layer = *layer;
    let _ = h.root.redraw();
    layer
}

#[test]
fn folder_menu_opens_the_palette_on_hover_and_paints_the_folder() {
    let mut h = Harness::new();
    let folder = h.vault.path().join("Проекты");
    std::fs::create_dir(&folder).unwrap();
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&folder));
    let layer = open_menu(&mut h, &folder);
    let closed = rect(&h, layer).width();
    h.act(layer, MenuChoice::Hover(MENU_COLOR));
    assert!(rect(&h, layer).width() > closed + 100.0, "palette did not open to the right of the menu");
    h.act(layer, MenuChoice::Hover(0));
    assert_eq!(rect(&h, layer).width(), closed, "palette did not close when hovering another item");
    h.act(layer, MenuChoice::Hover(MENU_COLOR));
    h.act(layer, MenuChoice::Sub(3));
    assert_eq!(h.app.tree.color(&folder), Some("green"));
    assert!(h.app.popup.is_none(), "menu remained after picking a color");
    let reopened = FileTree::new(Some(h.vault.path().to_path_buf()), h._data.path(), std::sync::Arc::new(|| {}));
    assert_eq!(reopened.color(&folder), Some("green"), "color was not saved");
    let layer = open_menu(&mut h, &folder);
    h.act(layer, MenuChoice::Hover(MENU_COLOR));
    h.act(layer, MenuChoice::Sub(0));
    assert_eq!(h.app.tree.color(&folder), None, "No color did not reset the color");
}

#[test]
fn files_have_no_color_item() {
    let mut h = Harness::new();
    let note = h.vault.path().join("a.md");
    h.app.open_file_menu(&mut h.root, vec![note.clone()], note, false, Point::new(300.0, 200.0), None);
    let Some(Popup::FileMenu { layer, .. }) = &h.app.popup else { panic!("menu not open") };
    let layer = *layer;
    let closed = rect(&h, layer).width();
    h.act(layer, MenuChoice::Hover(MENU_COLOR));
    assert_eq!(rect(&h, layer).width(), closed);
}
