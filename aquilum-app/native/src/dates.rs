use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};

use crate::i18n::{self, Language};

const RU: [&str; 12] =
    ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
const EN: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const ES: [&str; 12] =
    ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

pub fn offset_seconds() -> i64 {
    i64::from(Local::now().offset().local_minus_utc())
}

pub fn local(epoch_ms: u64) -> Option<DateTime<Local>> {
    Local.timestamp_millis_opt(epoch_ms as i64).single()
}

fn clock(language: Language, at: &impl Timelike) -> String {
    match language {
        Language::Ru | Language::Es => format!("{:02}:{:02}", at.hour(), at.minute()),
        Language::En => {
            let (pm, hour) = at.hour12();
            format!("{:02}:{:02} {}", hour, at.minute(), if pm { "PM" } else { "AM" })
        }
    }
}

fn calendar_day(language: Language, at: &impl Datelike, with_year: bool) -> String {
    let month = at.month0() as usize;
    match (language, with_year) {
        (Language::Ru, false) => format!("{} {}", at.day(), RU[month]),
        (Language::Ru, true) => format!("{} {} {} г.", at.day(), RU[month], at.year()),
        (Language::Es, false) => format!("{} de {}", at.day(), ES[month]),
        (Language::Es, true) => format!("{} de {} de {}", at.day(), ES[month], at.year()),
        (Language::En, false) => format!("{} {}", EN[month], at.day()),
        (Language::En, true) => format!("{} {}, {}", EN[month], at.day(), at.year()),
    }
}

fn day_and_time(language: Language, at: &(impl Datelike + Timelike)) -> String {
    let joint = match language {
        Language::Ru => " в ",
        // "a la una", but "a las dos" and onwards.
        Language::Es if at.hour() == 1 => " a la ",
        Language::Es => " a las ",
        Language::En => " at ",
    };
    format!("{}{joint}{}", calendar_day(language, at, false), clock(language, at))
}

pub fn time(epoch_ms: u64) -> String {
    local(epoch_ms).map(|at| clock(i18n::language(), &at)).unwrap_or_default()
}

pub fn day(epoch_ms: u64, with_year: bool) -> String {
    local(epoch_ms).map(|at| calendar_day(i18n::language(), &at, with_year)).unwrap_or_default()
}

pub fn date_time(epoch_ms: u64) -> String {
    local(epoch_ms).map(|at| day_and_time(i18n::language(), &at)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap().and_hms_opt(hour, 5, 0).unwrap()
    }

    #[test]
    fn days_and_times_in_every_language() {
        assert_eq!(day_and_time(Language::Ru, &at(14)), "17 ноября в 14:05");
        assert_eq!(day_and_time(Language::En, &at(14)), "November 17 at 02:05 PM");
        assert_eq!(day_and_time(Language::Es, &at(14)), "17 de noviembre a las 14:05");
        assert_eq!(day_and_time(Language::Es, &at(1)), "17 de noviembre a la 01:05");
        assert_eq!(calendar_day(Language::Ru, &at(0), true), "17 ноября 2026 г.");
        assert_eq!(calendar_day(Language::En, &at(0), true), "November 17, 2026");
        assert_eq!(calendar_day(Language::Es, &at(0), true), "17 de noviembre de 2026");
    }
}
