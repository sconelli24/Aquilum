//! Which notes are books (`type: book` in the properties), so the tree can give them their own icon.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

/// Properties live at the very top of a note: this much of the file is enough to find them.
const HEAD: u64 = 4096;

/// Per note: modification time and whether it was a book then. A note is only read again after it changes.
pub type Books = RefCell<HashMap<String, (SystemTime, bool)>>;

fn read_is_book(path: &Path) -> bool {
    let mut head = Vec::new();
    let read = std::fs::File::open(path).and_then(|file| file.take(HEAD).read_to_end(&mut head));
    read.is_ok() && crate::frontmatter::parse(&String::from_utf8_lossy(&head)).is_some_and(|fields| fields.is_book())
}

pub fn is_book(cache: &Books, path: &Path, key: &str) -> bool {
    if !aquilum_core::search::paths::is_note(path) || aquilum_core::search::paths::is_canvas(path) {
        return false;
    }
    let Some(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()).ok() else { return false };
    let mut cache = cache.borrow_mut();
    if let Some((seen, book)) = cache.get(key).filter(|(seen, _)| *seen == modified) {
        let _ = seen;
        return *book;
    }
    let book = read_is_book(path);
    cache.insert(key.to_owned(), (modified, book));
    book
}

/// Forgets what was known about `paths` and reads them again; `true` if a note shown in the tree became (or stopped being) a book.
pub fn recheck(cache: &Books, paths: &[std::path::PathBuf], comparable: impl Fn(&Path) -> String) -> bool {
    let mut changed = false;
    for path in paths {
        let key = comparable(path);
        let before = cache.borrow_mut().remove(&key).map(|(_, book)| book);
        if let Some(before) = before {
            changed |= is_book(cache, path, &key) != before;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_notes_with_the_book_type_are_books_and_a_change_is_noticed() {
        let dir = tempfile::tempdir().unwrap();
        let (book, note, canvas) = (dir.path().join("Libro.md"), dir.path().join("Nota.md"), dir.path().join("Hoja.canvas"));
        std::fs::write(&book, "---\ncover: true\ntype: book\nauthor: A\n---\n\ntext").unwrap();
        std::fs::write(&note, "---\ntype: note\n---\n\ntype: book").unwrap();
        std::fs::write(&canvas, "{}").unwrap();
        let cache = Books::default();
        assert!(is_book(&cache, &book, "libro"));
        assert!(!is_book(&cache, &note, "nota"));
        assert!(!is_book(&cache, &canvas, "hoja"));
        std::fs::write(&note, "---\ntype: book\n---\n").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options().write(true).open(&note).unwrap().set_modified(later).unwrap();
        assert!(is_book(&cache, &note, "nota"), "a note that became a book is noticed");
    }

    #[test]
    fn a_note_that_becomes_a_book_changes_its_row_without_reloading_the_tree() {
        use crate::ui::file_tree::model::TreeModel;
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("Заметка.md");
        std::fs::write(&note, "---
tags: []
---

текст").unwrap();
        let mut model = TreeModel::new(Some(dir.path().to_path_buf()), []);
        let book = |model: &TreeModel| model.rows().iter().find(|row| row.path == note).map(|row| row.book);
        assert_eq!(book(&model), Some(false));
        assert!(!model.notes_edited(std::slice::from_ref(&note)), "an unchanged note changes nothing");
        std::fs::write(&note, "---
type: book
tags: []
---

текст").unwrap();
        assert!(model.notes_edited(std::slice::from_ref(&note)));
        assert_eq!(book(&model), Some(true));
        std::fs::write(&note, "текст").unwrap();
        assert!(model.notes_edited(std::slice::from_ref(&note)));
        assert_eq!(book(&model), Some(false));
    }
}
