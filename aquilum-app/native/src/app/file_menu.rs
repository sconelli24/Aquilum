use super::*;
use crate::ui::menu::{Submenu, Swatch};
use crate::ui::settings::controls::{Changed, DropdownButton};
use crate::ui::palette;
use masonry::kurbo::Rect;

impl App {
    pub(super) fn open_file_menu(
        &mut self,
        root: &mut RenderRoot,
        targets: Vec<PathBuf>,
        clicked: PathBuf,
        folder: bool,
        at: Point,
        tab: Option<Uuid>,
    ) {
        let label = if tab.is_some() {
            t("tabs.actions")
        } else if folder {
            t("fileTree.folderActions")
        } else {
            t("fileTree.fileActions")
        };
        let mut labels = vec![(t("common.rename"), crate::ui::icons::PENCIL), (t("common.duplicate"), crate::ui::icons::COPY), (t("common.delete"), crate::ui::icons::TRASH)];
        if tab.is_some() {
            labels.push((t("tabs.close"), crate::ui::icons::X));
        }
        let colored = folder && tab.is_none();
        if colored {
            labels.push((t("palette.color"), crate::ui::icons::PALETTE));
        }
        let first_create = labels.len();
        if tab.is_none() {
            labels.extend(create_labels());
        }
        let items: Vec<crate::ui::menu::Entry<'_>> = labels
            .iter()
            .enumerate()
            .map(|(at, (label, icon))| crate::ui::menu::Entry { label, icon: Some(*icon), disabled: false, divided: tab.is_none() && at == first_create })
            .collect();
        let names: Vec<String> = std::iter::once(t("palette.none")).chain(palette::NAMES.iter().map(|name| palette::label(name))).collect();
        let theme = crate::ui::theme::current();
        let swatches: Vec<Swatch<'_>> = names.iter().zip(std::iter::once(None).chain(palette::NAMES.iter().map(|name| palette::color(name, &theme)))).map(|(label, color)| Swatch { label, color }).collect();
        let current = self.tree.color(&clicked).and_then(|name| palette::NAMES.iter().position(|known| *known == name)).map_or(0, |at| at + 1);
        let menu = if colored { Menu::with_submenu(&label, &items, &Submenu { at: MENU_COLOR, items: &swatches, checked: current }) } else { Menu::entries(&label, &items) };
        self.show_menu(root, menu, Rect::from_points(at, at), |layer| Popup::FileMenu { layer, targets, clicked, tab });
    }

    /// Right click on the empty part of the tree: the three creation commands for the vault root.
    pub(super) fn open_create_menu(&mut self, root: &mut RenderRoot, at: Point) {
        let Some(folder) = self.tree.root().map(Path::to_path_buf) else { return };
        let labels = create_labels();
        let items: Vec<crate::ui::menu::Entry<'_>> = labels.iter().map(|(label, icon)| crate::ui::menu::Entry { label, icon: Some(*icon), disabled: false, divided: false }).collect();
        let menu = Menu::entries(&t("fileTree.createActions"), &items);
        self.show_menu(root, menu, Rect::from_points(at, at), |layer| Popup::CreateMenu { layer, folder });
    }

    fn create_in(&mut self, root: &mut RenderRoot, create: Create, folder: &Path) -> bool {
        match create {
            Create::Note => self.create_note_in(root, folder),
            Create::Canvas => self.create_canvas_in(root, folder),
            Create::Folder => {
                self.create_folder_in(root, folder);
                false
            }
        }
    }

    pub(super) fn open_sort_menu(&mut self, root: &mut RenderRoot, button: WidgetId) {
        let Some(trigger) = root.get_widget(button).map(|w| w.ctx().bounding_box()) else { return };
        let labels: Vec<String> = crate::ui::file_tree::Sort::ALL.iter().map(|s| t(s.label())).collect();
        let items: Vec<&str> = labels.iter().map(String::as_str).collect();
        let checked = crate::ui::file_tree::Sort::ALL.iter().position(|s| *s == self.tree.sort()).unwrap_or(0);
        let menu = Menu::list(&t("fileTree.sort"), &items, checked, trigger);
        mark_trigger(root, button, true);
        self.show_menu(root, menu, trigger, |layer| Popup::SortMenu { layer, button });
    }

