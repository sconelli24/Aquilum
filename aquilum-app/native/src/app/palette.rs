//! Command palette: a searchable list of the app's commands, opened with a shortcut.

use aquilum_core::text::matcher::{IGNORE_CASE, Matcher};
use masonry::accesskit::Role;
use masonry::app::RenderRoot;
use masonry::core::keyboard::{Key, Modifiers};
use masonry::core::{ErasedAction, NewWidget, PropertySet, WidgetId};
use masonry::kurbo::Point;
use masonry::layout::Length;
use masonry::properties::Padding;
use masonry::widgets::Flex;

use super::{App, Outcome, Popup};
use crate::i18n::t;
use crate::ui::dialog::{CardSize, DialogAnswer, Modal, card};
use crate::ui::editor::{TextAction, TextEditor};
use crate::ui::handle::Handle;
use crate::ui::icons;
use crate::ui::keymap::{self, Command, Scope};
use crate::ui::scroll::ScrollArea;
use crate::ui::search::{self, CardAction, CardData, CardKind, Marks, Placeholder, ResultCard, empty_state_with, results_list};
use crate::ui::tokens::size;
use crate::ui::widgets::Slot;

/// What a palette row does when chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Command(Command),
    Settings,
    Graph,
    NewCanvas,
}

/// Shortcut commands offered in the palette, in the order they are listed.
const COMMANDS: [Command; 15] = [
    Command::NewNote,
    Command::NewTab,
    Command::CloseTab,
    Command::NextTab,
    Command::PreviousTab,
    Command::Back,
    Command::Forward,
    Command::QuickOpen,
    Command::Templates,
    Command::FindInNote,
    Command::FocusMode,
    Command::WideMode,
    Command::ZoomIn,
    Command::ZoomOut,
    Command::ZoomReset,
];

#[derive(Clone, Debug)]
pub struct Entry {
    pub item: Item,
    pub label: String,
    pub keys: String,
}

pub struct PaletteState {
    pub layer: WidgetId,
    pub field: Handle<TextEditor>,
    pub placeholder: Handle<Placeholder>,
    pub query: String,
    pub body: Handle<Slot>,
    pub list: Option<Handle<ScrollArea<Flex>>>,
    pub entries: Vec<Entry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub cards: Vec<WidgetId>,
}

fn marks(text: &str, query: &str) -> Marks {
    Matcher::new(query, IGNORE_CASE).find_all(text).into_iter().map(|found| (found.start, found.end)).collect()
}

fn filtered(entries: &[Entry], query: &str) -> Vec<usize> {
    let query = query.trim();
    (0..entries.len()).filter(|&i| query.is_empty() || !marks(&entries[i].label, query).is_empty()).collect()
}

/// Every command the palette lists, with its current shortcut.
pub fn entries() -> Vec<Entry> {
    let mut entries: Vec<Entry> = COMMANDS
        .iter()
        .filter_map(|&command| {
            let label = keymap::title(Scope::Global, command)?;
            Some(Entry { item: Item::Command(command), label: t(&format!("settings.shortcuts.{label}")), keys: keymap::label(Scope::Global, command) })
        })
        .collect();
    let plain = |item, label: &str| Entry { item, label: t(label), keys: String::new() };
    entries.push(plain(Item::NewCanvas, "fileTree.newCanvas"));
    entries.push(plain(Item::Graph, "palette.openGraph"));
    entries.push(plain(Item::Settings, "palette.openSettings"));
    entries
}

impl App {
    pub fn open_palette(&mut self, root: &mut RenderRoot) {
        if matches!(self.popup, Some(Popup::Palette(_))) {
            return;
        }
        self.commit_note();
        let search_box = search::find_box(t("palette.find"));
        let body = Slot::fit(NewWidget::new(Flex::column()).erased());
        let body_handle = Handle::of(&body);
        let card = card(search_box.widget, body, search::template_footer());
        let layer = Modal::new(card, &t("palette.title"), CardSize::Anchored(search::card_size), Role::Dialog, false);
        let layer_id = layer.id();
        root.add_layer(layer, Point::ORIGIN);
        root.focus_on(Some(search_box.field.id()));

        let entries = entries();
        let filtered = (0..entries.len()).collect();
        self.popup = Some(Popup::Palette(PaletteState {
            layer: layer_id,
            field: search_box.field,
            placeholder: search_box.placeholder,
            query: String::new(),
            body: body_handle,
            list: None,
            entries,
            filtered,
            selected: 0,
            cards: Vec::new(),
        }));
        self.render_palette(root);
    }

    fn close_palette(&mut self, root: &mut RenderRoot) {
        if let Some(Popup::Palette(state)) = self.popup.take_if(|popup| matches!(popup, Popup::Palette(_))) {
            crate::ui::dialog::close(root, state.layer);
            if let Some(ids) = &self.note_ids {
                root.focus_on(Some(ids.body.id()));
            }
        }
    }

