use masonry::core::StyleSet;
use masonry::parley::{FontContext, LayoutContext};

use super::cornell::Lane;
use super::tests::markdown;
use super::Document;

const NOTE: &str = "---\ntype: cornell\n---\n\n# Tema\n\n> [!question] ¿Qué es?\n> sigue\n\nUna nota\n\n> cita común\n\n> [!summary] Resumen\n> final\n";

fn laid(text: &str, width: f32) -> Document {
    let mut doc = markdown(text);
    doc.keep = (0.0, 10_000.0);
    let (mut fcx, mut lcx, styles) = (FontContext::default(), LayoutContext::new(), StyleSet::new(16.0));
    doc.layout(&mut fcx, &mut lcx, &styles, Some(width), None, None);
    doc
}

fn lanes(doc: &Document) -> Vec<Lane> {
    (0..doc.line_count()).map(|i| doc.lane(i)).collect()
}

#[test]
fn cornell_notes_split_cues_notes_and_the_summary() {
    use Lane::{Cue, Full, Notes};
    let doc = laid(NOTE, 900.0);
    // properties (3 lines), blank, heading, blank, question x2, blank, note, blank, quote, blank, summary x2, blank
    assert_eq!(lanes(&doc), [Full, Full, Full, Notes, Full, Notes, Cue, Cue, Notes, Notes, Notes, Notes, Notes, Full, Full, Notes]);
}

#[test]
fn notes_start_right_of_the_cue_column_and_everything_else_does_not() {
    let doc = laid(NOTE, 900.0);
    let room = f64::from(doc.cue_width() + 20.0);
    let x = |i: usize| doc.paragraph(i).decor().x;
    assert!(x(9) >= room, "a note starts at {} and the column at {room}", x(9));
    assert!(x(4) < room, "the heading keeps the left edge");
    let last = 14;
    assert!(x(last) < room, "the summary takes the whole width");
}

#[test]
fn a_narrow_window_and_other_notes_stay_in_one_column() {
    let narrow = laid(NOTE, 500.0);
    assert!(lanes(&narrow).iter().all(|lane| *lane == Lane::Full));
    let plain = laid(&NOTE.replace("cornell", "book"), 900.0);
    assert!(lanes(&plain).iter().all(|lane| *lane == Lane::Full));
}

fn long_cue() -> String {
    format!("---\ntype: cornell\n---\n\n> [!question] {}\n\nNota corta\n\n> [!question] Segunda\n\nOtra\n", "palabra ".repeat(40))
}

#[test]
fn a_cue_takes_no_height_but_its_row_is_as_tall_as_the_cue() {
    let doc = laid(&long_cue(), 900.0);
    // 0-2 properties, 3 blank, 4 cue, 5 blank, 6 note, 7 blank, 8 cue, 9 blank, 10 note
    let cue = doc.heights.text(4);
    assert_eq!(doc.heights.get(4), 0.0, "a cue takes no room of its own");
    assert!(cue > doc.heights.text(6) * 3.0, "the cue wraps in its narrow column: {cue}");
    assert!(doc.top(8) - doc.top(4) >= cue + doc.cue_offset(4) - 0.5, "the next row starts {} below the first, the cue is {cue} tall", doc.top(8) - doc.top(4));
    assert!(doc.top(8) - doc.top(4) <= cue + doc.cue_offset(4) + 1.0, "the row is no taller than it needs");
    assert!(doc.top(4) == doc.top(5) && (doc.cue_top(4)..doc.cue_top(4) + 40.0).contains(&doc.origin(4).y), "a cue hangs from the top of its row");
}

#[test]
fn a_tall_row_of_notes_is_not_stretched_by_a_short_cue() {
    let notes = "linea\n\n".repeat(12);
    let doc = laid(&format!("---\ntype: cornell\n---\n\n> [!question] Corta\n\n{notes}> [!question] Otra\n"), 900.0);
    let (first, second) = (4, doc.line_count() - 2);
    let row: f64 = (first + 1..second).map(|k| doc.heights.text(k)).sum();
    assert!((doc.top(second) - doc.top(first) - row).abs() < 1.0, "a row of notes taller than its cue keeps its own height");
}

#[test]
fn the_left_column_answers_to_clicks_by_cue_line() {
    let doc = laid(&long_cue(), 900.0);
    let y = doc.cue_top(4) + 5.0;
    assert_eq!(doc.cue_at(5.0, y), Some(4));
    let second_line = doc.cue_top(4) + doc.heights.text(4) - 5.0;
    assert_eq!(doc.cue_at(5.0, second_line), Some(4), "the lines of a wrapped cue are one paragraph");
    assert_eq!(doc.cue_at(f64::from(doc.cue_width()) + 30.0, y), None, "the notes column is not the cues");
    assert_eq!(doc.cue_at(5.0, doc.top(8) + doc.heights.text(8) + 40.0), None, "below the cue there is nothing to hit");
}
