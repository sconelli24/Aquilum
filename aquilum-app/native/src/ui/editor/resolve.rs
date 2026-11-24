use std::collections::HashSet;
use std::sync::Arc;

use masonry::kurbo::Point;
use masonry::peniko::ImageBrush;

use super::table::Command;

#[derive(Clone, Debug, PartialEq)]
pub enum Fetch<T> {
    Pending,
    Ready(T),
    Missing(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    Link { text: String, target: String },
    Progress(f64),
    Check { done: bool, target: String, line: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Table,
    List,
    Tasks,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub shape: Shape,
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Arc<[Vec<Vec<Part>>]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Book {
    pub title: String,
    pub linked: bool,
    pub author: String,
    pub cover: Option<ImageBrush>,
    pub pages: Option<(u32, u32)>,
    pub file: Option<std::path::PathBuf>,
    pub page: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Media {
    pub image: Option<ImageBrush>,
    pub natural: (u32, u32),
    pub video: bool,
    pub path: Option<std::path::PathBuf>,
}

/// Why an embedded note is not drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// The note is already being shown further up, so showing it again would never end.
    Loop,
    /// Embeds go only so many notes deep.
    Depth,
}

/// A note, or part of one, shown inside another note.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteText {
    pub path: std::path::PathBuf,
    pub title: String,
    /// The heading or block id the embed points at, if any.
    pub label: Option<String>,
    pub text: Arc<str>,
    /// Where `text` starts in the file, so opening the note can land there.
    pub start: usize,
    pub blocked: Option<Blocked>,
}

#[derive(Clone, Debug)]
pub enum Request {
    TableMenu { at: Point, commands: Vec<(Command, bool)> },
    Open { target: String, wiki: bool, new_tab: bool },
    View(ImageBrush, Option<std::path::PathBuf>),
    ToggleTask { target: String, line: usize },
    Read { file: std::path::PathBuf, page: Option<std::path::PathBuf>, title: String },
    Suggest(Option<Suggestions>),
    LinkTitle { editor: masonry::core::WidgetId, placeholder: String, url: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestions {
    pub editor: masonry::core::WidgetId,
    pub caret: masonry::kurbo::Rect,
    pub query: String,
    pub items: Vec<String>,
    pub selected: usize,
}

pub trait Resolver: Send + Sync {
    fn dataview(&self, query: &str) -> (Fetch<Grid>, u64);
    fn media(&self, source: &str, wiki: bool) -> (Fetch<Media>, u64);
    fn book(&self, target: &str) -> (Fetch<Book>, u64);
    /// A note, a section (`anchor` is a heading) or a block (`anchor` is `^id`) to show inside the open note.
    fn note(&self, _target: &str, _anchor: Option<&str>) -> (Fetch<NoteText>, u64) {
        (Fetch::Missing(String::new()), 0)
    }
    /// What resolves the embeds of a note that is itself shown inside this one: one level deeper, aware of the notes above.
    fn nested(&self, _note: &std::path::Path) -> Option<Arc<dyn Resolver>> {
        None
    }
    fn request(&self, request: Request);
    fn sweep(&self) {}
    fn suggest(&self, _query: &str) -> Option<Vec<String>> {
        None
    }
    fn suggest_min(&self) -> Option<usize> {
        None
    }
    fn unresolved(&self) -> (Arc<HashSet<String>>, u64) {
        (Arc::default(), 0)
    }
    fn resolve(&self, _targets: Vec<String>) {}
}
