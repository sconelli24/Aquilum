//! What the sidebar calendar shows: the month grid, the daily notes and the progress of the year.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, NaiveDate};

use crate::i18n::Language;

/// Weeks drawn for a month: enough for any month starting on any weekday.
pub const WEEKS: usize = 6;

const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_ES: [&str; 12] = ["Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio", "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre"];
const MONTHS_RU: [&str; 12] = ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"];

const WEEKDAYS_EN: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
const WEEKDAYS_ES: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];
const WEEKDAYS_RU: [&str; 7] = ["П", "В", "С", "Ч", "П", "С", "В"];

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Name of a month on its own ("November"), in the interface language.
pub fn month_name(language: Language, month: u32) -> &'static str {
    let at = (month.clamp(1, 12) - 1) as usize;
    match language {
        Language::En => MONTHS_EN[at],
        Language::Es => MONTHS_ES[at],
        Language::Ru => MONTHS_RU[at],
    }
}

/// Initials of the weekdays, Monday first.
pub fn weekdays(language: Language) -> [&'static str; 7] {
    match language {
        Language::En => WEEKDAYS_EN,
        Language::Es => WEEKDAYS_ES,
        Language::Ru => WEEKDAYS_RU,
    }
}

/// The month `by` months away from `(year, month)`.
pub fn shifted(year: i32, month: u32, by: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + by;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// The days of a month in rows of seven, Monday first; `None` pads the first and last rows.
pub fn grid(year: i32, month: u32) -> [[Option<NaiveDate>; 7]; WEEKS] {
    let mut rows = [[None; 7]; WEEKS];
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else { return rows };
    let lead = first.weekday().num_days_from_monday() as usize;
    let mut day = first;
    while day.month() == month {
        let cell = lead + day.day0() as usize;
        rows[cell / 7][cell % 7] = Some(day);
        let Some(next) = day.succ_opt() else { break };
        day = next;
    }
    rows
}

/// File name of the daily note of `date`, without the folder.
pub fn daily_name(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Folder of the daily notes: the configured path inside the vault, or the vault itself.
pub fn daily_folder(vault: &Path, folder: &str) -> PathBuf {
    let folder = folder.trim().trim_matches(['/', '\\']);
    if folder.is_empty() || folder.split(['/', '\\']).any(|part| part == "..") { vault.to_path_buf() } else { vault.join(folder) }
}

pub fn daily_path(vault: &Path, folder: &str, date: NaiveDate) -> PathBuf {
    daily_folder(vault, folder).join(format!("{}.md", daily_name(date)))
}

/// The template file a setting names inside the templates folder: the name with or without
/// `.md`, possibly in a subfolder. `None` when the setting is empty, climbs out of the folder, or
/// names nothing.
pub fn template_file(templates: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim().trim_matches(['/', '\\']);
    if name.is_empty() || name.split(['/', '\\']).any(|part| part == "..") {
        return None;
    }
    let with_extension = if name.to_lowercase().ends_with(".md") { name.to_owned() } else { format!("{name}.md") };
    let path = templates.join(with_extension);
    path.is_file().then_some(path)
}

/// Days of the month that already have a daily note.
pub fn days_with_notes(folder: &Path, year: i32, month: u32) -> BTreeSet<u32> {
    let Ok(entries) = std::fs::read_dir(folder) else { return BTreeSet::new() };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let date = NaiveDate::parse_from_str(name.strip_suffix(".md")?, "%Y-%m-%d").ok()?;
            (date.year() == year && date.month() == month && entry.file_type().is_ok_and(|kind| kind.is_file())).then(|| date.day())
        })
        .collect()
}

/// How far into its year a day is, by days and by (ISO) weeks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub day: u32,
    pub days: u32,
    pub week: u32,
    pub weeks: u32,
}

impl Progress {
    pub fn of(date: NaiveDate) -> Self {
        let days = NaiveDate::from_ymd_opt(date.year(), 12, 31).map_or(365, |last| last.ordinal());
        // The week-based year can differ from the calendar year around New Year.
        let iso = date.iso_week();
        let weeks = NaiveDate::from_ymd_opt(iso.year(), 12, 28).map_or(52, |last| last.iso_week().week());
        Progress { day: date.ordinal(), days, week: iso.week(), weeks }
    }

    pub fn day_fraction(self) -> f64 {
        f64::from(self.day) / f64::from(self.days.max(1))
    }

