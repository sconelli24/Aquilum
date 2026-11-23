use std::path::PathBuf;

use aquilum_core::canvas::Node;
use masonry::app::RenderRoot;
use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::kurbo::{Point, Rect};

use super::Bound;
use crate::app::{App, Outcome, Popup};
use crate::canvas::geometry::{self, snap};
use crate::i18n::t;
use crate::ui::canvas::{CanvasAction, CanvasView, Prompt};
use crate::ui::editor::{TextAction, TextEditor};
use crate::ui::handle::Handle;
use crate::ui::icons;
use crate::ui::menu::{Entry, Menu, MenuChoice, Submenu, Swatch};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum CanvasCommand {
    AddCard,
    AddNote,
    AddLink,
    AddGroup,
    Edit,
    Open,
    Rename,
    EditLabel,
    EditUrl,
    Color,
    Delete,
    Fit,
}

const COLOR_NAMES: [&str; 6] = ["canvas.colorRed", "canvas.colorOrange", "canvas.colorYellow", "canvas.colorGreen", "canvas.colorCyan", "canvas.colorPurple"];

impl CanvasCommand {
    fn entry(self) -> (&'static str, icons::Icon) {
        match self {
            CanvasCommand::AddCard => ("canvas.addCard", icons::SQUARE_PEN),
            CanvasCommand::AddNote => ("canvas.addNote", icons::FILE_TEXT),
            CanvasCommand::AddLink => ("canvas.addLink", icons::ARROW_UP_RIGHT),
            CanvasCommand::AddGroup => ("canvas.addGroup", icons::FOLDER),
            CanvasCommand::Edit => ("canvas.edit", icons::PENCIL),
            CanvasCommand::Open => ("canvas.openNote", icons::FILE),
            CanvasCommand::Rename => ("canvas.renameGroup", icons::TYPE),
            CanvasCommand::EditLabel => ("canvas.editLabel", icons::TAG),
            CanvasCommand::EditUrl => ("canvas.editUrl", icons::PENCIL),
            CanvasCommand::Color => ("canvas.color", icons::PALETTE),
            CanvasCommand::Delete => ("common.delete", icons::TRASH),
            CanvasCommand::Fit => ("canvas.fit", icons::ZOOM_IN),
        }
    }
}

impl App {
    fn is_canvas_widget(&self, id: WidgetId) -> bool {
        self.canvas.view.is_some_and(|v| v.id() == id) || self.canvas.editors.contains_key(&id) || self.canvas.prompt.as_ref().is_some_and(|(h, _)| h.id() == id)
    }

    pub(in crate::app) fn on_canvas_action(&mut self, root: &mut RenderRoot, id: WidgetId, action: &ErasedAction) -> Option<Outcome> {
        if !self.is_canvas_widget(id) {
            return None;
        }
        if let Some(action) = action.downcast_ref::<CanvasAction>() {
            return Some(self.canvas_action(root, action.clone()));
        }
        let text = action.downcast_ref::<TextAction>()?;
        if let Some((handle, target)) = self.canvas.prompt.clone().filter(|(h, _)| h.id() == id) {
            match text {
                TextAction::Entered(answer) => self.prompt_answered(root, handle, &target, answer),
                TextAction::Cancelled => self.close_canvas_prompt(root),
                _ => {}
            }
            return Some(Outcome::default());
        }
        match (self.canvas.editors.get(&id).cloned(), text) {
            (Some(Bound::Text(node)), TextAction::Changed(value)) => {
                if self.canvas.doc.as_mut().is_some_and(|doc| doc.set_text(&node, value))
                    && let Some(view) = self.canvas.view
                {
                    view.edit(root, |mut v| CanvasView::set_text(&mut v, &node, value));
                }
            }
            (Some(Bound::Note(path)), TextAction::Edited(edits)) => self.note_card_edited(root, id, &path, edits),
            (Some(_), TextAction::Cancelled) => {
                if let Some(view) = self.canvas.view {
                    view.edit(root, |mut v| CanvasView::stop_editing(&mut v));
                    root.focus_on(Some(view.id()));
                }
                self.commit_canvas();
            }
            _ => {}
        }
        Some(Outcome::default())
    }

