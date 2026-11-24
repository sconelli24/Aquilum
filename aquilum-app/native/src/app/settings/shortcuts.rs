//! Listening for the next key combination while a shortcut is being changed in Settings.

use masonry::app::RenderRoot;
use masonry::core::keyboard::{Key, Modifiers, NamedKey};

use super::{App, Outcome, Popup};
use crate::ui::keymap;

impl App {
    /// While a shortcut row is waiting for keys, every key press goes here instead of running
    /// commands. `None` means no shortcut is being changed.
    pub fn capture_key(&mut self, root: &mut RenderRoot, key: &Key, modifiers: Modifiers) -> Option<Outcome> {
        let label = keymap::capturing()?;
        if !matches!(self.popup, Some(Popup::Settings { .. })) {
            keymap::stop_capture();
            return None;
        }
        if matches!(key, Key::Named(NamedKey::Escape)) && modifiers.is_empty() {
            keymap::stop_capture();
            self.refresh_settings(root);
            return Some(Outcome::default());
        }
        // Modifier keys alone, or combinations that cannot be shortcuts, keep listening.
        if let Some(chord) = keymap::chord_from(key, modifiers) {
            match keymap::conflicting(label, &chord) {
                Some(other) => keymap::set_conflict(Some(other)),
                None => keymap::assign(label, chord),
            }
            self.refresh_settings(root);
        }
        Some(Outcome::default())
    }
}