    pub(super) fn open_tabs_menu(&mut self, root: &mut RenderRoot, button: WidgetId) {
        let Some(trigger) = root.get_widget(button).map(|w| w.ctx().bounding_box()) else { return };
        let titles: Vec<String> = self.tabs.tabs().iter().map(crate::tabs::Tab::label).collect();
        let items: Vec<&str> = titles.iter().map(String::as_str).collect();
        let checked = self.tabs.tabs().iter().position(|tab| tab.id == self.tabs.active_id()).unwrap_or(0);
        let width = crate::ui::tokens::size::TAB_LIST_WIDTH;
        let origin = Menu::dropdown_origin(trigger, self.window, width);
        let menu = Menu::dropdown(&t("tabs.list"), &items, checked, trigger, self.window, width);
        mark_trigger(root, button, true);
        self.show_layer(root, menu, origin, |layer| Popup::TabsMenu { layer, button });
    }

    fn dropdown_answer(&mut self, root: &mut RenderRoot, layer: WidgetId, button: WidgetId, id: WidgetId, action: &ErasedAction) -> Option<Option<usize>> {
        if id == button && (action.downcast_ref::<crate::ui::widgets::Pressed>().is_some() || action.downcast_ref::<Changed>().is_some()) {
            root.remove_layer(layer);
            mark_trigger(root, button, false);
            return Some(None);
        }
        let choice = action.downcast_ref::<MenuChoice>()?;
        if let MenuChoice::Item(_) = choice {
            root.remove_layer(layer);
        }
        mark_trigger(root, button, false);
        Some(match choice {
            MenuChoice::Item(index) => Some(*index),
            MenuChoice::Dismissed | MenuChoice::Hover(_) | MenuChoice::Sub(_) => None,
        })
    }

    fn pick_tab(&mut self, root: &mut RenderRoot, index: usize) -> Outcome {
        let Some(id) = self.tabs.tabs().get(index).map(|tab| tab.id) else { return Outcome::default() };
        self.on_titlebar(root, &crate::ui::titlebar::TitlebarAction::Select(id))
    }

    pub(super) fn on_popup_action(&mut self, root: &mut RenderRoot, id: WidgetId, action: &ErasedAction) -> Outcome {
        match self.popup.take() {
            Some(Popup::FileMenu { layer, targets, clicked, tab }) => {
                let Some(choice) = action.downcast_ref::<MenuChoice>() else {
                    self.popup = Some(Popup::FileMenu { layer, targets, clicked, tab });
                    return Outcome::default();
                };
                if let MenuChoice::Hover(index) = choice {
                    let colored = tab.is_none() && clicked.is_dir();
                    root.edit_widget(layer, |mut menu| Menu::show_sub(&mut menu.downcast(), colored && *index == MENU_COLOR));
                    self.popup = Some(Popup::FileMenu { layer, targets, clicked, tab });
                    return Outcome::default();
                }
                if let MenuChoice::Item(_) | MenuChoice::Sub(_) = choice {
                    root.remove_layer(layer);
                }
                match choice {
                    MenuChoice::Sub(index) => {
                        let folders: Vec<PathBuf> = targets.into_iter().filter(|path| path.is_dir()).collect();
                        self.tree.set_color(root, &folders, index.checked_sub(1).and_then(|at| palette::NAMES.get(at).copied()));
                    }
                    MenuChoice::Item(item) if tab.is_none() && let Some(create) = create_choice(*item, clicked.is_dir()) => {
                        let folder = if clicked.is_dir() { Some(clicked) } else { clicked.parent().map(Path::to_path_buf) };
                        let title = folder.is_some_and(|folder| self.create_in(root, create, &folder));
                        return Outcome { title, ..Outcome::default() };
                    }
                    MenuChoice::Item(MENU_DUPLICATE) => file_ops::duplicate_all(&self.workspace.core, targets),
                    MenuChoice::Item(MENU_DELETE) => self.confirm_delete(root, targets),
                    MenuChoice::Item(MENU_CLOSE) if let Some(tab) = tab => {
                        self.tabs.close(tab);
                        self.show_active(root);
                        return Outcome { title: true, ..Outcome::default() };
                    }
                    MenuChoice::Item(MENU_RENAME) if tab.is_some() => self.start_tab_rename(root, &clicked),
                    MenuChoice::Item(MENU_RENAME) => self.tree.start_rename(root, clicked),
                    MenuChoice::Item(_) | MenuChoice::Dismissed | MenuChoice::Hover(_) => {}
                }
            }
            Some(Popup::CreateMenu { layer, folder }) => {
                let Some(choice) = action.downcast_ref::<MenuChoice>() else {
                    self.popup = Some(Popup::CreateMenu { layer, folder });
                    return Outcome::default();
                };
                match choice {
                    MenuChoice::Hover(_) => self.popup = Some(Popup::CreateMenu { layer, folder }),
                    MenuChoice::Item(item) => {
                        root.remove_layer(layer);
                        if let Some(create) = Create::ALL.get(*item) {
                            let title = self.create_in(root, *create, &folder);
                            return Outcome { title, ..Outcome::default() };
                        }
                    }
                    MenuChoice::Sub(_) | MenuChoice::Dismissed => root.remove_layer(layer),
                }
            }
            Some(Popup::Confirm(dialog, then)) => self.on_confirm(root, dialog, then, id, action),
            Some(Popup::Menu { layer, commands }) => return self.on_menu(root, layer, commands, action),
            Some(Popup::CanvasMenu { layer, commands }) => return self.on_canvas_menu(root, layer, commands, action),
            Some(Popup::SortMenu { layer, button }) => match self.dropdown_answer(root, layer, button, id, action) {
                None => self.popup = Some(Popup::SortMenu { layer, button }),
                Some(chosen) => {
                    if let Some(sort) = chosen.and_then(|index| crate::ui::file_tree::Sort::ALL.get(index)) {
                        self.tree.set_sort(root, *sort);
                    }
                }
            },
            Some(Popup::TabsMenu { layer, button }) => match self.dropdown_answer(root, layer, button, id, action) {
                None => self.popup = Some(Popup::TabsMenu { layer, button }),
                Some(chosen) => return chosen.map_or_else(Outcome::default, |index| self.pick_tab(root, index)),
            },
            Some(Popup::Viewer { layer }) => {
                if action.downcast_ref::<crate::ui::viewer::ViewerClosed>().is_some() {
                    root.remove_layer(layer);
                } else {
                    self.popup = Some(Popup::Viewer { layer });
                }
            }
            Some(popup @ (Popup::Settings { .. } | Popup::Search(_) | Popup::Templates(_) | Popup::Palette(_) | Popup::Workspaces(_) | Popup::Reader(_))) => self.popup = Some(popup),
            None => {}
        }
        Outcome::default()
    }