    fn canvas_action(&mut self, root: &mut RenderRoot, action: CanvasAction) -> Outcome {
        match action {
            CanvasAction::Edit(canvas) => self.commit_canvas_edit(root, canvas),
            CanvasAction::Undo { redo } => {
                let restored = self.canvas.doc.as_mut().and_then(|doc| doc.undo(redo).then(|| doc.canvas().clone()));
                if let Some(canvas) = restored {
                    self.canvas_replaced(root, canvas);
                }
            }
            CanvasAction::Camera { x, y, scale } => {
                let path = self.canvas.doc.as_ref().map(|d| d.path().to_path_buf());
                if let Some((workspace, file)) = path.and_then(|p| self.canvas_key(&p)) {
                    let view = aquilum_core::ui_state::models::CanvasView { center_x: x, center_y: y, scale };
                    if let Err(error) = self.workspace.core.ui_state.save_canvas_view(workspace, &file, view, crate::dates::now_ms()) {
                        report!("canvas view not saved: {error:?}");
                    }
                }
            }
            CanvasAction::Editing(Some(node)) => self.canvas_focus_editor(root, &node),
            CanvasAction::Editing(None) => self.commit_canvas(),
            CanvasAction::Open { file, new_tab } => {
                if let Some(root_path) = self.tree.root() {
                    let path = crate::paths::absolute(root_path, &file);
                    if !self.open_in(root, path.clone(), new_tab) && path.exists() {
                        crate::system::reveal(&path);
                    }
                    return Outcome { title: true, ..Outcome::default() };
                }
            }
            CanvasAction::OpenUrl(url) => crate::system::open_url(&url),
            CanvasAction::Menu { at, world, node, edge } => self.open_canvas_menu(root, at, world, node, edge),
            CanvasAction::Prompt { target, text } => self.ask_canvas(root, target, &text),
            CanvasAction::PromptClosed => self.canvas.prompt = None,
        }
        Outcome::default()
    }

    fn ask_canvas(&mut self, root: &mut RenderRoot, target: Prompt, text: &str) {
        let Some(view) = self.canvas.view else { return };
        let editor = NewWidget::new(TextEditor::single_line(text).styled(crate::ui::style::editor_text)).with_class(crate::ui::style::EDITOR_TEXT);
        let handle = Handle::of(&editor);
        let field = crate::ui::canvas::prompt_field(editor);
        view.edit(root, |mut v| CanvasView::set_prompt(&mut v, target.clone(), field));
        self.canvas.prompt = Some((handle, target));
        root.focus_on(Some(handle.id()));
        handle.edit(root, |mut e| {
            let len = e.widget.len();
            TextEditor::select_byte_range(&mut e, 0, len);
        });
    }

    fn close_canvas_prompt(&mut self, root: &mut RenderRoot) {
        self.canvas.prompt = None;
        if let Some(view) = self.canvas.view {
            view.edit(root, |mut v| CanvasView::close_prompt(&mut v));
            root.focus_on(Some(view.id()));
        }
    }

    fn prompt_answered(&mut self, root: &mut RenderRoot, _handle: Handle<TextEditor>, target: &Prompt, answer: &str) {
        self.close_canvas_prompt(root);
        let Some(view) = self.canvas.view else { return };
        let canvas = match target {
            Prompt::Note(world) => {
                let Some(file) = self.find_note(answer) else {
                    let message = crate::i18n::t_with("canvas.noteNotFound", &[("name", answer.trim())]);
                    view.edit(root, |mut v| CanvasView::flash(&mut v, message));
                    return;
                };
                let node = Node::file(&file, snap(world.x), snap(world.y), 400.0, 400.0);
                view.edit(root, |mut v| CanvasView::add_nodes(&mut v, vec![node]))
            }
            target => view.edit(root, |mut v| CanvasView::answer(&mut v, target, answer)).flatten(),
        };
        if let Some(canvas) = canvas {
            self.commit_canvas_edit(root, canvas);
        }
    }

    /// Vault-relative path of the note a typed name or path points at.
    fn find_note(&self, name: &str) -> Option<String> {
        let root = self.tree.root()?;
        let name = name.trim().trim_start_matches("[[").trim_end_matches("]]").trim();
        if name.is_empty() {
            return None;
        }
        let direct = [name.to_owned(), format!("{name}.md")].into_iter().map(|n| crate::paths::absolute(root, &n)).find(|p| p.is_file());
        let found = direct.or_else(|| {
            let key = name.to_lowercase();
            aquilum_core::search::paths::markdown_files(root).find(|p| aquilum_core::search::paths::title(p).to_lowercase() == key)
        })?;
        crate::paths::relative(root, &found)
    }

