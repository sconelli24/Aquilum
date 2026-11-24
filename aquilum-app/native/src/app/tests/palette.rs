use super::*;
use crate::ui::keymap::Command;

fn open(h: &mut Harness) {
    let outcome = h.app.command(&mut h.root, Command::Palette);
    assert!(outcome.is_some(), "the palette command was not handled");
    h.settle();
    assert!(matches!(h.app.popup, Some(Popup::Palette(_))), "palette did not open");
}

fn visible(h: &Harness) -> usize {
    let Some(Popup::Palette(state)) = &h.app.popup else { panic!("palette not open") };
    state.filtered.len()
}

#[test]
fn ctrl_p_is_bound_to_the_palette() {
    use masonry::core::keyboard::{Key, Modifiers};
    let command = crate::ui::keymap::command(&[crate::ui::keymap::Scope::Global], &Key::Character("p".into()), Modifiers::CONTROL);
    assert_eq!(command, Some(Command::Palette));
}

#[test]
fn the_palette_lists_every_command_and_filters_as_you_type() {
    let mut h = Harness::new();
    open(&mut h);
    let all = visible(&h);
    assert!(all >= 15, "only {all} commands listed");
    h.type_text(&crate::i18n::t("fileTree.newCanvas"));
    assert_eq!(visible(&h), 1, "filter should leave only the canvas command");
    h.type_text("zzzz");
    assert_eq!(visible(&h), 0);
}

#[test]
fn enter_closes_the_palette_and_runs_the_command() {
    let mut h = Harness::new();
    open(&mut h);
    h.type_text(&crate::i18n::t("fileTree.newCanvas"));
    let outcome = h.app.palette_key(&mut h.root, &Key::Named(NamedKey::Enter), Modifiers::empty());
    assert!(outcome.is_some() && h.app.popup.is_none(), "Enter did not choose the command");
    h.deliver();
    let canvases = std::fs::read_dir(h.vault.path()).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "canvas")).count();
    assert!(canvases >= 1 || crate::vault_files::notes_dir(&h.app.workspace.core, h.vault.path()).read_dir().is_ok_and(|d| d.flatten().any(|e| e.path().extension().is_some_and(|x| x == "canvas"))), "no canvas was created");
}

#[test]
fn a_shortcut_command_chosen_from_the_palette_runs() {
    let mut h = Harness::new();
    let before = h.app.tabs.tabs().len();
    open(&mut h);
    h.type_text(&crate::i18n::t("settings.shortcuts.newTab"));
    let outcome = h.app.palette_key(&mut h.root, &Key::Named(NamedKey::Enter), Modifiers::empty());
    assert!(outcome.is_some() && h.app.popup.is_none());
    h.deliver();
    assert_eq!(h.app.tabs.tabs().len(), before + 1, "New tab did not open a tab");
}

#[test]
fn escape_closes_the_palette() {
    let mut h = Harness::new();
    open(&mut h);
    let _ = h.app.back(&mut h.root);
    h.deliver();
    assert!(h.app.popup.is_none());
}

mod rebinding {
    use super::*;
    use crate::ui::keymap;
    use crate::ui::settings::schema::Section;
    use masonry::core::keyboard::{Key, Modifiers, NamedKey};

    fn mods() -> Modifiers {
        if cfg!(target_os = "macos") { Modifiers::META | Modifiers::SHIFT } else { Modifiers::CONTROL | Modifiers::SHIFT }
    }

    #[test]
    fn the_next_key_combination_becomes_the_shortcut_and_escape_cancels() {
        let mut h = Harness::new();
        h.app.show_settings(&mut h.root, Section::Shortcuts);
        h.settle();
        keymap::start_capture("newNote");
        assert_eq!(keymap::capturing(), Some("newNote"));
        // Escape stops listening and changes nothing
        let outcome = h.app.capture_key(&mut h.root, &Key::Named(NamedKey::Escape), Modifiers::empty());
        assert!(outcome.is_some() && keymap::capturing().is_none() && !keymap::customized("newNote"));
        // A modifier on its own keeps listening
        keymap::start_capture("newNote");
        let _ = h.app.capture_key(&mut h.root, &Key::Named(NamedKey::Shift), Modifiers::SHIFT);
        assert_eq!(keymap::capturing(), Some("newNote"));
        // A combination already in use is refused and listening continues
        let ctrl_t = if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL };
        let _ = h.app.capture_key(&mut h.root, &Key::Character("t".into()), ctrl_t);
        assert_eq!(keymap::conflict(), Some("newTab"));
        assert_eq!(keymap::capturing(), Some("newNote"));
        // A free one is accepted
        let _ = h.app.capture_key(&mut h.root, &Key::Character("j".into()), mods());
        assert!(keymap::capturing().is_none() && keymap::customized("newNote"));
        keymap::reset("newNote");
    }

    #[test]
    fn nothing_is_captured_when_settings_is_closed() {
        let mut h = Harness::new();
        keymap::start_capture("newNote");
        assert!(h.app.capture_key(&mut h.root, &Key::Character("j".into()), mods()).is_none());
        assert!(keymap::capturing().is_none(), "capture stayed on after settings closed");
    }
}
