use chrono::Datelike;
use masonry::kurbo::Point;

use super::*;
use crate::calendar;

fn bounds(h: &Harness) -> masonry::kurbo::Rect {
    h.root.get_widget(h.app.chrome.as_ref().unwrap().calendar.id()).unwrap().ctx().bounding_box()
}

fn month_shown(h: &Harness) -> (i32, u32) {
    h.app.chrome.as_ref().unwrap().calendar.get(&h.root).unwrap().inner().month()
}

/// A point of the calendar, in window coordinates.
fn at(h: &Harness, local: impl Fn(&crate::ui::calendar::CalendarPanel) -> Point) -> (f64, f64) {
    let panel = h.app.chrome.as_ref().unwrap().calendar.get(&h.root).unwrap();
    let local = local(panel.inner());
    let origin = bounds(h).origin();
    (origin.x + local.x, origin.y + local.y)
}

fn configure(h: &mut Harness, change: impl FnOnce(&mut aquilum_core::settings::models::CalendarSettings)) {
    let mut config = h.config();
    change(&mut config.calendar);
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.sync_calendar(&mut h.root);
    h.settle();
}

#[test]
fn the_calendar_follows_its_settings() {
    let mut h = Harness::new();
    h.settle();
    let full = bounds(&h).height();
    assert!(full > 150.0, "the calendar is not shown: {full}");

    // Only the days by default; the weeks are an extra bar the user turns on.
    configure(&mut h, |c| c.progress_weeks = true);
    let with_weeks = bounds(&h).height();
    assert!(with_weeks > full, "the weeks bar did not appear: {with_weeks} vs {full}");
    configure(&mut h, |c| c.progress_weeks = false);
    assert_eq!(bounds(&h).height(), full, "the weeks bar stayed");

    configure(&mut h, |c| c.progress = false);
    let month = bounds(&h).height();
    assert!(month > 100.0 && month < full, "the progress bar stayed: {month} of {full}");

    configure(&mut h, |c| c.show = false);
    assert_eq!(bounds(&h).height(), 0.0, "the calendar stayed in the sidebar");

    configure(&mut h, |c| {
        c.show = true;
        c.progress = true;
    });
    assert_eq!(bounds(&h).height(), full, "the calendar did not come back as it was");
}

#[test]
fn clicking_a_day_creates_and_opens_its_daily_note_once() {
    let mut h = Harness::new();
    configure(&mut h, |c| c.confirm = false);
    h.settle();
    let today = calendar::today();
    let note = h.vault.path().join("Daily").join(format!("{}.md", calendar::daily_name(today)));
    assert!(!note.exists());

    let (x, y) = at(&h, |panel| panel.day_center(today).expect("today is on show"));
    let outcome = h.click(x, y);
    h.settle();
    assert!(outcome.title, "the title did not follow the new tab");
    assert!(note.is_file(), "the daily note was not created");
    assert!(h.app.tabs.active().path.as_deref().is_some_and(|p| aquilum_core::search::paths::same_path(p, &note)), "the daily note is not the open tab");
    let marked = h.app.calendar_state(today.year(), today.month()).marked;
    assert!(marked.contains(&today.day()), "the day is not marked: {marked:?}");

    // A second click opens the same note instead of making another.
    std::fs::write(&note, "# hoy").unwrap();
    let (x, y) = at(&h, |panel| panel.day_center(today).unwrap());
    h.click(x, y);
    h.settle();
    let notes: Vec<_> = std::fs::read_dir(note.parent().unwrap()).unwrap().filter_map(Result::ok).collect();
    assert_eq!(notes.len(), 1, "another note was created: {notes:?}");
    assert_eq!(std::fs::read_to_string(&note).unwrap(), "# hoy");
}

#[test]
fn daily_notes_go_to_the_configured_folder() {
    let mut h = Harness::new();
    configure(&mut h, |c| c.confirm = false);
    configure(&mut h, |c| c.folder = "Journal/2026".into());
    let today = calendar::today();
    let (x, y) = at(&h, |panel| panel.day_center(today).unwrap());
    h.click(x, y);
    h.settle();
    assert!(h.vault.path().join("Journal/2026").join(format!("{}.md", calendar::daily_name(today))).is_file());
}

#[test]
fn the_arrows_and_the_title_move_between_months() {
    let mut h = Harness::new();
    h.settle();
    let today = calendar::today();
    assert_eq!(month_shown(&h), (today.year(), today.month()));

    let (x, y) = at(&h, |panel| panel.arrow_center(false));
    h.click(x, y);
    assert_eq!(month_shown(&h), calendar::shifted(today.year(), today.month(), -1));
    let (x, y) = at(&h, |panel| panel.arrow_center(true));
    h.click(x, y);
    h.click(x, y);
    assert_eq!(month_shown(&h), calendar::shifted(today.year(), today.month(), 1));

    let (x, y) = at(&h, |panel| panel.title_center());
    h.click(x, y);
    assert_eq!(month_shown(&h), (today.year(), today.month()), "the title did not bring back the current month");
}

