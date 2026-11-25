use std::path::{Path, PathBuf};

use aquilum_core::Core;
use aquilum_core::files::document::read_text;
use aquilum_core::files::gate;
use aquilum_core::history::Source;

use super::apply_placeholders;
use crate::frontmatter::{self, AUTHOR, BOOK_COVER, BOOK_FILE, BOOK_TYPE, CORNELL_TYPE, COVER, CREATED, PAGE_COVER, PAGES, RATING, READ_PERCENT, READER_POSITION, STATUS, TAGS, TYPE};
use crate::i18n::{self, Language, t_in};

const CREATED_PLACEHOLDER: &str = "{{date:DD-MM-YYYY}} | ({{time:HH:mm}})";

/// Template tail: a separator and a list of links with a caption in the interface language.
fn tail(language: Language, blank_lines: usize) -> String {
    format!("{}***\n\n{}\n- [[]]\n", "\n".repeat(blank_lines), t_in(language, "templateDialog.starterLinks"))
}

fn note_starter(language: Language) -> String {
    let fields = [(CREATED, CREATED_PLACEHOLDER), (TAGS, "[]"), (AUTHOR, ""), ("source", "")];
    frontmatter::template(&fields) + &tail(language, 4)
}

fn book_starter(language: Language) -> String {
    let tags = format!("[{}, ]", t_in(language, "templateDialog.starterBookTag"));
    let fields = [
        (CREATED, CREATED_PLACEHOLDER),
        (COVER, "true"),
        (TYPE, BOOK_TYPE),
        (AUTHOR, ""),
        (STATUS, "to-read"),
        (PAGES, ""),
        (TAGS, tags.as_str()),
        (RATING, "0"),
        (BOOK_COVER, ""),
        (PAGE_COVER, "pattern:dotgrid"),
        (BOOK_FILE, ""),
        (READER_POSITION, ""),
        (READ_PERCENT, ""),
    ];
    frontmatter::template(&fields) + &tail(language, 3)
}

/// Cornell notes: a cue (a question) opens each block of notes and a summary closes the page (`::question` and `::summary`).
fn cornell_starter(language: Language) -> String {
    let tags = format!("[{}, ]", t_in(language, "templateDialog.starterCornellTag"));
    let fields = [(CREATED, CREATED_PLACEHOLDER), (TYPE, CORNELL_TYPE), (TAGS, tags.as_str())];
    let cue = t_in(language, "templateDialog.starterCornellCue");
    let summary = t_in(language, "templateDialog.starterCornellSummary");
    format!("{}\n::question {cue}\n\n\n\n::summary {summary}\n", frontmatter::template(&fields))
}

pub fn template_text(path: &Path) -> String {
    apply_placeholders(&read_text(path).unwrap_or_default())
}

/// The text of a template for the daily note of `date`.
pub fn template_text_on(path: &Path, date: chrono::NaiveDate) -> String {
    super::placeholders::apply_placeholders_on(&read_text(path).unwrap_or_default(), date)
}

/// The set of starter templates the app offers: 1 added Cornell notes.
pub const STARTERS: u32 = 1;

pub fn write_starters(core: &Core, folder: &Path) -> Vec<PathBuf> {
    write_starters_since(core, folder, None)
}

/// The starters that are not in the folder yet; with `seen`, only those added to the app after that set, so a template that was deleted on
/// purpose does not come back.
pub fn write_starters_since(core: &Core, folder: &Path, seen: Option<u32>) -> Vec<PathBuf> {
    if let Err(error) = std::fs::create_dir_all(folder) {
        report!("templates folder not created: {error}");
        return Vec::new();
    }
    let mut created = Vec::new();
    let language = i18n::language();
    // Each starter with the set that brought it.
    let all = [("Note.md", note_starter(language), 0), ("Book.md", book_starter(language), 0), ("Cornell.md", cornell_starter(language), 1)];
    for (name, content, _) in all.into_iter().filter(|(_, _, since)| seen.is_none_or(|seen| *since > seen)) {
        let path = folder.join(name);
        if path.exists() {
            continue;
        }
        match gate::create(core, &path, &content, Source::Me, None) {
            Ok(_) => created.push(path),
            Err(error) => report!("{} not created: {error:?}", path.display()),
        }
    }
    created
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starters_keep_their_text_and_have_valid_frontmatter() {
        assert_eq!(note_starter(Language::Ru), "---\ncreated: {{date:DD-MM-YYYY}} | ({{time:HH:mm}})\ntags: []\nauthor: \nsource: \n---\n\n\n\n\n***\n\nСсылки:\n- [[]]\n");
        let book = book_starter(Language::Ru);
        assert_eq!(book, "---\ncreated: {{date:DD-MM-YYYY}} | ({{time:HH:mm}})\ncover: true\ntype: book\nauthor: \nstatus: to-read\npages: \ntags: [книга, ]\nrating: 0\nBook_cover: \nPage_cover: pattern:dotgrid\nbook_file: \nreader_position: \nread_percent: \n---\n\n\n\n***\n\nСсылки:\n- [[]]\n");
        assert!(frontmatter::end(&book).is_some_and(|end| book[end..].starts_with("\n\n\n\n***")));
        assert!(frontmatter::parse(&book).is_some_and(|fields| fields.is_book() && fields.has_page_cover()));
    }

    #[test]
    fn the_cornell_starter_has_a_cue_and_a_summary() {
        let text = cornell_starter(Language::Es);
        assert!(frontmatter::parse(&text).is_some_and(|fields| fields.is_cornell() && !fields.is_book()));
        assert!(text.contains("tags: [cornell, ]") && text.contains("\n::question Pregunta clave\n") && text.ends_with("\n::summary Resumen\n"));
    }

    #[test]
    fn starters_follow_the_interface_language() {
        assert!(note_starter(Language::En).ends_with("\n\n\n\n***\n\nLinks:\n- [[]]\n"));
        assert!(note_starter(Language::Es).ends_with("\n\n\n\n***\n\nEnlaces:\n- [[]]\n"));
        let book = book_starter(Language::Es);
        assert!(book.contains("\ntags: [libro, ]\n") && book.ends_with("\n\n\n***\n\nEnlaces:\n- [[]]\n"));
        assert!(frontmatter::parse(&book).is_some_and(|fields| fields.is_book() && fields.has_page_cover()));
    }

    #[test]
    fn crlf_templates_reach_the_editor_without_carriage_returns() {
        let dir = crate::test_dir::tempdir();
        let path = dir.path().join("Книга.md");
        std::fs::write(&path, "---\r\ntype: book\r\n---\r\n\r\nСсылки:\r\n- [[]]\r\n").unwrap();
        let text = template_text(&path);
        assert!(!text.contains('\r'));
        assert!(frontmatter::parse(&text).is_some_and(|fields| fields.is_book()));
    }
}
