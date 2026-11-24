use std::path::{Path, PathBuf};
use std::sync::Arc;

use masonry::app::RenderRoot;

use super::super::App;
use super::BOOK_EXTENSIONS;
use crate::frontmatter::{self, BOOK_COVER, BOOK_FILE, COVER, PAGE_COVER, PAGE_COVER_POSITION, PAGES, READER_POSITION, READ_PERCENT};
use crate::ui::widgets::TextButton;
use crate::vault_files::{self, Source};

#[derive(Debug)]
pub(in crate::app) enum Picked {
    Workspace(PathBuf),
    NewVault(Result<PathBuf, String>),
    CloudVault(PathBuf),
    PageCover(PathBuf, PathBuf),
    BookCover(PathBuf, PathBuf),
    Book(PathBuf, Option<PathBuf>),
}

const BYTES_PER_PAGE: u64 = 1024;

pub(in crate::app) fn synthetic_pages(existing: Option<&str>, bytes: u64) -> String {
    let total = u32::try_from(bytes.div_ceil(BYTES_PER_PAGE).max(1)).unwrap_or(u32::MAX);
    let current = existing
        .and_then(frontmatter::parse_pages)
        .map_or(0, |(current, known)| ((f64::from(current) / f64::from(known)).clamp(0.0, 1.0) * f64::from(total)).round() as u32);
    format!("{current}/{total}")
}

impl App {
    pub(super) fn book_pressed(&mut self, root: &mut RenderRoot) {
        let fields = self.fields();
        let file = fields.as_ref().and_then(|f| f.text(BOOK_FILE).map(str::to_owned));
        if let Some(file) = file {
            if let Some(path) = vault_files::resolve(self.tree.root(), &file)
                && let Some(note) = self.note.as_ref().map(|n| n.path.clone())
            {
                let title = note.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let position = fields.as_ref().and_then(|f| f.text(READER_POSITION).map(str::to_owned));
                self.open_reader(root, super::super::reader::Opening { file: path, page: Some(note), title, initial: position, peek: false });
            }
            return;
        }
        self.pick_book(root);
    }

    pub(super) fn pick_book(&mut self, root: &mut RenderRoot) {
        let Some(note) = self.note.as_ref().map(|n| n.path.clone()) else { return };
        self.set_book_busy(root, true);
        let filter = crate::i18n::t("book.filesFilter");
        self.pick(move || Some(Picked::Book(note, crate::native_dialog::pick_file(&filter, &BOOK_EXTENSIONS))));
    }

    pub(in crate::app) fn pick(&self, choose: impl FnOnce() -> Option<Picked> + Send + 'static) {
        let (sender, wake) = (self.picked_tx.clone(), Arc::clone(&self.workspace.wake));
        std::thread::spawn(move || {
            if let Some(picked) = choose()
                && sender.send(picked).is_ok()
            {
                wake();
            }
        });
    }

    fn set_book_busy(&self, root: &mut RenderRoot, busy: bool) {
        if let Some(ids) = self.note_ids.as_ref().map(|n| &n.hero.1) {
            for button in [ids.book_button.map(|b| b.id()), ids.book_menu].into_iter().flatten() {
                root.edit_widget(button, |mut b| TextButton::set_disabled(&mut b.downcast(), busy));
            }
        }
    }

    /// Gives the open note the book's title, but only while its name is still a placeholder (untitled, or the name of a template);
    /// a name the user chose is left alone.
    fn name_note_after_book(&mut self, root: &mut RenderRoot, title: &str) {
        let Some(from) = self.note.as_ref().map(|n| n.path.clone()) else { return };
        let mut placeholders: Vec<String> = self.scan_templates_names();
        placeholders.push(crate::file_ops::untitled());
        if let Some(to) = book_note_target(&from, title, &placeholders, |path| path.exists()) {
            self.rename_file(root, from, to);
        }
    }

    fn import_book(&mut self, root: &mut RenderRoot, file: &Path) {
        let Some(workspace) = self.tree.root().map(Path::to_path_buf) else { return };
        match vault_files::import(&self.workspace.core, &workspace, Source::Path(file), "book", "epub") {
            Ok(relative) => {
                let bytes = std::fs::metadata(file).map_or(0, |m| m.len());
                self.reset_book_position();
                let pages = if crate::reader::pdf::is_pdf(file) { String::new() } else { synthetic_pages(None, bytes) };
                let mut fields = vec![(BOOK_FILE, relative), (PAGES, pages), (READER_POSITION, String::new()), (READ_PERCENT, "0".into())];
                // The book's own cover becomes its thumbnail; a book without one keeps the current thumbnail.
                if let Some((bytes, extension)) = crate::reader::cover::of(file)
                    && let Some(cover) = self.import_cover(Source::Bytes(&bytes, &format!("cover.{extension}")))
                {
                    fields.push((BOOK_COVER, cover));
                }
                self.set_fields(root, &fields);
                if let Some(title) = crate::reader::cover::title(file) {
                    self.name_note_after_book(root, &title);
                }
            }
            Err(error) => report!("book not loaded: {error:?}"),
        }
    }

    pub(super) fn pick_image(&self, page_cover: bool) {
        let Some(note) = self.note.as_ref().map(|n| n.path.clone()) else { return };
        let filter = crate::i18n::t("book.imagesFilter");
        self.pick(move || {
            let file = crate::native_dialog::pick_file(&filter, &crate::images::EXTENSIONS)?;
            Some(if page_cover { Picked::PageCover(note, file) } else { Picked::BookCover(note, file) })
        });
    }