    fn open_canvas_menu(&mut self, root: &mut RenderRoot, at: Point, world: Point, node: Option<String>, edge: Option<String>) {
        let Some(doc) = &self.canvas.doc else { return };
        if doc.broken.is_some() {
            return;
        }
        let canvas = doc.canvas();
        let kind = node.as_deref().and_then(|id| canvas.node(id)).map(|n| (n.kind(), n.file.clone()));
        use aquilum_core::canvas::NodeKind as K;
        let commands: Vec<CanvasCommand> = match (&kind, &edge) {
            (Some((K::Text, _)), _) => vec![CanvasCommand::Edit, CanvasCommand::Color, CanvasCommand::Delete],
            (Some((K::File, file)), _) => {
                let note = file.as_deref().is_some_and(|f| aquilum_core::search::paths::is_note(std::path::Path::new(f)));
                let mut list = if note { vec![CanvasCommand::Edit, CanvasCommand::Open] } else { vec![CanvasCommand::Open] };
                list.extend([CanvasCommand::Color, CanvasCommand::Delete]);
                list
            }
            (Some((K::Link, _)), _) => vec![CanvasCommand::Open, CanvasCommand::EditUrl, CanvasCommand::Color, CanvasCommand::Delete],
            (Some((K::Group, _)), _) => vec![CanvasCommand::Rename, CanvasCommand::Color, CanvasCommand::Delete],
            (Some((K::Other, _)), _) => vec![CanvasCommand::Delete],
            (None, Some(_)) => vec![CanvasCommand::EditLabel, CanvasCommand::Color, CanvasCommand::Delete],
            (None, None) => vec![CanvasCommand::AddCard, CanvasCommand::AddNote, CanvasCommand::AddLink, CanvasCommand::AddGroup, CanvasCommand::Fit],
        };
        let labels: Vec<(String, icons::Icon)> = commands.iter().map(|c| c.entry()).map(|(key, icon)| (t(key), icon)).collect();
        let entries: Vec<Entry<'_>> = labels
            .iter()
            .zip(&commands)
            .map(|((label, icon), command)| Entry { label, icon: Some(*icon), disabled: false, divided: *command == CanvasCommand::Delete })
            .collect();
        let title = t("canvas.title");
        let menu = match commands.iter().position(|c| *c == CanvasCommand::Color) {
            Some(color_at) => {
                let names: Vec<String> = std::iter::once(t("palette.none")).chain(COLOR_NAMES.iter().map(|k| t(k))).collect();
                let colors = std::iter::once(None).chain((1..=6).map(|i| geometry::color(Some(&i.to_string()))));
                let swatches: Vec<Swatch<'_>> = names.iter().zip(colors).map(|(label, color)| Swatch { label, color }).collect();
                Menu::with_submenu(&title, &entries, &Submenu { at: color_at, items: &swatches, checked: 0 })
            }
            None => Menu::entries(&title, &entries),
        };
        self.canvas.menu = Some((world, node, edge));
        self.show_menu(root, menu, Rect::from_points(at, at), |layer| Popup::CanvasMenu { layer, commands });
    }

    pub(in crate::app) fn on_canvas_menu(&mut self, root: &mut RenderRoot, layer: WidgetId, commands: Vec<CanvasCommand>, action: &ErasedAction) -> Outcome {
        let Some(choice) = action.downcast_ref::<MenuChoice>() else {
            self.popup = Some(Popup::CanvasMenu { layer, commands });
            return Outcome::default();
        };
        match choice {
            MenuChoice::Hover(index) => {
                let color = commands.get(*index) == Some(&CanvasCommand::Color);
                root.edit_widget(layer, |mut menu| Menu::show_sub(&mut menu.downcast(), color));
                self.popup = Some(Popup::CanvasMenu { layer, commands });
                Outcome::default()
            }
            MenuChoice::Sub(index) => {
                root.remove_layer(layer);
                let color = index.checked_sub(1).map(|i| (i + 1).to_string());
                let canvas = self.canvas.view.and_then(|v| v.edit(root, |mut v| CanvasView::set_color(&mut v, color)).flatten());
                if let Some(canvas) = canvas {
                    self.commit_canvas_edit(root, canvas);
                }
                Outcome::default()
            }
            MenuChoice::Item(index) => {
                root.remove_layer(layer);
                match commands.get(*index) {
                    Some(&command) => self.canvas_command(root, command),
                    None => Outcome::default(),
                }
            }
            MenuChoice::Dismissed => Outcome::default(),
        }
    }

