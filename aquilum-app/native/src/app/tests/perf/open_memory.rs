use super::*;

use std::time::{Duration, Instant};


use crate::memory_probe::{heap, peak, report, reset_peak, trace};
use crate::stats::mb;

#[test]
#[ignore]
fn big_note_open_memory() {
    let mut b = Bench::start(None);
    let note = b.h.copy_note();
    let (core, workspace) = (std::sync::Arc::clone(&b.h.app.workspace.core), b.h.vault.path().to_string_lossy().into_owned());
    core.search.prepare(&workspace).unwrap();
    assert!(wait_index(&core, &workspace), "index not ready");
    std::thread::sleep(Duration::from_secs(2));
    trace(true);
    let base = heap();
    reset_peak();
    b.h.open(note);
    b.timed(true);
    eprintln!("open and first frame: peak +{:.0} MB, after +{:.0} MB", mb(peak() - base), mb(heap() - base));
    let (mut top, mut snapshot) = (heap(), report());
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        b.h.app.on_core_events(&mut b.h.root);
        b.timed(false);
        if heap() > top {
            (top, snapshot) = (heap(), report());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("background 10 s: peak +{:.0} MB (largest snapshot +{:.0}), at end +{:.0} MB", mb(peak() - base), mb(top - base), mb(heap() - base));
    eprintln!("{snapshot}");
}
