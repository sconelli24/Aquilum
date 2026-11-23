use masonry::core::ScrollDelta;

use super::*;
use crate::app::tests::dataview_scroll::{bar, open_wide_table, wheel};

fn offset(h: &Harness) -> Option<f64> {
    bar(h).map(|span| span.offset)
}

#[test]
fn scrolling_a_wide_dataview_table_repaints_what_moved() {
    let spec = FIXED;
    let mut h = open_wide_table(sized(spec, None));
    let mut frames = Frames::new(&mut h, spec, true);
    let _ = frames.draw(&mut h);
    for step in 0..6 {
        wheel(&mut h, ScrollDelta::LineDelta(0.0, if step < 4 { -1.0 } else { 1.0 }), Modifiers::SHIFT, spec.scale);
        let before = offset(&h);
        let report = frames.draw(&mut h);
        assert_eq!(report.differ, 0, "step {step}: partial frame differs from the full one in {} pixels, offset {before:?}", report.differ);
        assert!(report.raster > 0, "step {step}: table scroll repainted nothing");
    }
}

#[test]
fn narrowing_the_column_keeps_a_scrolled_dataview_table_steady() {
    let spec = FIXED;
    let mut h = open_wide_table(sized(spec, None));
    wheel(&mut h, ScrollDelta::LineDelta(0.0, -2.0), Modifiers::SHIFT, spec.scale);
    let sash = h.app.chrome.as_ref().unwrap().sidebar_sash;
    let at = rect_of(&h, sash, spec.scale).center();
    let mut frames = Frames::new(&mut h, spec, true);
    pointer(&mut h, at.x, at.y, spec.scale, false);
    press(&mut h, at.x, at.y, spec.scale);
    h.deliver();
    let _ = frames.draw(&mut h);
    for i in 1..=12 {
        pointer(&mut h, at.x + f64::from(i) * 7.0 * spec.scale, at.y, spec.scale, true);
        h.deliver();
        let report = frames.draw(&mut h);
        assert_eq!(report.differ, 0, "step {i}: partial frame differs from the full one in {} pixels", report.differ);
    }
    release(&mut h, at.x, at.y, spec.scale);
}
