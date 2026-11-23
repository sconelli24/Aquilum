use aquilum_core::canvas::{Canvas, Node, NodeKind};
use masonry::core::WidgetId;
use masonry::kurbo::Point;

use super::*;
use crate::ui::canvas::CanvasAction;
use crate::ui::editor::{Edit, TextAction, TextEditor};

/// A board with a text card and a card showing the harness note `a.md`.
fn board(h: &Harness) -> (std::path::PathBuf, Canvas) {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("# Idea\n\nfirst", 0.0, 0.0, 260.0, 140.0));
    canvas.nodes.push(Node::file("a.md", 400.0, 0.0, 300.0, 200.0));
    let path = h.write_note("Board.canvas", &canvas.to_json());
    (path, canvas)
}

fn open_board(h: &mut Harness) -> (std::path::PathBuf, Canvas) {
    let (path, canvas) = board(h);
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
    assert!(h.open(path.clone()), "canvas did not open");
    h.settle();
    (path, canvas)
}

fn save_all(h: &mut Harness) {
    for _ in 0..200 {
        h.app.tick(&mut h.root, std::time::Instant::now() + std::time::Duration::from_secs(3600));
        if h.app.canvas_deadline().is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    crate::disk::wait();
    h.app.tick(&mut h.root, std::time::Instant::now() + std::time::Duration::from_secs(3600));
}

fn on_disk(path: &std::path::Path) -> Canvas {
    Canvas::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn center_of(h: &Harness, id: WidgetId) -> (f64, f64) {
    let rect = h.root.get_widget(id).unwrap().ctx().bounding_box();
    (rect.center().x, rect.center().y)
}

#[test]
fn a_canvas_is_listed_opened_and_its_cards_render_real_content() {
    let mut h = Harness::new();
    let (path, canvas) = open_board(&mut h);
    assert!(h.app.tree.names().iter().any(|name| name == "Board.canvas"), "canvas missing from the tree");
    assert_eq!(h.app.title(), "Board — Apus");
    assert!(h.app.canvas_shown());
    let shown = h.app.canvas_board(&h.root).unwrap();
    assert_eq!(shown.nodes.len(), 2);
    let text = h.app.canvas_editor(&canvas.nodes[0].id).expect("text card has an editor");
    assert_eq!(h.root.get_widget(text).unwrap().downcast::<TextEditor>().unwrap().inner().text(), "# Idea\n\nfirst");
    let note = h.app.canvas_note_editor(&h.vault.path().join("a.md")).expect("note card has an editor");
    assert_eq!(h.root.get_widget(note).unwrap().downcast::<TextEditor>().unwrap().inner().text(), "текст");
    let bounds = h.root.get_widget(text).unwrap().ctx().bounding_box();
    assert!(bounds.width() > 10.0 && bounds.height() > 10.0, "card content is laid out on screen: {bounds:?}");
    h.snapshot("canvas_open");
    assert_eq!(on_disk(&path), canvas, "opening must not rewrite the file");
}

#[test]
fn typing_in_cards_saves_the_canvas_and_the_note() {
    let mut h = Harness::new();
    let (path, canvas) = open_board(&mut h);
    let text = h.app.canvas_editor(&canvas.nodes[0].id).unwrap();
    h.act(text, TextAction::Changed("hola".into()));
    let note_path = h.vault.path().join("a.md");
    let note = h.app.canvas_note_editor(&note_path).unwrap();
    h.root.edit_widget(note, |mut e| TextEditor::apply_external(&mut e.downcast(), 0..0, "nuevo ", true));
    h.act(note, TextAction::Edited(vec![Edit { range: 0..0, inserted: "nuevo ".len(), head: true }]));
    save_all(&mut h);
    assert_eq!(on_disk(&path).nodes[0].text.as_deref(), Some("hola"));
    assert_eq!(std::fs::read_to_string(&note_path).unwrap(), "nuevo текст");
}

#[test]
fn double_click_creates_a_card_and_undo_removes_it() {
    let mut h = Harness::new();
    let (path, _) = open_board(&mut h);
    let view = h.app.canvas_view().unwrap();
    let area = h.root.get_widget(view).unwrap().ctx().bounding_box();
    let at = Point::new(area.x0 + 40.0, area.y1 - 40.0);
    h.press(PointerState { count: 2, ..pointer_at(at.x, at.y, 1.0) }, PointerButton::Primary, MOUSE);
    let board = h.app.canvas_board(&h.root).unwrap();
    assert_eq!(board.nodes.len(), 3, "double-click on the board adds a card");
    let new = board.nodes.last().unwrap().clone();
    assert_eq!(new.kind(), NodeKind::Text);
    assert!(h.app.canvas_editor(&new.id).is_some(), "the new card got an editor");
    save_all(&mut h);
    assert_eq!(on_disk(&path).nodes.len(), 3);
    h.act(view, CanvasAction::Undo { redo: false });
    assert_eq!(h.app.canvas_board(&h.root).unwrap().nodes.len(), 2);
    h.act(view, CanvasAction::Undo { redo: true });
    assert_eq!(h.app.canvas_board(&h.root).unwrap().nodes.len(), 3);
}

#[test]
fn dragging_a_card_moves_it_on_a_grid_and_saves() {
    let mut h = Harness::new();
    let (path, canvas) = open_board(&mut h);
    let text = h.app.canvas_editor(&canvas.nodes[0].id).unwrap();
    let from = center_of(&h, text);
    h.drag(from, (from.0 + 120.0, from.1 + 60.0));
    let moved = h.app.canvas_board(&h.root).unwrap().nodes[0].clone();
    assert!(moved.x > 0.0 && moved.y > 0.0, "card moved: {moved:?}");
    assert_eq!(moved.x % 20.0, 0.0, "moves snap to the grid");
    save_all(&mut h);
    assert_eq!(on_disk(&path).nodes[0].x, moved.x);
}

#[test]
fn deleting_a_selected_card_drops_its_edges() {
    let mut h = Harness::new();
    let (path, mut canvas) = board(&h);
    canvas.edges.push(aquilum_core::canvas::Edge::new(&canvas.nodes[0].id, None, &canvas.nodes[1].id, None));
    std::fs::write(&path, canvas.to_json()).unwrap();
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
    assert!(h.open(path.clone()));
    h.settle();
    let text = h.app.canvas_editor(&canvas.nodes[0].id).unwrap();
    let at = center_of(&h, text);
    h.click(at.0, at.1);
    h.key(Key::Named(NamedKey::Delete), Modifiers::empty());
    let board = h.app.canvas_board(&h.root).unwrap();
    assert_eq!(board.nodes.len(), 1);
    assert!(board.edges.is_empty());
    save_all(&mut h);
    assert!(on_disk(&path).edges.is_empty());
}

#[test]
fn the_context_menu_adds_a_note_card_by_name() {
    let mut h = Harness::new();
    let (path, _) = open_board(&mut h);
    let view = h.app.canvas_view().unwrap();
    h.act(view, CanvasAction::Menu { at: Point::new(500.0, 500.0), world: Point::new(0.0, 600.0), node: None, edge: None });
    let Some(Popup::CanvasMenu { layer, .. }) = &h.app.popup else { panic!("canvas menu not open") };
    let layer = *layer;
    h.act(layer, crate::ui::menu::MenuChoice::Item(1));
    let prompt = h.app.canvas_prompt().expect("asks for the note name");
    h.act(prompt, TextAction::Entered("a".into()));
    let board = h.app.canvas_board(&h.root).unwrap();
    assert_eq!(board.nodes.len(), 3);
    assert_eq!(board.nodes[2].file.as_deref(), Some("a.md"));
    assert_eq!((board.nodes[2].x, board.nodes[2].y), (0.0, 600.0));
    save_all(&mut h);
    assert_eq!(on_disk(&path).nodes.len(), 3);
}

#[test]
fn an_outside_edit_reloads_the_board() {
    let mut h = Harness::new();
    h.watch_vault();
    let (path, mut canvas) = open_board(&mut h);
    canvas.nodes.push(Node::text("from outside", 0.0, 400.0, 200.0, 100.0));
    std::fs::write(&path, canvas.to_json()).unwrap();
    assert!(h.wait_for(|h| h.app.canvas_board(&h.root).is_some_and(|b| b.nodes.len() == 3)), "outside edit not picked up");
    let id = canvas.nodes[2].id.clone();
    assert!(h.app.canvas_editor(&id).is_some(), "the new card got content");
}

#[test]
fn the_new_tab_page_creates_and_opens_a_canvas() {
    let mut h = Harness::new();
    let button = h.app.chrome.as_ref().unwrap().create_canvas;
    h.act(button, crate::ui::widgets::Pressed);
    h.settle();
    let created = std::fs::read_dir(h.vault.path()).unwrap().filter_map(Result::ok).map(|e| e.path()).find(|p| aquilum_core::search::paths::is_canvas(p)).expect("canvas file created");
    assert_eq!(Canvas::parse(&std::fs::read_to_string(&created).unwrap()).unwrap(), Canvas::default());
    assert!(h.app.canvas_shown(), "the new canvas is open");
}

#[test]
fn leaving_the_tab_saves_and_returning_restores_the_board() {
    let mut h = Harness::new();
    let (path, canvas) = open_board(&mut h);
    let text = h.app.canvas_editor(&canvas.nodes[0].id).unwrap();
    h.act(text, TextAction::Changed("kept".into()));
    assert!(h.open(h.vault.path().join("a.md")));
    assert!(!h.app.canvas_shown());
    assert_eq!(on_disk(&path).nodes[0].text.as_deref(), Some("kept"), "leaving the canvas saves it");
    assert!(h.open(path.clone()));
    h.settle();
    assert_eq!(h.app.canvas_board(&h.root).unwrap().nodes[0].text.as_deref(), Some("kept"));
}

#[test]
fn a_new_card_takes_the_keyboard_and_escape_gives_it_back() {
    let mut h = Harness::new();
    open_board(&mut h);
    let view = h.app.canvas_view().unwrap();
    let area = h.root.get_widget(view).unwrap().ctx().bounding_box();
    h.press(PointerState { count: 2, ..pointer_at(area.x0 + 40.0, area.y1 - 40.0, 1.0) }, PointerButton::Primary, MOUSE);
    let new = h.app.canvas_board(&h.root).unwrap().nodes.last().unwrap().id.clone();
    assert_eq!(h.app.canvas_editing(&h.root).as_deref(), Some(new.as_str()));
    let editor = h.app.canvas_editor(&new).unwrap();
    assert_eq!(h.root.focused_widget(), Some(editor), "typing goes to the new card");
    h.type_text("hi");
    assert_eq!(h.root.get_widget(editor).unwrap().downcast::<TextEditor>().unwrap().inner().text(), "hi");
    assert_eq!(h.app.canvas_board(&h.root).unwrap().nodes.last().unwrap().text.as_deref(), Some("hi"));
    h.escape();
    assert_eq!(h.app.canvas_editing(&h.root), None, "escape leaves the card");
    assert_eq!(h.root.focused_widget(), Some(view));
}

#[test]
fn renaming_the_open_canvas_keeps_saving_to_the_new_file() {
    let mut h = Harness::new();
    let (path, canvas) = open_board(&mut h);
    let to = h.vault.path().join("Renamed.canvas");
    h.app.rename_file(&mut h.root, path.clone(), to.clone());
    assert!(wait_until(300, 10, || to.exists()), "file renamed");
    let text = h.app.canvas_editor(&canvas.nodes[0].id).unwrap();
    h.act(text, TextAction::Changed("after".into()));
    save_all(&mut h);
    assert!(!path.exists(), "the old name is not written again");
    assert_eq!(on_disk(&to).nodes[0].text.as_deref(), Some("after"));
}

#[test]
fn the_canvas_tab_and_camera_come_back_after_a_restart() {
    let mut h = Harness::new();
    let (path, _) = open_board(&mut h);
    let view = h.app.canvas_view().unwrap();
    h.act(view, CanvasAction::Camera { x: 120.0, y: -40.0, scale: 0.75 });
    h.app.persist();
    let mut h = h.restarted(1.0);
    h.app.restore(&mut h.root);
    h.settle();
    assert!(h.app.canvas_shown(), "canvas tab restored");
    assert!(h.app.canvas_shows(&path));
    let view = h.app.canvas_view().unwrap();
    let camera = h.root.get_widget(view).unwrap().downcast::<crate::ui::canvas::CanvasView>().unwrap().inner().camera();
    assert!((camera.0 - 120.0).abs() < 1e-6 && (camera.1 + 40.0).abs() < 1e-6 && (camera.2 - 0.75).abs() < 1e-6, "{camera:?}");
}

#[test]
fn groups_edges_links_and_selection_are_drawn() {
    let mut h = Harness::new();
    let mut canvas = Canvas::default();
    let mut group = Node::group(Some("Plan"), -40.0, -60.0, 820.0, 360.0);
    group.color = Some("4".into());
    canvas.nodes.push(group);
    canvas.nodes.push(Node::text("## Goal\n\n- [ ] ship the canvas\n- **bold** and *italic*", 0.0, 0.0, 300.0, 200.0));
    canvas.nodes.push(Node::file("a.md", 440.0, 0.0, 300.0, 200.0));
    let mut link = Node::link("https://example.com/docs", 0.0, 380.0, 300.0, 100.0);
    link.color = Some("6".into());
    canvas.nodes.push(link);
    let mut edge = aquilum_core::canvas::Edge::new(&canvas.nodes[1].id, None, &canvas.nodes[2].id, None);
    edge.label = Some("explains".into());
    canvas.edges.push(edge);
    canvas.edges.push(aquilum_core::canvas::Edge::new(&canvas.nodes[1].id, Some(aquilum_core::canvas::Side::Bottom), &canvas.nodes[3].id, Some(aquilum_core::canvas::Side::Top)));
    let path = h.write_note("Plan.canvas", &canvas.to_json());
    assert!(h.open(path));
    h.settle();
    let text = h.app.canvas_editor(&canvas.nodes[1].id).unwrap();
    let at = center_of(&h, text);
    h.click(at.0, at.1);
    h.pointer_move(at.0, at.1);
    h.settle();
    h.snapshot("canvas_rich");
    let board = h.app.canvas_board(&h.root).unwrap();
    assert_eq!(board, canvas, "drawing and selecting change nothing");
}