    pub fn week_fraction(self) -> f64 {
        f64::from(self.week) / f64::from(self.weeks.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn months_step_across_years() {
        assert_eq!(shifted(2026, 11, 1), (2026, 12));
        assert_eq!(shifted(2026, 12, 1), (2027, 1));
        assert_eq!(shifted(2026, 1, -1), (2025, 12));
        assert_eq!(shifted(2026, 3, -15), (2024, 12));
    }

    #[test]
    fn the_grid_starts_on_monday_and_holds_every_day_once() {
        // November 2026 starts on a Sunday: six empty cells in front of the 1st.
        let rows = grid(2026, 11);
        assert_eq!(rows[0][..6], [None; 6]);
        assert_eq!(rows[0][6], Some(date(2026, 11, 1)));
        let days: Vec<u32> = rows.iter().flatten().flatten().map(|d| d.day()).collect();
        assert_eq!(days, (1..=30).collect::<Vec<_>>());
        // February 2027 starts on a Monday and fits four rows exactly.
        let rows = grid(2027, 2);
        assert_eq!(rows[0][0], Some(date(2027, 2, 1)));
        assert_eq!(rows[3][6], Some(date(2027, 2, 28)));
        assert!(rows[4].iter().all(Option::is_none));
    }

    #[test]
    fn a_month_that_needs_six_rows_gets_them() {
        // March 2026 starts on a Sunday and has 31 days.
        let rows = grid(2026, 3);
        assert_eq!(rows[5][0], Some(date(2026, 3, 30)));
        assert_eq!(rows[5][1], Some(date(2026, 3, 31)));
    }

    #[test]
    fn daily_notes_live_in_the_configured_folder() {
        let vault = Path::new("vault");
        assert_eq!(daily_path(vault, "Daily", date(2026, 11, 24)), vault.join("Daily").join("2026-11-24.md"));
        assert_eq!(daily_path(vault, "", date(2026, 1, 5)), vault.join("2026-01-05.md"));
        assert_eq!(daily_path(vault, " /Journal/2026/ ", date(2026, 1, 5)), vault.join("Journal/2026").join("2026-01-05.md"));
        // A folder cannot climb out of the vault.
        assert_eq!(daily_path(vault, "../outside", date(2026, 1, 5)), vault.join("2026-01-05.md"));
    }

    #[test]
    fn a_template_is_found_by_name_with_or_without_the_extension() {
        let dir = crate::test_dir::tempdir();
        std::fs::create_dir(dir.path().join("Sub")).unwrap();
        std::fs::write(dir.path().join("Daily.md"), "").unwrap();
        std::fs::write(dir.path().join("Sub/Week.md"), "").unwrap();
        let found = |name: &str| template_file(dir.path(), name);
        assert_eq!(found("Daily"), Some(dir.path().join("Daily.md")));
        assert_eq!(found(" Daily.md "), Some(dir.path().join("Daily.md")));
        assert_eq!(found("Sub/Week"), Some(dir.path().join("Sub/Week.md")));
        assert_eq!(found(""), None);
        assert_eq!(found("Missing"), None);
        assert_eq!(found("../Daily"), None, "a template cannot come from outside its folder");
        assert_eq!(found("Sub"), None, "a folder is not a template");
    }

    #[test]
    fn the_year_progress_counts_days_and_weeks() {
        let p = Progress::of(date(2026, 11, 24));
        assert_eq!((p.day, p.days), (328, 365));
        assert_eq!((p.week, p.weeks), (48, 53));
        assert_eq!(Progress::of(date(2028, 12, 31)).days, 366);
        assert!((Progress::of(date(2026, 12, 31)).day_fraction() - 1.0).abs() < 1e-9);
        // 2027-01-01 still belongs to the last ISO week of 2026 (53).
        let p = Progress::of(date(2027, 1, 1));
        assert_eq!((p.day, p.days, p.week, p.weeks), (1, 365, 53, 53));
    }

    #[test]
    fn only_daily_notes_of_the_month_are_marked() {
        let dir = crate::test_dir::tempdir();
        for name in ["2026-11-03.md", "2026-11-24.md", "2026-10-31.md", "2026-11-05.txt", "notes.md", "2026-11-07.md.bak"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        std::fs::create_dir(dir.path().join("2026-11-09.md")).unwrap();
        assert_eq!(days_with_notes(dir.path(), 2026, 11), BTreeSet::from([3, 24]));
        assert!(days_with_notes(&dir.path().join("missing"), 2026, 11).is_empty());
    }
}
