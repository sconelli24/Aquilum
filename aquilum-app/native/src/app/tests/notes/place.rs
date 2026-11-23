use super::*;

#[test]
fn note_reopens_at_saved_scroll_position() {
    use masonry::core::{PointerScrollEvent, ScrollDelta};
    let mut h = Harness::new();
    let long = h.vault.path().join("Длинная.md");
    let body: String = (0..300).map(|i| format!("Абзац {i}\n\n")).collect();
    std::fs::write(&long, &body).unwrap();
    let other = h.vault.path().join("Другая.md");
    std::fs::write(&other, "текст").unwrap();
    h.open(long.clone());
    let _ = h.root.redraw();
    let state = pointer_at(600.0, 300.0, 1.0);
    h.root.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent { pointer: MOUSE, delta: ScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -2500.0)), state }));
    h.deliver();
    let saved = note_offset(&h);
    assert!(saved > 2000.0, "scroll did not work: {saved}");
    h.open(other);
    let _ = h.root.redraw();
    h.open(long);
    let _ = h.root.redraw();
    assert!(h.app.restore_view(&mut h.root), "position not restored");
    let _ = h.root.redraw();
    let restored = note_offset(&h);
    assert!((restored - saved).abs() < 1.0, "saved {saved}, restored {restored}");
}

fn long_note(h: &Harness) -> std::path::PathBuf {
    let path = h.vault.path().join("Длинная.md");
    let body: String = (0..400)
        .map(|i| match i % 4 {
            0 => format!("Абзац {i}: {}

", "длинный текст абзаца, который переносится на несколько строк, ".repeat(6)),
            1 => format!("## Заголовок {i}

"),
            _ => format!("Абзац {i}

"),
        })
        .collect();
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn a_restarted_app_reopens_a_long_note_at_the_place_it_was_left() {
    use masonry::core::{PointerScrollEvent, ScrollDelta};
    let mut h = Harness::new();
    let long = long_note(&h);
    h.open(long);
    let _ = h.root.redraw();
    let state = pointer_at(600.0, 300.0, 1.0);
    h.root.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent { pointer: MOUSE, delta: ScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -9000.0)), state }));
    h.settle();
    h.deliver();
    h.app.capture_view(&h.root);
    let saved = h.app.view.expect("position remembered");
    let mut h = h.restarted(1.0);
    h.app.restore(&mut h.root);
    h.parked();
    for _ in 0..3 {
        h.app.restore_view(&mut h.root);
        let _ = h.root.redraw();
    }
    h.app.capture_view(&h.root);
    let reopened = h.app.view.expect("position after restart");
    assert_eq!(reopened.0, saved.0, "a different part of the note opened after restart");
    assert!((reopened.1 - saved.1).abs() < 1.0, "shift within the paragraph: was {}, now {}", saved.1, reopened.1);
}