    fn render_palette(&mut self, root: &mut RenderRoot) {
        let Some(Popup::Palette(state)) = &mut self.popup else { return };
        if state.filtered.is_empty() {
            state.cards.clear();
            state.list = None;
            let empty = empty_state_with(icons::SEARCH, &t("palette.empty"), None, None);
            let empty = empty.with_props(PropertySet::new().with(Padding::all(Length::px(size::SPACE_24))));
            let body = state.body;
            body.edit(root, |mut slot| Slot::set_child(&mut slot, empty.erased()));
            return;
        }
        let query = state.query.trim().to_owned();
        let cards: Vec<_> = state
            .filtered
            .iter()
            .enumerate()
            .map(|(row, &index)| {
                let entry = &state.entries[index];
                let data = CardData {
                    title_marks: marks(&entry.label, &query),
                    title: entry.label.clone(),
                    extension: entry.keys.clone(),
                    snippet: String::new(),
                    count: String::new(),
                    extension_marks: Vec::new(),
                    snippet_marks: Vec::new(),
                };
                ResultCard::template(data, row == state.selected)
            })
            .collect();
        let (column, ids) = results_list(cards);
        state.cards = ids;
        let scroll = NewWidget::new(ScrollArea::new(column));
        state.list = Some(Handle::of(&scroll));
        let body = state.body;
        body.edit(root, |mut slot| Slot::set_child(&mut slot, scroll.erased()));
    }

    fn select_palette_row(&mut self, root: &mut RenderRoot, index: usize) {
        let Some(Popup::Palette(state)) = &mut self.popup else { return };
        super::search::select_card(root, CardKind::Template, &state.cards, state.list, &mut state.selected, index);
    }

    fn palette_query_changed(&mut self, root: &mut RenderRoot, query: String) {
        let Some(Popup::Palette(state)) = &mut self.popup else { return };
        state.filtered = filtered(&state.entries, &query);
        state.selected = 0;
        let (placeholder, empty): (Handle<Placeholder>, bool) = (state.placeholder, query.is_empty());
        state.query = query;
        placeholder.edit(root, |mut p| Placeholder::set_empty(&mut p, empty));
        self.render_palette(root);
    }

    pub fn palette_key(&mut self, root: &mut RenderRoot, key: &Key, modifiers: Modifiers) -> Option<Outcome> {
        let Some(Popup::Palette(state)) = &self.popup else { return None };
        let count = state.filtered.len();
        match keymap::command(&[Scope::List], key, modifiers)? {
            command @ (Command::ListNext | Command::ListPrevious) => {
                if count > 0 {
                    self.select_palette_row(root, keymap::cycle(state.selected, count, command == Command::ListNext));
                }
                Some(Outcome::default())
            }
            Command::ListOpen => Some(if count > 0 { self.run_palette_row(root, state.selected) } else { Outcome::default() }),
            _ => None,
        }
    }

    /// Closes the palette and runs the chosen row.
    fn run_palette_row(&mut self, root: &mut RenderRoot, row: usize) -> Outcome {
        let Some(Popup::Palette(state)) = &self.popup else { return Outcome::default() };
        let Some(entry) = state.filtered.get(row).and_then(|&index| state.entries.get(index)) else { return Outcome::default() };
        let item = entry.item;
        self.close_palette(root);
        match item {
            Item::Command(command) => self.command(root, command).unwrap_or_default(),
            Item::Settings => {
                self.open_settings(root);
                Outcome::default()
            }
            Item::Graph => {
                self.open_graph(root);
                Outcome { title: true, ..Outcome::default() }
            }
            Item::NewCanvas => {
                let folder = self.tree.root().map(std::path::Path::to_path_buf);
                let title = folder.is_some_and(|folder| self.create_canvas_in(root, &folder));
                Outcome { title, ..Outcome::default() }
            }
        }
    }

    pub fn on_palette_action(&mut self, root: &mut RenderRoot, id: WidgetId, action: &ErasedAction) -> Outcome {
        let Some(Popup::Palette(state)) = &self.popup else { return Outcome::default() };
        if id == state.layer && matches!(action.downcast_ref::<DialogAnswer>(), Some(DialogAnswer::Cancel)) {
            self.close_palette(root);
            return Outcome::default();
        }
        if id == state.field.id() {
            if let Some(TextAction::Changed(query)) = action.downcast_ref::<TextAction>() {
                self.palette_query_changed(root, query.clone());
            }
            return Outcome::default();
        }
        if let Some(index) = state.cards.iter().position(|&card| card == id) {
            match action.downcast_ref::<CardAction>() {
                Some(CardAction::Hover) => self.select_palette_row(root, index),
                Some(CardAction::Open { .. }) => return self.run_palette_row(root, index),
                None => {}
            }
        }
        Outcome::default()
    }
}
