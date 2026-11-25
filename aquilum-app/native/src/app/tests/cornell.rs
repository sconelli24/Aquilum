use super::*;
use crate::app::palette::{Item, entries};

const HEAD: &str = "---\ntype: cornell\n---\n\n";

/// A Cornell note open with its properties shown, the caret at the start of line `line`.
fn open(h: &mut Harness, text: &str, down: usize) {
    let note = h.write_note("Clase.md", text);
    h.app.interface.set_metadata_expanded(&note, true);
    h.open(note);
    let _ = h.root.redraw();
    h.focus_editor();
    for _ in 0..down {
        h.key(Key::Named(NamedKey::ArrowDown), Modifiers::empty());
    }
    h.key(Key::Named(NamedKey::End), Modifiers::empty());
}

#[test]
fn typing_two_colons_lists_the_cue_and_the_summary_and_enter_writes_the_marker() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}Antes\n"), 4);
    h.type_text("\n\n::");
    h.app.on_core_events(&mut h.root);
    let _ = h.root.redraw();
    let shown = h.root.get_widget(h.app.editor_id().unwrap()).unwrap().downcast::<crate::ui::editor::TextEditor>().unwrap().inner().suggestions();
    assert_eq!(shown.map(|(items, _)| items), Some(vec!["::question".to_owned(), "::summary".to_owned()]));
    h.type_text("su");
    h.app.on_core_events(&mut h.root);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert!(h.body_text().contains("Antes\n\n::summary "), "{:?}", h.body_text());
}

#[test]
fn other_notes_do_not_suggest_cues() {
    let mut h = Harness::new();
    open(&mut h, "---\ntype: note\n---\n\nAntes\n", 4);
    h.type_text("\n\n::");
    h.app.on_core_events(&mut h.root);
    assert!(h.app.suggest_layer.is_none(), "a plain note has no cues");
}

#[test]
fn enter_at_the_end_of_a_cue_goes_on_to_its_notes() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}::question Hola\n\n\nNota\n"), 4);
    let text = h.body_text();
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(h.body_text(), text, "going on to the notes does not change the text");
    assert_eq!(super::support::caret(&h), HEAD.len() + "::question Hola\n\n".len(), "the caret is in the line of the notes");
}

#[test]
fn enter_at_the_end_of_a_cue_with_no_notes_makes_room_for_them() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}::question Hola"), 4);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(h.body_text(), format!("{HEAD}::question Hola\n\n"));
    assert_eq!(super::support::caret(&h), h.body_text().len());
}

#[test]
fn shift_enter_and_enter_in_the_middle_keep_a_cue_going() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}::question Hola\n\nNota\n"), 4);
    h.key(Key::Named(NamedKey::Enter), Modifiers::SHIFT);
    assert_eq!(h.body_text(), format!("{HEAD}::question Hola\n\n\nNota\n"), "Shift+Enter is an ordinary new line");
}

#[test]
fn a_new_row_goes_before_the_summary_and_leaves_the_caret_in_its_cue() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}::question A\n\nnota\n\n::summary fin\n"), 0);
    h.app.insert_cornell_row(&mut h.root);
    h.settle();
    let at = format!("{HEAD}::question A\n\nnota\n\n").len();
    assert_eq!(h.body_text(), format!("{HEAD}::question A\n\nnota\n\n::question \n\n\n\n::summary fin\n"));
    assert_eq!(super::support::caret(&h), at + "::question ".len());
}

#[test]
fn a_new_row_without_a_summary_goes_at_the_end() {
    let mut h = Harness::new();
    open(&mut h, &format!("{HEAD}::question A\n\nnota"), 0);
    h.app.insert_cornell_row(&mut h.root);
    h.settle();
    assert_eq!(h.body_text(), format!("{HEAD}::question A\n\nnota\n\n::question \n\n\n\n"));
}

#[test]
fn the_palette_offers_a_new_row_in_cornell_notes_only() {
    assert!(entries(true).iter().any(|entry| entry.item == Item::CornellRow));
    assert!(!entries(false).iter().any(|entry| entry.item == Item::CornellRow));
}
