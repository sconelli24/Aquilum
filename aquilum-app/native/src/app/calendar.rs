//! The calendar in the sidebar: keeps it in step with the settings and the daily notes, and opens
//! the note of a clicked day.

use chrono::Datelike;
use masonry::app::RenderRoot;
use masonry::core::{ErasedAction, WidgetId};

use super::confirm::Confirmed;
use super::{App, Outcome};
use crate::calendar;
use crate::file_ops;
use crate::i18n::{t, t_with};
use crate::ui::calendar::{CalendarAction, CalendarPanel, CalendarState};

impl App {
    /// Whether the calendar shows, with what, for a month: from the settings and the daily folder.
    pub(in crate::app) fn calendar_state(&self, year: i32, month: u32) -> CalendarState {
        let config = self.workspace.core.settings.get_config().calendar;
        let vault = self.tree.root();
        let shown = config.show && vault.is_some();
        let marked = match vault {
            Some(vault) if shown => calendar::days_with_notes(&calendar::daily_folder(vault, &config.folder), year, month),
            _ => Default::default(),
        };
        CalendarState { shown, progress: config.progress, weeks: config.progress_weeks, marked }
    }

    /// The calendar of a newly built interface, on the current month.
    pub(in crate::app) fn initial_calendar(&self) -> CalendarState {
        let today = calendar::today();
        self.calendar_state(today.year(), today.month())
    }

    /// Follows the settings and the files after a change.
    pub(in crate::app) fn sync_calendar(&mut self, root: &mut RenderRoot) {
        let Some(handle) = self.chrome.as_ref().map(|chrome| chrome.calendar) else { return };
        let Some((year, month)) = handle.get(root).map(|panel| panel.inner().month()) else { return };
        let state = self.calendar_state(year, month);
        handle.edit(root, |mut panel| CalendarPanel::set_state(&mut panel, state, calendar::today()));
    }

    pub(in crate::app) fn on_calendar_action(&mut self, root: &mut RenderRoot, id: WidgetId, action: &ErasedAction) -> Option<Outcome> {
        if self.chrome.as_ref().map(|chrome| chrome.calendar.id()) != Some(id) {
            return None;
        }
        match action.downcast_ref::<CalendarAction>()? {
            CalendarAction::Month { .. } => {
                self.sync_calendar(root);
                Some(Outcome::default())
            }
            CalendarAction::Day(date) => {
                let title = self.open_daily(root, *date);
                Some(Outcome { title, ..Outcome::default() })
            }
        }
    }

    /// Opens the daily note of `date`. A day without a note asks first (unless the settings say
    /// not to), as the Calendar plugin of Obsidian does, and only then creates it.
    fn open_daily(&mut self, root: &mut RenderRoot, date: chrono::NaiveDate) -> bool {
        let Some(vault) = self.tree.root().map(std::path::Path::to_path_buf) else { return false };
        let config = self.workspace.core.settings.get_config().calendar;
        let path = calendar::daily_path(&vault, &config.folder, date);
        if path.exists() {
            return self.open(root, path);
        }
        if config.confirm {
            let name = calendar::daily_name(date);
            self.ask(
                root,
                Confirmed::CreateDaily(date),
                &t("calendar.newTitle"),
                &t_with("calendar.newDescription", &[("name", &name)]),
                &t("common.cancel"),
                &t("calendar.create"),
            );
            return false;
        }
        self.create_daily(root, date)
    }

    /// Creates the daily note of `date` in the daily folder, from the template if there is one,
    /// and opens it. A note that appeared meanwhile is opened as it is.
    pub(in crate::app) fn create_daily(&mut self, root: &mut RenderRoot, date: chrono::NaiveDate) -> bool {
        let Some(vault) = self.tree.root().map(std::path::Path::to_path_buf) else { return false };
        let config = self.workspace.core.settings.get_config().calendar;
        let mut path = calendar::daily_path(&vault, &config.folder, date);
        if !path.exists() {
            let folder = calendar::daily_folder(&vault, &config.folder);
            // The template, if one is set and exists, with `{{date}}` meaning this day.
            let content = self
                .templates_dir()
                .and_then(|dir| calendar::template_file(&dir, &config.template))
                .map(|template| super::templates::template_text_on(&template, date))
                .unwrap_or_default();
            match file_ops::create_note_with(&self.workspace.core, &folder, Some(&calendar::daily_name(date)), &content) {
                Ok(created) => {
                    self.tree.paths_changed(root, std::slice::from_ref(&created));
                    path = created;
                }
                Err(error) => {
                    report!("daily note not created: {error}");
                    return false;
                }
            }
            self.sync_calendar(root);
        }
        self.open(root, path)
    }

    /// The layer of the open "create the daily note?" dialog.
    #[cfg(test)]
    pub(in crate::app) fn daily_dialog_layer(&self) -> Option<WidgetId> {
        match &self.popup {
            Some(super::Popup::Confirm(dialog, Confirmed::CreateDaily(_))) => Some(dialog.layer),
            _ => None,
        }
    }
}
