mod context;
use context::Context;

use super::*;
use crate::ui::editor::resolve::{Request, Suggestions};
use crate::ui::keymap::{self, Command, Scope};
use masonry::core::WidgetId;
use masonry::core::keyboard::Modifiers;
use aquilum_core::text::matcher::{IGNORE_CASE, Matcher};

const OPEN: &str = "[[";
const CLOSE: &str = "]]";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    Link { wrap: bool, close: bool },
    /// A heading of a note: `[[Note#Heading`.
    Heading { close: bool },
    Key,
    Value,
    /// The whole text of a note-name field.
    Name,
}

impl Source {
    fn lead(self) -> usize {
        match self {
            Source::Link { wrap: false, .. } => OPEN.len(),
            _ => 0,
        }
    }

    fn local(self) -> bool {
        !matches!(self, Source::Link { .. } | Source::Heading { .. } | Source::Name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Suggest {
    from: usize,
    to: usize,
    query: String,
    items: Vec<String>,
    /// Level of each heading (H1 to H6); empty for the other kinds of suggestion.
    hints: Vec<String>,
    selected: usize,
    source: Source,
    /// The note whose headings are listed, to ask again when they arrive.
    note: Option<String>,
}

impl TextEditor {
    fn keep_selected(&self, query: &str, items: &[String]) -> usize {
        self.suggest.as_ref().filter(|s| s.query == query).map_or(0, |s| s.selected.min(items.len().saturating_sub(1)))
    }

    pub(super) fn update_suggest(&mut self, id: WidgetId) {
        if let Some(next) = self.property_suggest() {
            let selected = self.keep_selected(&next.query, &next.items);
            self.suggest = Some(Suggest { selected, ..next });
            return self.publish_suggest(id);
        }
        if self.suggest_names {
            let Some(resolver) = self.doc.resolver.clone() else { return };
            let query = self.doc.whole();
            if query.trim().is_empty() {
                return self.close_suggest();
            }
            let items = match resolver.suggest(query.trim()) {
                Some(items) => items,
                None => self.suggest.as_ref().map(|s| s.items.clone()).unwrap_or_default(),
            };
            let selected = self.keep_selected(&query, &items);
            self.suggest = Some(Suggest { from: 0, to: self.doc.len(), query, items, hints: Vec::new(), selected, source: Source::Name, note: None });
            return self.publish_suggest(id);
        }
        let Some(resolver) = self.doc.resolver.clone() else { return };
        let Some(min) = resolver.suggest_min() else { return self.close_suggest() };
        let Some(context) = self.suggest_context() else { return self.close_suggest() };
        let Context { from, to, query, source, note } = context;
        if let (Source::Heading { .. }, Some(target)) = (source, note.as_deref()) {
            // The list opens as soon as the # is typed, with every heading, and narrows as the title is typed.
            let (items, hints) = match self.heading_items(&resolver, target, &query) {
                Some(found) => found,
                None => self.suggest.as_ref().map(|s| (s.items.clone(), s.hints.clone())).unwrap_or_default(),
            };
            let selected = self.keep_selected(&query, &items);
            self.suggest = Some(Suggest { from, to, query, items, hints, selected, source, note });
            return self.publish_suggest(id);
        }
        if query.chars().count() < min {
            return self.close_suggest();
        }
        let items = match resolver.suggest(&query) {
            Some(items) => items,
            None => self.suggest.as_ref().map(|s| s.items.clone()).unwrap_or_default(),
        };
        let selected = self.keep_selected(&query, &items);
        self.suggest = Some(Suggest { from, to, query, items, hints: Vec::new(), selected, source, note: None });
        self.publish_suggest(id);
    }

    /// Titles and level tags of the headings of `target` that match `query`; `None` while the note is still being read.
    fn heading_items(&self, resolver: &std::sync::Arc<dyn crate::ui::editor::resolve::Resolver>, target: &str, query: &str) -> Option<(Vec<String>, Vec<String>)> {
        let list = if target.trim().is_empty() {
            aquilum_core::search::headings::headings(&self.doc.whole()).into_iter().map(|heading| (heading.level, heading.title)).collect()
        } else {
            resolver.headings(target.trim())?
        };
        let matcher = Matcher::new(query.trim(), IGNORE_CASE);
        let (mut items, mut hints) = (Vec::new(), Vec::new());
        for (level, title) in list.into_iter().filter(|(_, title)| !title.is_empty() && (query.trim().is_empty() || matcher.first(title).is_some())) {
            items.push(title);
            hints.push(format!("H{level}"));
        }
        Some((items, hints))
    }

    pub(super) fn poll_suggest(&mut self, id: WidgetId) {
        let Some(current) = self.suggest.as_ref().filter(|s| !s.source.local()) else { return };
        let Some(resolver) = self.doc.resolver.clone() else { return };
        let (items, hints) = match (current.source, current.note.as_deref()) {
            (Source::Heading { .. }, Some(target)) => match self.heading_items(&resolver, target, &current.query) {
                Some(found) => found,
                None => return,
            },
            _ => match resolver.suggest(&current.query) {
                Some(items) => (items, Vec::new()),
                None => return,
            },
        };
        if items != current.items {
            if let Some(s) = &mut self.suggest {
                s.items = items;
                s.hints = hints;
                s.selected = 0;
            }
            self.publish_suggest(id);
        }
    }

    pub(super) fn publish_suggest(&mut self, id: WidgetId) {
        let Some(resolver) = self.doc.resolver.clone() else { return };
        let view = self.suggest.as_ref().filter(|s| !s.items.is_empty() && self.laid_out()).map(|s| Suggestions {
            editor: id,
            caret: self.caret_at(s.from - s.source.lead(), 0, Affinity::Downstream),
            query: s.query.clone(),
            items: s.items.clone(),
            hints: s.hints.clone(),
            selected: s.selected,
        });
        if view != self.suggest_shown {
            self.suggest_shown = view.clone();
            resolver.request(Request::Suggest(view));
        }
    }

    pub(super) fn close_suggest(&mut self) {
        self.suggest = None;
        if self.suggest_shown.take().is_some()
            && let Some(resolver) = &self.doc.resolver
        {
            resolver.request(Request::Suggest(None));
        }
    }

    pub(super) fn hide_suggest(&mut self) {
        if self.suggest_shown.take().is_some()
            && let Some(resolver) = &self.doc.resolver
        {
            resolver.request(Request::Suggest(None));
        }
    }

    pub fn accept_suggestion(this: &mut WidgetMut<'_, Self>, index: usize) {
        if this.widget.accept_suggest(index) {
            Self::edited(this);
        }
    }

    pub(super) fn suggest_open(&self) -> bool {
        self.suggest_shown.is_some()
    }

    pub(super) fn suggest_key(&mut self, id: WidgetId, key: &Key, modifiers: Modifiers) -> Option<bool> {
        let count = self.suggest.as_ref().map_or(0, |s| s.items.len());
        if !self.suggest_open() || count == 0 {
            return None;
        }
        let page = 8;
        let current = self.suggest.as_ref().map_or(0, |s| s.selected);
        if keymap::closes(key, modifiers) {
            self.close_suggest();
            return Some(false);
        }
        let selected = match keymap::command(&[Scope::List, Scope::Navigation], key, modifiers)? {
            command @ (Command::ListNext | Command::ListPrevious) => keymap::cycle(current, count, command == Command::ListNext),
            Command::PageDown => (current + page).min(count - 1),
            Command::PageUp => current.saturating_sub(page),
            Command::ListOpen | Command::ListCreate => return Some(self.accept_suggest(current)),
            _ => return None,
        };
        if let Some(s) = &mut self.suggest {
            s.selected = selected;
        }
        self.publish_suggest(id);
        Some(false)
    }

    pub(super) fn accept_suggest(&mut self, index: usize) -> bool {
        let Some(s) = self.suggest.clone() else { return false };
        let Some(title) = s.items.get(index) else { return false };
        let insert = match s.source {
            Source::Link { wrap, close } => format!("{}{title}{}", if wrap { OPEN } else { "" }, if close { CLOSE } else { "" }),
            Source::Heading { close } => format!("{}{}", aquilum_core::search::headings::link_text(title), if close { CLOSE } else { "" }),
            Source::Key => format!("{title}: "),
            Source::Value | Source::Name => title.clone(),
        };
        let skip = matches!(s.source, Source::Link { close: false, .. } | Source::Heading { close: false }) && self.doc.after(s.to).starts_with(CLOSE);
        let caret = s.from + insert.len() + if skip { CLOSE.len() } else { 0 };
        self.dispatch(vec![(s.from..s.to, insert)], Some((caret, caret)), Kind::Other);
        self.close_suggest();
        self.suggest_submit = s.source == Source::Name;
        true
    }
}
