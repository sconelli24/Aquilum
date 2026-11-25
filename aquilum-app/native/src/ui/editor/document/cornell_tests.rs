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
