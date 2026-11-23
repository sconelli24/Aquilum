use super::*;

use std::time::{Duration, Instant};

use crate::stats::mb;

fn frames(b: &mut Bench, at: (f64, f64)) -> (f64, f64, f64, f64) {
    let mut times = Vec::new();
    for speed in [60.0, -45.0, 60.0] {
        let mut v: f64 = speed;
        while v.abs() > 1.0 {
            let t = Instant::now();
            b.h.scroll(at, b.scale, (0.0, -v));
            b.h.deliver();
            b.timed(false);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            v *= 0.955;
        }
    }
    let [p50, p95, max] = percentiles(&times, [0.5, 0.95, 1.0]);
    (p50, p95, max, times.len() as f64)
}

fn speeds(b: &mut Bench, at: (f64, f64)) {
    for px in [100.0, 300.0, 1000.0, 3000.0] {
        let mut times = Vec::new();
        for _ in 0..120 {
            let t = Instant::now();
            b.h.scroll(at, b.scale, (0.0, -px));
            b.h.deliver();
            b.timed(false);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let [p50, p95, max] = percentiles(&times, [0.5, 0.95, 1.0]);
        eprintln!("fast scroll {px} px per frame: p50 {p50:.1} ms, p95 {p95:.1}, max {max:.1}");
    }
}

fn typing(b: &mut Bench, at: (f64, f64)) {
    let scroll = b.h.app.note_ids.as_ref().unwrap().scroll.clone();
    scroll.edit(&mut b.h.root, |mut area| crate::ui::scroll::ScrollArea::set_fraction(&mut area, 0.5));
    b.h.deliver();
    b.h.focus_editor();
    b.h.click(at.0, at.1);
    b.timed(false);
    let keys = std::iter::repeat_n(Key::Character("ы".into()), 5).chain(std::iter::repeat_n(Key::Named(NamedKey::Enter), 3)).chain(std::iter::repeat_n(Key::Named(NamedKey::Backspace), 3));
    let mut times = Vec::new();
    for key in keys {
        let t = Instant::now();
        b.h.key(key, Modifiers::empty());
        b.timed(false);
        times.push(format!("{:.1}", t.elapsed().as_secs_f64() * 1000.0));
    }
    eprintln!("typing in the middle (5 letters, 3 Enter, 3 Backspace), ms: {}", times.join(" "));
    let t = Instant::now();
    b.h.app.tick(&mut b.h.root, Instant::now() + Duration::from_secs(3600));
    let ui = t.elapsed();
    crate::disk::wait();
    eprintln!("save: UI thread {ui:?}, background write {:?}", t.elapsed());
}

#[test]
#[ignore]
fn big_note_audit() {
    aq_trace::start();
    let mut b = Bench::start(None);
    let note = b.h.copy_note();
    let heap = crate::memory_probe::heap();
    let t = Instant::now();
    b.h.open(note);
    let opened = t.elapsed();
    let (layers, _) = b.h.root.redraw();
    let scene = t.elapsed();
    b.renderer.draw_frame(&layers, b.w as u16, b.hh as u16, b.scale, theme::background(), true).unwrap();
    let shown = t.elapsed();
    let busy = |b: &Bench| b.h.app.embeds.as_ref().is_some_and(crate::app::embeds::Embeds::busy);
    while busy(&b) && t.elapsed() < Duration::from_secs(60) {
        b.h.app.on_core_events(&mut b.h.root);
        b.timed(false);
    }
    let settled = t.elapsed();
    let doc = b.h.app.text.byte_len();
    eprintln!("note {:.0} MB: open {opened:?}, first scene {scene:?}, first frame {shown:?}, embeds ready {settled:?}", mb(doc));
    eprintln!("heap after open +{:.0} MB", mb(crate::memory_probe::heap().saturating_sub(heap)));
    let before = Threads::now();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get) as f64;
    let mut windows = Vec::new();
    for _ in 0..12 {
        let (cpu, idle) = (crate::memory_probe::cpu_ms(), Instant::now());
        while idle.elapsed() < Duration::from_secs(5) {
            b.h.app.on_core_events(&mut b.h.root);
            std::thread::sleep(Duration::from_millis(16));
        }
        windows.push(format!("{:.1}", (crate::memory_probe::cpu_ms() - cpu) / idle.elapsed().as_secs_f64() / 10.0 / threads));
    }
    eprintln!("idle for 5 s, % of all {threads} threads: {}", windows.join(" "));
    eprintln!("heap after idle +{:.0} MB", mb(crate::memory_probe::heap().saturating_sub(heap)));
    eprintln!("idle threads: {}", before.busy(50.0));
    let at = (f64::from(b.w) / 2.0, f64::from(b.hh) / 2.0);
    let scroll = b.h.app.note_ids.as_ref().unwrap().scroll.clone();
    for fraction in [0.0, 0.5, 0.95] {
        let t = Instant::now();
        scroll.edit(&mut b.h.root, |mut area| crate::ui::scroll::ScrollArea::set_fraction(&mut area, fraction));
        b.h.deliver();
        b.timed(false);
        let jump = t.elapsed();
        let (p50, p95, max, n) = frames(&mut b, at);
        let t = Instant::now();
        b.h.app.flush_view();
        let flush = t.elapsed();
        eprintln!("position {:>3.0}%: jump {jump:?}; frames {n}, p50 {p50:.2} ms, p95 {p95:.2}, max {max:.2}; position write {flush:?}", fraction * 100.0);
    }
    let mut drag = Vec::new();
    for step in 0..10 {
        let _zone = aq_trace::zone("bench: slider step");
        let t = Instant::now();
        scroll.edit(&mut b.h.root, |mut area| crate::ui::scroll::ScrollArea::set_fraction(&mut area, 0.10 + f64::from(step) * 0.01));
        b.h.deliver();
        b.timed(false);
        drag.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    eprintln!("slider, steps of 1 %: {}", drag.iter().map(|t| format!("{t:.0}")).collect::<Vec<_>>().join(" "));
    let (mut stuck, mut worst) = (0, 0.0f64);
    for _ in 0..200 {
        let before = super::super::support::note_offset(&b.h);
        let t = Instant::now();
        b.h.scroll(at, b.scale, (0.0, -3.0));
        b.h.deliver();
        b.timed(false);
        worst = worst.max(t.elapsed().as_secs_f64() * 1000.0);
        stuck += usize::from(super::super::support::note_offset(&b.h) == before);
    }
    eprintln!("touchpad 3 px: 200 events, without shift {stuck}, worst frame {worst:.1} ms");
    speeds(&mut b, at);
    typing(&mut b, at);
    let toggle = b.h.app.chrome.as_ref().unwrap().toggle_sidebar;
    b.h.act(toggle, crate::ui::widgets::Pressed);
    let mut panel = Vec::new();
    for _ in 0..30 {
        let t = Instant::now();
        b.h.anim(Duration::from_micros(8333));
        b.timed(false);
        panel.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let [p50, max] = percentiles(&panel, [0.5, 1.0]);
    eprintln!("panel: 30 frames, p50 {p50:.1} ms, max {max:.1} ms");
    if let Ok(out) = std::env::var("AQ_TRACE") {
        aq_trace::write(std::path::Path::new(&out)).unwrap();
    }
}