    pub(in crate::app) fn on_picked(&mut self, root: &mut RenderRoot, picked: Picked) -> bool {
        let current = |note: &PathBuf| self.shows(note);
        match picked {
            Picked::Workspace(path) => return self.switch_workspace(root, path).title,
            Picked::NewVault(made) => return self.vault_made(root, made),
            Picked::CloudVault(path) => return self.join_cloud(root, path).title,
            Picked::PageCover(note, file) if current(&note) => {
                if let Some(relative) = self.import_cover(Source::Path(&file)) {
                    self.set_fields(root, &[(COVER, "true".into()), (PAGE_COVER, relative), (PAGE_COVER_POSITION, String::new())]);
                }
            }
            Picked::BookCover(note, file) if current(&note) => {
                if let Some(relative) = self.import_cover(Source::Path(&file)) {
                    self.set_fields(root, &[(BOOK_COVER, relative)]);
                }
            }
            Picked::Book(note, file) => {
                self.set_book_busy(root, false);
                if let Some(file) = file.filter(|_| current(&note)) {
                    self.import_book(root, &file);
                }
            }
            Picked::PageCover(..) | Picked::BookCover(..) => {}
        }
        false
    }

    fn import_cover(&self, source: Source<'_>) -> Option<String> {
        let workspace = self.tree.root()?.to_path_buf();
        let fallback = match source {
            Source::Path(_) => "jpg",
            Source::Bytes(..) => "png",
        };
        vault_files::import(&self.workspace.core, &workspace, source, "cover", fallback)
            .map_err(|error| report!("cover not saved: {error:?}"))
            .ok()
    }

    pub(super) fn paste_book_cover(&mut self, root: &mut RenderRoot) {
        let Some(bytes) = crate::clipboard::png() else { return };
        if let Some(relative) = self.import_cover(Source::Bytes(&bytes, "image.png")) {
            self.set_fields(root, &[(BOOK_COVER, relative)]);
        }
    }
}

/// `name` or `name 2`, `name (2)`: a placeholder name followed by the counter the app adds to avoid clashes.
fn is_placeholder(stem: &str, placeholders: &[String]) -> bool {
    placeholders.iter().any(|base| {
        stem == base
            || stem.strip_prefix(base.as_str()).and_then(|rest| rest.strip_prefix(' ')).is_some_and(|rest| {
                let counter = rest.strip_prefix('(').and_then(|r| r.strip_suffix(')')).unwrap_or(rest);
                !counter.is_empty() && counter.chars().all(|c| c.is_ascii_digit())
            })
    })
}

/// Where the note should move to take the book's title, or `None` to leave it where it is.
fn book_note_target(from: &Path, title: &str, placeholders: &[String], exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let stem = from.file_stem()?.to_string_lossy().into_owned();
    if !is_placeholder(&stem, placeholders) {
        return None;
    }
    let name = crate::file_ops::sanitize_file_name(title);
    if name.is_empty() || name == stem {
        return None;
    }
    let folder = from.parent()?;
    (0..100).map(|i| folder.join(if i == 0 { format!("{name}.md") } else { format!("{name} {i}.md") })).find(|candidate| !exists(candidate))
}

#[cfg(test)]
mod tests {
    use super::synthetic_pages;

    #[test]
    fn synthetic_pages_keep_the_reading_fraction() {
        assert_eq!(synthetic_pages(None, 0), "0/1");
        assert_eq!(synthetic_pages(None, 10 * 1024), "0/10");
        assert_eq!(synthetic_pages(Some("50/100"), 20 * 1024), "10/20");
        assert_eq!(synthetic_pages(Some("5/0"), 4 * 1024), "0/4");
        assert_eq!(synthetic_pages(Some("200/100"), 4 * 1024), "4/4");
    }

    use super::{book_note_target, is_placeholder};
    use std::path::{Path, PathBuf};

    fn names() -> Vec<String> {
        vec!["Book".into(), "Untitled".into()]
    }

    #[test]
    fn only_placeholder_names_count_as_not_chosen_yet() {
        for stem in ["Book", "Book 1", "Book (2)", "Untitled", "Untitled 12"] {
            assert!(is_placeholder(stem, &names()), "{stem}");
        }
        for stem in ["Dune", "Bookshelf", "Book club", "Book 1a", "Book ()", "My Book"] {
            assert!(!is_placeholder(stem, &names()), "{stem}");
        }
    }

    #[test]
    fn a_placeholder_note_takes_the_book_title_and_never_overwrites_another_note() {
        let free = |_: &Path| false;
        let target = book_note_target(Path::new("/v/Book.md"), "Dune: Messiah", &names(), free);
        assert_eq!(target, Some(PathBuf::from("/v/Dune Messiah.md")), "characters a file name cannot hold are replaced");
        let taken = |path: &Path| path == Path::new("/v/Dune.md") || path == Path::new("/v/Dune 1.md");
        assert_eq!(book_note_target(Path::new("/v/Book 3.md"), "Dune", &names(), taken), Some(PathBuf::from("/v/Dune 2.md")));
        assert_eq!(book_note_target(Path::new("/v/My reading.md"), "Dune", &names(), free), None, "a name the user chose stays");
        assert_eq!(book_note_target(Path::new("/v/Book.md"), "  ", &names(), free), None, "an empty title changes nothing");
        assert_eq!(book_note_target(Path::new("/v/Dune.md"), "Dune", &names(), free), None);
    }
}
