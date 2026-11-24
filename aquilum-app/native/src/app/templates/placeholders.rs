use chrono::{Datelike, Timelike};

use crate::i18n::{self, Language};

const DEFAULT_DATE: &str = "YYYY-MM-DD";
const DEFAULT_TIME: &str = "HH:mm";
const TOKENS: [&str; 16] = ["YYYY", "MMMM", "DDDD", "MMM", "DDD", "YY", "MM", "DD", "HH", "mm", "ss", "M", "D", "H", "m", "s"];

/// Month and weekday names (starting Monday) for `MMMM`, `MMM`, `DDDD`, `DDD`, as in the moment.js locales.
struct Names {
    months: [&'static str; 12],
    months_short: [&'static str; 12],
    days: [&'static str; 7],
    days_short: [&'static str; 7],
}

const RU: Names = Names {
    months: ["январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь"],
    months_short: ["янв.", "февр.", "март", "апр.", "май", "июнь", "июль", "авг.", "сент.", "окт.", "нояб.", "дек."],
    days: ["понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье"],
    days_short: ["пн", "вт", "ср", "чт", "пт", "сб", "вс"],
};

const EN: Names = Names {
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    days_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
};

const ES: Names = Names {
    months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
    months_short: ["ene.", "feb.", "mar.", "abr.", "may.", "jun.", "jul.", "ago.", "sept.", "oct.", "nov.", "dic."],
    days: ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"],
    days_short: ["lun.", "mar.", "mié.", "jue.", "vie.", "sáb.", "dom."],
};

fn names(language: Language) -> &'static Names {
    match language {
        Language::Ru => &RU,
        Language::En => &EN,
        Language::Es => &ES,
    }
}

/// Substitutes `{{date}}` and `{{time}}`; month and weekday names use the interface language.
pub fn apply_placeholders(text: &str) -> String {
    apply_at(text, &chrono::Local::now(), i18n::language())
}

/// Like [`apply_placeholders`], but `{{date}}` is the given day (at the current time of day), as
/// in the daily note of a day other than today.
pub fn apply_placeholders_on(text: &str, date: chrono::NaiveDate) -> String {
    let now = chrono::Local::now();
    apply_at(text, &date.and_time(now.time()), i18n::language())
}

fn apply_at<T: Datelike + Timelike>(text: &str, now: &T, language: Language) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str("{{");
            rest = after;
            continue;
        };
        let inner = &after[..end];
        let (kind, pattern) = inner.split_once(':').map_or((inner, None), |(k, p)| (k, Some(p)));
        let default = match kind.trim().to_lowercase().as_str() {
            "date" => Some(DEFAULT_DATE),
            "time" => Some(DEFAULT_TIME),
            _ => None,
        };
        match default {
            Some(default) if !pattern.is_some_and(|p| p.contains('}')) => {
                let pattern = pattern.map(str::trim).filter(|p| !p.is_empty()).unwrap_or(default);
                out.push_str(&format_moment(pattern, now, names(language)));
            }
            _ => out.push_str(&rest[start..start + 2 + end + 2]),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn part<T: Datelike + Timelike>(token: &str, now: &T, names: &Names) -> String {
    match token {
        "YYYY" => format!("{:04}", now.year()),
        "YY" => format!("{:02}", now.year().rem_euclid(100)),
        "MMMM" => names.months[now.month0() as usize].to_owned(),
        "MMM" => names.months_short[now.month0() as usize].to_owned(),
        "MM" => format!("{:02}", now.month()),
        "M" => now.month().to_string(),
        "DDDD" => names.days[now.weekday().num_days_from_monday() as usize].to_owned(),
        "DDD" => names.days_short[now.weekday().num_days_from_monday() as usize].to_owned(),
        "DD" => format!("{:02}", now.day()),
        "D" => now.day().to_string(),
        "HH" => format!("{:02}", now.hour()),
        "H" => now.hour().to_string(),
        "mm" => format!("{:02}", now.minute()),
        "m" => now.minute().to_string(),
        "ss" => format!("{:02}", now.second()),
        _ => now.second().to_string(),
    }
}

fn format_moment<T: Datelike + Timelike>(pattern: &str, now: &T, names: &Names) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(c) = rest.chars().next() {
        if c == '['
            && let Some(close) = rest.find(']')
        {
            out.push_str(&rest[1..close]);
            rest = &rest[close + 1..];
        } else if let Some(token) = TOKENS.iter().find(|t| rest.starts_with(**t)) {
            out.push_str(&part(token, now, names));
            rest = &rest[token.len()..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moment() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap().and_hms_opt(9, 5, 7).unwrap()
    }

    #[test]
    fn formats_like_the_tauri_version() {
        let now = moment();
        let ru = Language::Ru;
        assert_eq!(apply_at("Сегодня {{date}}, время {{time}}", &now, ru), "Сегодня 2026-10-11, время 09:05");
        assert_eq!(apply_at("{{ Date : DDDD, D MMMM YYYY [в] H:mm:ss }}", &now, ru), "воскресенье, 11 октябрь 2026 в 9:05:07");
        assert_eq!(apply_at("{{date:100%}} {{title}} {{date:}}", &now, ru), "100% {{title}} 2026-10-11");
        assert_eq!(apply_at("{{date:YY.M.D}} {{time:HH-mm}}", &now, ru), "26.10.11 09-05");
    }

    #[test]
    fn month_and_day_names_follow_the_interface_language() {
        let now = moment();
        let pattern = "{{date:DDDD, D MMMM YYYY (DDD, MMM)}}";
        assert_eq!(apply_at(pattern, &now, Language::Ru), "воскресенье, 11 октябрь 2026 (вс, окт.)");
        assert_eq!(apply_at(pattern, &now, Language::En), "Sunday, 11 October 2026 (Sun, Oct)");
        assert_eq!(apply_at(pattern, &now, Language::Es), "domingo, 11 octubre 2026 (dom., oct.)");
    }
}

#[cfg(test)]
mod on_a_day {
    use super::*;

    #[test]
    fn the_date_is_the_given_day_not_today() {
        let text = apply_placeholders_on("{{date:YYYY-MM-DD}} {{date:DDDD}}", chrono::NaiveDate::from_ymd_opt(2020, 2, 29).unwrap());
        assert!(text.starts_with("2020-02-29 "), "{text}");
    }
}
