use super::support::{TAG, unfocused};
use crate::ui::editor::TextEditor;

const WIDTH: u32 = 600;
const HEIGHT: u32 = 400;

fn editor_guides(source: &str) -> Vec<(f64, f64, f64)> {
    let mut root = unfocused(TextEditor::new(source).markdown(), WIDTH, HEIGHT);
    let _ = root.redraw();
    root.get_widget_with_tag(TAG).expect("editor").inner().outline_guides_all()
}

fn render(source: &str, name: &str) -> Vec<u32> {
    let mut root = unfocused(TextEditor::new(source).markdown(), WIDTH, HEIGHT);
    let (layers, _) = root.redraw();
    let mut renderer = crate::render::Renderer::new(2);
    renderer.render(&layers, WIDTH as u16, HEIGHT as u16, 1.0, crate::ui::theme::background()).unwrap();
    if let Ok(out) = std::env::var("AQ_OUT") {
        std::fs::write(std::path::Path::new(&out).join(name), renderer.png()).unwrap();
    }
    renderer.frame_pixels().to_vec()
}

#[test]
fn flat_list_has_no_vertical_guides() {
    let guides = editor_guides("- один\n- два\n- три");
    assert_eq!(guides.len(), 0, "a flat list without nesting must have no guide lines");
}

#[test]
fn single_nested_sublist_has_one_guide_stopping_at_last_child() {
    let source = "- родитель\n  - потомок 1\n  - потомок 2\n- следующий";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 1, "there must be exactly 1 line from the parent to child 2");
    let (x, y0, y1) = guides[0];
    assert!(x > 10.0 && x < 25.0, "x coordinate must match level 0: {x}");
    assert!(y1 > y0, "y1 ({y1}) must be greater than y0 ({y0})");
    assert!(y0 >= 20.0, "the line must start with an indent enlarged by the token (y0 = {y0})");
}

#[test]
fn guide_ends_at_paragraph_bottom_not_bullet_middle() {
    let source = "- родитель\n  - потомок 1\n  - потомок 2";
    let mut root = unfocused(TextEditor::new(source).markdown(), WIDTH, HEIGHT);
    let _ = root.redraw();
    let editor = root.get_widget_with_tag(TAG).expect("editor").inner();
    let guides = editor.outline_guides_all();
    assert_eq!(guides.len(), 1);
    let (_, _, y1) = guides[0];
    let child_bottom = editor.paragraph_bottom(2);
    assert!((y1 - child_bottom).abs() < 1e-4, "the line must end at the bottom of the last paragraph ({child_bottom}), not at the bullet: {y1}");
}

#[test]
fn guide_metrics_use_design_tokens() {
    assert_eq!(crate::ui::tokens::size::EDITOR_LIST_GUIDE_GAP, 12.0);
    assert_eq!(crate::ui::tokens::size::EDITOR_LIST_GUIDE_WIDTH, 1.0);
    let dark = crate::ui::tokens::DARK;
    assert!(dark.editor_list_guide_color.components[3] > dark.border_subtle.components[3], "in the dark theme the guides are more visible than a subtle border");
}

#[test]
fn deeply_nested_lists_have_hierarchical_guides() {
    let source = "- A\n  - B\n    - C\n  - D";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 2, "there must be 2 guides (level 0 from A to D, level 1 from B to C)");
    let (x0, _, _) = guides[0];
    let (x1, _, _) = guides[1];
    assert!(x1 > x0, "the level 1 guide must be to the right of level 0: {x1} > {x0}");
}

#[test]
fn nested_code_block_stays_in_scope_and_does_not_conflict() {
    let source = "- родитель\n  - пункт с кодом\n    ```rust\n    let x = 1;\n    ```\n  - второй пункт";
    let guides = editor_guides(source);
    assert!(guides.len() >= 1, "guides must be present with a nested code block");
    let (x_parent, _, y1_parent) = guides[0];
    assert!(x_parent < 30.0, "the parent line runs to the left of the code: {x_parent}");
    assert!(y1_parent > 0.0);
}

#[test]
fn unindented_horizontal_rule_breaks_list_and_is_not_crossed() {
    let source = "- пункт 1\n  - подпункт 1\n---\n- пункт 2";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 1, "the line must stop at subitem 1 before the separator");
}

#[test]
fn unindented_code_block_breaks_list() {
    let source = "- пункт 1\n```rust\nlet x = 1;\n```\n- пункт 2";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 0, "an unindented code block interrupts the list, no dangling lines");
}

#[test]
fn loose_list_with_blank_lines_continues_guides() {
    let source = "- родитель\n\n  - потомок 1\n\n  - потомок 2";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 1, "blank lines between items do not break the guide");
}

#[test]
fn trailing_blank_line_stops_guide_at_last_item() {
    let source = "- родитель\n  - потомок\n\nобычный текст";
    let guides = editor_guides(source);
    assert_eq!(guides.len(), 1);
}

#[test]
fn rendered_pixels_show_guide_lines() {
    let flat_pixels = render("- один\n- два\n- три", "flat-list.png");
    let nested_pixels = render("- родитель\n  - потомок 1\n  - потомок 2", "nested-list.png");
    let diff = flat_pixels.iter().zip(&nested_pixels).filter(|(a, b)| a != b).count();
    assert!(diff > 0, "guide lines must be drawn when there are nested items");
}