    fn canvas_command(&mut self, root: &mut RenderRoot, command: CanvasCommand) -> Outcome {
        let Some(view) = self.canvas.view else { return Outcome::default() };
        let Some((world, node, edge)) = self.canvas.menu.take() else { return Outcome::default() };
        let snapped = Point::new(snap(world.x), snap(world.y));
        let node_ref = node.as_deref().and_then(|id| self.canvas.doc.as_ref()?.canvas().node(id).cloned());
        let added = match command {
            CanvasCommand::AddCard => {
                let card = Node::text("", snapped.x, snapped.y, 260.0, 120.0);
                let id = card.id.clone();
                let canvas = view.edit(root, |mut v| CanvasView::add_nodes(&mut v, vec![card]));
                if let Some(canvas) = canvas {
                    self.commit_canvas_edit(root, canvas);
                    view.edit(root, |mut v| CanvasView::begin_edit(&mut v, &id));
                    self.canvas_focus_editor(root, &id);
                }
                None
            }
            CanvasCommand::AddNote => {
                self.ask_canvas(root, Prompt::Note(snapped), "");
                None
            }
            CanvasCommand::AddLink => {
                let link = Node::link("", snapped.x, snapped.y, 400.0, 120.0);
                let id = link.id.clone();
                let canvas = view.edit(root, |mut v| CanvasView::add_nodes(&mut v, vec![link]));
                if let Some(canvas) = canvas {
                    self.commit_canvas_edit(root, canvas);
                    self.ask_canvas(root, Prompt::LinkUrl(id), "https://");
                }
                None
            }
            CanvasCommand::AddGroup => {
                let around = view.get(root).and_then(|v| v.inner().selection_bounds()).map(|r| r.inflate(40.0, 40.0));
                let rect = around.unwrap_or_else(|| Rect::from_origin_size(snapped, (480.0, 320.0)));
                let group = Node::group(Some(&t("canvas.group")), rect.x0, rect.y0, rect.width(), rect.height());
                let id = group.id.clone();
                let canvas = view.edit(root, |mut v| CanvasView::add_nodes(&mut v, vec![group]));
                if let Some(canvas) = canvas {
                    self.commit_canvas_edit(root, canvas);
                    self.ask_canvas(root, Prompt::GroupLabel(id), &t("canvas.group"));
                }
                None
            }
            CanvasCommand::Edit => {
                if let Some(id) = node {
                    view.edit(root, |mut v| CanvasView::begin_edit(&mut v, &id));
                    self.canvas_focus_editor(root, &id);
                }
                None
            }
            CanvasCommand::Open => {
                match node_ref {
                    Some(n) if n.url.is_some() => crate::system::open_url(n.url.as_deref().unwrap_or_default()),
                    Some(n) => {
                        if let (Some(file), Some(root_path)) = (n.file.as_deref(), self.tree.root()) {
                            let path: PathBuf = crate::paths::absolute(root_path, file);
                            if !self.open_in(root, path.clone(), true) && path.exists() {
                                crate::system::reveal(&path);
                            }
                            return Outcome { title: true, ..Outcome::default() };
                        }
                    }
                    None => {}
                }
                None
            }
            CanvasCommand::Rename => {
                if let Some(n) = node_ref {
                    self.ask_canvas(root, Prompt::GroupLabel(n.id.clone()), n.label.as_deref().unwrap_or_default());
                }
                None
            }
            CanvasCommand::EditUrl => {
                if let Some(n) = node_ref {
                    self.ask_canvas(root, Prompt::LinkUrl(n.id.clone()), n.url.as_deref().unwrap_or_default());
                }
                None
            }
            CanvasCommand::EditLabel => {
                let label = edge.as_deref().and_then(|id| self.canvas.doc.as_ref()?.canvas().edges.iter().find(|e| e.id == id)?.label.clone());
                if let Some(id) = edge {
                    self.ask_canvas(root, Prompt::EdgeLabel(id), label.as_deref().unwrap_or_default());
                }
                None
            }
            CanvasCommand::Color => None,
            CanvasCommand::Delete => view.edit(root, |mut v| CanvasView::delete_selection(&mut v)).flatten(),
            CanvasCommand::Fit => {
                view.edit(root, |mut v| CanvasView::fit_all(&mut v));
                None
            }
        };
        if let Some(canvas) = added {
            self.commit_canvas_edit(root, canvas);
        }
        Outcome::default()
    }
}