#[test]
fn the_days_with_notes_of_the_month_on_show_are_marked() {
    let mut h = Harness::new();
    let today = calendar::today();
    let (year, month) = calendar::shifted(today.year(), today.month(), -1);
    std::fs::create_dir_all(h.vault.path().join("Daily")).unwrap();
    h.write_note(&format!("Daily/{year}-{month:02}-15.md"), "x");
    h.settle();
    assert!(!h.app.calendar_state(today.year(), today.month()).marked.contains(&15) || today.day() == 15);
    assert!(h.app.calendar_state(year, month).marked.contains(&15));
}

#[test]
fn the_calendar_paints_in_the_sidebar() {
    let mut h = Harness::new();
    let today = calendar::today();
    std::fs::create_dir_all(h.vault.path().join("Daily")).unwrap();
    for day in [3, 9, today.day()] {
        h.write_note(&format!("Daily/{}-{:02}-{day:02}.md", today.year(), today.month()), "x");
    }
    h.app.sync_calendar(&mut h.root);
    h.settle();
    let frame = h.frame(1000, 800, 1.0);
    super::support::dump("calendar.png", &frame);
    assert!(bounds(&h).height() > 0.0);
}

#[test]
fn a_new_daily_note_starts_from_the_template_with_the_date_of_its_day() {
    let mut h = Harness::new();
    configure(&mut h, |c| c.confirm = false);
    let today = calendar::today();
    let templates = h.vault.path().join("Templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("Daily.md"), "# {{date:DD-MM-YYYY}}\n\n## Tasks\n").unwrap();
    configure(&mut h, |c| c.template = "Daily".into());

    // A day of the previous month, so `{{date}}` cannot be today's by accident.
    let (year, month) = calendar::shifted(today.year(), today.month(), -1);
    let day = chrono::NaiveDate::from_ymd_opt(year, month, 15).unwrap();
    let (x, y) = at(&h, |panel| panel.arrow_center(false));
    h.click(x, y);
    let (x, y) = at(&h, |panel| panel.day_center(day).expect("the day is on show"));
    h.click(x, y);
    h.settle();

    let note = h.vault.path().join("Daily").join(format!("{}.md", calendar::daily_name(day)));
    assert_eq!(std::fs::read_to_string(&note).unwrap(), format!("# 15-{month:02}-{year}\n\n## Tasks\n"));
}

#[test]
fn without_a_usable_template_the_daily_note_is_blank() {
    let mut h = Harness::new();
    configure(&mut h, |c| c.confirm = false);
    let today = calendar::today();
    for (name, folder_exists) in [("", false), ("Missing", true), ("../outside", true)] {
        let templates = h.vault.path().join("Templates");
        if folder_exists {
            std::fs::create_dir_all(&templates).unwrap();
        }
        std::fs::write(h.vault.path().join("outside.md"), "should not be read").unwrap();
        configure(&mut h, |c| c.template = name.into());
        let note = h.vault.path().join("Daily").join(format!("{}.md", calendar::daily_name(today)));
        let _ = std::fs::remove_file(&note);
        let (x, y) = at(&h, |panel| panel.day_center(today).unwrap());
        h.click(x, y);
        h.settle();
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "", "template {name:?} left text in the note");
    }
}

fn click_today(h: &mut Harness) {
    let today = calendar::today();
    let (x, y) = at(h, |panel| panel.day_center(today).expect("today is on show"));
    h.click(x, y);
    h.settle();
}

fn today_note(h: &Harness) -> std::path::PathBuf {
    h.vault.path().join("Daily").join(format!("{}.md", calendar::daily_name(calendar::today())))
}

#[test]
fn a_day_without_a_note_asks_before_creating_it() {
    let mut h = Harness::new();
    h.settle();
    let note = today_note(&h);

    click_today(&mut h);
    let layer = h.app.daily_dialog_layer().expect("no confirmation asked");
    assert!(!note.exists(), "the note was created before the answer");

    h.act(layer, crate::ui::dialog::DialogAnswer::Cancel);
    assert!(h.app.daily_dialog_layer().is_none(), "the dialog stayed open after cancelling");
    assert!(!note.exists(), "cancelling created the note");

    click_today(&mut h);
    let layer = h.app.daily_dialog_layer().expect("no confirmation asked the second time");
    h.escape();
    assert!(h.app.daily_dialog_layer().is_none(), "the dialog did not close on Escape");
    assert!(!note.exists(), "Escape created the note");
    let _ = layer;

    click_today(&mut h);
    let layer = h.app.daily_dialog_layer().expect("no confirmation asked the third time");
    let outcome = h.act(layer, crate::ui::dialog::DialogAnswer::Confirm);
    h.settle();
    assert!(outcome.title, "the title did not follow the new note");
    assert!(note.is_file(), "confirming did not create the note");
    assert!(h.app.daily_dialog_layer().is_none());
    assert!(h.app.tabs.active().path.as_deref().is_some_and(|p| aquilum_core::search::paths::same_path(p, &note)), "the new note is not open");
}

#[test]
fn a_day_that_has_a_note_opens_without_asking() {
    let mut h = Harness::new();
    let note = today_note(&h);
    std::fs::create_dir_all(note.parent().unwrap()).unwrap();
    std::fs::write(&note, "# hoy").unwrap();
    h.app.sync_calendar(&mut h.root);
    click_today(&mut h);
    assert!(h.app.daily_dialog_layer().is_none(), "asked about a note that exists");
    assert!(h.app.tabs.active().path.as_deref().is_some_and(|p| aquilum_core::search::paths::same_path(p, &note)));
}