    fn on_menu(&mut self, root: &mut RenderRoot, layer: WidgetId, commands: Vec<MenuCommand>, action: &ErasedAction) -> Outcome {
        let Some(choice) = action.downcast_ref::<MenuChoice>() else {
            self.popup = Some(Popup::Menu { layer, commands });
            return Outcome::default();
        };
        let MenuChoice::Item(item) = choice else { return Outcome::default() };
        root.remove_layer(layer);
        match commands.get(*item) {
            Some(&MenuCommand::Version(index, command)) => self.version_command(root, index, command),
            Some(&MenuCommand::Table(command)) => self.table_command(root, command),
            Some(&MenuCommand::Note(command)) => return self.note_command(root, command),
            Some(&MenuCommand::Cover(action)) => self.cover_action(root, action),
            Some(&MenuCommand::Book(command)) => self.book_command(root, command),
            None => {}
        }
        Outcome::default()
    }
}

fn mark_trigger(root: &mut RenderRoot, button: WidgetId, open: bool) {
    root.edit_widget(button, |mut w| {
        if let Some(mut icon) = w.try_downcast::<crate::ui::widgets::IconButton>() {
            crate::ui::widgets::IconButton::set_pressed(&mut icon, open);
        } else if let Some(mut field) = w.try_downcast::<DropdownButton>() {
            DropdownButton::set_open(&mut field, open);
        }
    });
}

#[derive(Clone, Copy)]
enum Create {
    Note,
    Canvas,
    Folder,
}

impl Create {
    const ALL: [Create; 3] = [Create::Note, Create::Canvas, Create::Folder];
}

fn create_labels() -> Vec<(String, crate::ui::icons::Icon)> {
    vec![
        (t("fileTree.newNote"), crate::ui::icons::FILE_TEXT),
        (t("fileTree.newCanvas"), crate::ui::icons::CANVAS),
        (t("fileTree.newFolder"), crate::ui::icons::FOLDER_PLUS),
    ]
}

/// Which creation command a row-menu item is: they follow rename, duplicate and delete (and the
/// colour entry, which only folders have).
fn create_choice(item: usize, folder: bool) -> Option<Create> {
    let first = if folder { MENU_COLOR + 1 } else { MENU_COLOR };
    Create::ALL.get(item.checked_sub(first)?).copied()
}
