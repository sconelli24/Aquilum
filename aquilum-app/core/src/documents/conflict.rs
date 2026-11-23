use crate::search::note_date::to_civil_at;
use crate::text::content::{find_byte, read_bytes, Content};
use crate::text::pieces::Pieces;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictCause {
    Diverged,
    Displaced,
    Reverted,
}

struct Explanation {
    title: &'static str,
    why: &'static str,
}

fn explanation(cause: ConflictCause) -> Explanation {
    match cause {
        ConflictCause::Diverged => Explanation {
            title: "file and document changed at the same time",
            why: "Both the file on disk and the document open in the editor changed after the last sync check. \
                  They could not be merged without loss, so the note kept the version from disk \
                  and the editor version is saved here.",
        },
        ConflictCause::Displaced => Explanation {
            title: "lines did not fit during the merge",
            why: "An external edit arrived in the note, and merging it with what was being typed in the editor \
                  displaced the lines below. The rest of the note text is intact.",
        },
        ConflictCause::Reverted => Explanation {
            title: "version changes reverted over later edits",
            why: "Reverting the changes of one version from the history touched lines that were edited later. \
                  In the note those places went back to how they were before that version, and the later text \
                  of those lines is saved here.",
        },
    }
}

pub struct ConflictReport {
    pub cause: ConflictCause,
    pub synced_hash: Option<String>,
}

pub struct LocalMinute {
    pub date: String,
    pub time: String,
}

impl LocalMinute {
    pub fn at(unix_nanos: i64, offset_seconds: i64) -> Self {
        let (year, month, day, inside) = to_civil_at(unix_nanos, offset_seconds);
        Self {
            date: format!("{year:04}-{month:02}-{day:02}"),
            time: format!("{:02}:{:02}", inside / 3600, (inside % 3600) / 60),
        }
    }

    pub fn now(offset_seconds: i64) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos() as i64);
        Self::at(nanos, offset_seconds)
    }
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn copy_stem(stem: &str, moment: &LocalMinute, device: &str) -> String {
    format!("{stem} (conflict {} {} {device})", moment.date, moment.time.replace(':', "-"))
}

pub fn copy_content<'a>(
    note_path: &str,
    stem: &str,
    report: &ConflictReport,
    moment: &LocalMinute,
    body: &'a dyn Content,
) -> Option<Pieces<Box<dyn Content + 'a>>> {
    let len = body.byte_len();
    find_byte(body, 0..len, false, |byte| !byte.is_ascii_whitespace()).ok()??;
    let explanation = explanation(report.cause);
    let stamp = format!("{} {}", moment.date, moment.time);
    let mut lines = vec![
        "---".to_owned(),
        format!("conflict: {stamp}"),
        format!("note: {}", quoted(stem)),
        format!("reason: {}", quoted(explanation.title)),
        "---".to_owned(),
        String::new(),
        format!("> This is a conflict copy. The text below did not make it into the note \"{stem}\"; it is saved here so it is not lost."),
        ">".to_owned(),
        format!("> **Why.** {}", explanation.why),
        ">".to_owned(),
        format!("> **When.** {stamp}"),
        format!("> **Note.** `{note_path}`"),
    ];
    if let Some(hash) = &report.synced_hash {
        lines.push(format!("> **Last sync fingerprint.** `{}`", one_line(hash)));
    }
    lines.extend([String::new(), "## Text that did not make it into the note".to_owned(), String::new()]);
    let head = lines.join("\n");
    let ends = read_bytes(body, len - 1..len).ok()? == b"\n";
    let (head_len, tail) = (head.len(), if ends { "" } else { "\n" });
    let mut copy: Pieces<Box<dyn Content + 'a>> = Pieces::new(vec![Box::new(head), Box::new(body), Box::new(tail)]);
    copy.push(0, 0..head_len);
    copy.push(1, 0..len);
    copy.push(2, 0..tail.len());
    Some(copy)
}

#[cfg(test)]
#[path = "conflict_tests.rs"]
mod tests;
