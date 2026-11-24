use masonry::core::keyboard::{Code, Key, Modifiers};

use super::chord::{Chord, Name, Shift};
use super::table::BINDINGS;
use super::{Command, Scope, command};

const WIDGETS: &[&[Scope]] = &[
    &[Scope::Text, Scope::Editor, Scope::Navigation, Scope::Caret],
    &[Scope::Text, Scope::Navigation, Scope::Table, Scope::Caret],
    &[Scope::Image],
    &[Scope::Text, Scope::Image],
    &[Scope::List, Scope::Navigation],
    &[Scope::List],
    &[Scope::Find],
    &[Scope::Text, Scope::Reader],
    &[Scope::Dialog],
    &[Scope::Slider],
    &[Scope::Choice],
    &[Scope::Number],
];

fn press(chord: &Chord) -> (Key, Modifiers) {
    let key = match chord.name {
        Name::Char(c) => Key::Character(c.to_string()),
        Name::Named(named) => Key::Named(named),
    };
    let mut modifiers = Modifiers::empty();
    if chord.action {
        modifiers |= crate::test_dir::command();
    }
    if chord.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if chord.alt {
        modifiers |= Modifiers::ALT;
    }
    if chord.shift == Shift::Yes {
        modifiers |= Modifiers::SHIFT;
    }
    (key, modifiers)
}

#[test]
fn every_chord_runs_its_own_command_and_no_global_one_hides_a_field_key() {
    for scopes in [&[Scope::Global][..]].iter().chain(WIDGETS) {
        for binding in BINDINGS.iter().filter(|b| scopes.contains(&b.scope)) {
            for chord in binding.chords.iter().chain(binding.hidden).filter(|c| c.here()) {
                let (key, modifiers) = press(chord);
                assert_eq!(command(scopes, &key, modifiers), Some(binding.command), "{} in {scopes:?} is intercepted", chord.label());
                if !scopes.contains(&Scope::Global) {
                    assert_eq!(command(&[Scope::Global], &key, modifiers), None, "a global chord hides {}", chord.label());
                }
            }
        }
    }
}

#[test]
fn latin_layout_shortcuts_use_the_letter_in_every_scope() {
    for scopes in [&[Scope::Global][..]].iter().chain(WIDGETS) {
        for binding in BINDINGS.iter().filter(|b| scopes.contains(&b.scope)) {
            for chord in binding.chords.iter().chain(binding.hidden).filter(|c| c.here() && matches!(c.name, Name::Char(c) if c.is_ascii_alphabetic())) {
                let (key, modifiers) = press(chord);
                let code = if chord.name == Name::Char('c') { Code::KeyI } else { Code::KeyC };
                let actual = super::shortcut_key(key.clone(), code, modifiers);
                assert_eq!(actual, key, "{} in {scopes:?} changed with the physical key", chord.label());
                assert_eq!(command(scopes, &actual, modifiers), Some(binding.command));
            }
        }
    }
}

#[test]
fn non_latin_shortcuts_keep_working_without_affecting_typing_or_altgr() {
    use masonry::core::keyboard::NamedKey;
    let action = crate::test_dir::command();
    let key = Key::Character("с".into());
    let resolved = super::shortcut_key(key.clone(), Code::KeyC, action);
    assert_eq!(command(&[Scope::Text], &resolved, action), Some(Command::Copy));
    assert_eq!(super::shortcut_key(key.clone(), Code::KeyC, Modifiers::empty()), key);
    assert_eq!(super::shortcut_key(key.clone(), Code::KeyC, action | Modifiers::ALT), key);
    for key in [Key::Character(";".into()), Key::Character("C".into()), Key::Named(NamedKey::Tab), Key::Named(NamedKey::Unidentified)] {
        assert_eq!(super::shortcut_key(key.clone(), Code::KeyC, action), key);
    }
}

#[test]
fn every_shortcut_is_listed_in_settings_with_every_translation() {
    let listed: Vec<&str> = super::groups().into_iter().flat_map(|(title, rows)| std::iter::once(title).chain(rows.into_iter().map(|(label, _)| label))).collect();
    for locale in [include_str!("../../../assets/locales/ru.json"), include_str!("../../../assets/locales/en.json"), include_str!("../../../assets/locales/es.json")] {
        let json: serde_json::Value = serde_json::from_str(locale).unwrap();
        for key in &listed {
            assert!(json["settings"]["shortcuts"][key].is_string(), "missing translation settings.shortcuts.{key}");
        }
        for binding in BINDINGS {
            if !binding.chords.is_empty() {
                assert!(json["settings"]["shortcuts"][binding.label].is_string(), "missing translation settings.shortcuts.{}", binding.label);
            }
        }
    }
    assert!(BINDINGS.iter().all(|b| !b.chords.is_empty() || !b.hidden.is_empty() || !b.gestures.is_empty()));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn insert_and_delete_chords_work_like_ctrl_letters() {
    let named = |key| Key::Named(key);
    let letter = |c: &str| Key::Character(c.into());
    use masonry::core::keyboard::NamedKey;
    let text = |key: &Key, modifiers| command(&[Scope::Text], key, modifiers);
    assert_eq!(text(&letter("C"), Modifiers::CONTROL), Some(Command::Copy));
    assert_eq!(text(&named(NamedKey::Insert), Modifiers::CONTROL), Some(Command::Copy));
    assert_eq!(text(&named(NamedKey::Insert), Modifiers::SHIFT), Some(Command::Paste));
    assert_eq!(text(&named(NamedKey::Delete), Modifiers::SHIFT), Some(Command::Cut));
    assert_eq!(text(&named(NamedKey::Delete), Modifiers::empty()), None);
    assert_eq!(text(&letter("c"), Modifiers::CONTROL | Modifiers::ALT), None, "AltGr must not copy");
    assert_eq!(super::label(Scope::Text, Command::Copy), "Ctrl + C / Ctrl + Insert");
    assert_eq!(super::label(Scope::Global, Command::FocusMode), "Ctrl + Shift + F");
}

#[cfg(target_os = "macos")]
#[test]
fn mac_navigation_chords() {
    use masonry::core::keyboard::NamedKey;
    let named = |key| Key::Named(key);
    let editor = |key: &Key, modifiers| command(&[Scope::Text, Scope::Editor, Scope::Navigation, Scope::Caret], key, modifiers);
    let meta = Modifiers::META;
    let alt = Modifiers::ALT;
    // ⌥←/→ moves by words
    assert_eq!(editor(&named(NamedKey::ArrowLeft), alt), Some(Command::WordLeft));
    assert_eq!(editor(&named(NamedKey::ArrowRight), alt), Some(Command::WordRight));
    assert_eq!(editor(&named(NamedKey::ArrowLeft), alt | Modifiers::SHIFT), Some(Command::WordLeft));
    assert_eq!(editor(&named(NamedKey::ArrowRight), alt | Modifiers::SHIFT), Some(Command::WordRight));
    // ⌘←/→ goes to line start/end
    assert_eq!(editor(&named(NamedKey::ArrowLeft), meta), Some(Command::LineStart));
    assert_eq!(editor(&named(NamedKey::ArrowRight), meta), Some(Command::LineEnd));
    assert_eq!(editor(&named(NamedKey::ArrowLeft), meta | Modifiers::SHIFT), Some(Command::LineStart));
    assert_eq!(editor(&named(NamedKey::ArrowRight), meta | Modifiers::SHIFT), Some(Command::LineEnd));
    // ⌘↑/↓ goes to note start/end
    assert_eq!(editor(&named(NamedKey::ArrowUp), meta), Some(Command::DocumentStart));
    assert_eq!(editor(&named(NamedKey::ArrowDown), meta), Some(Command::DocumentEnd));
    assert_eq!(editor(&named(NamedKey::ArrowUp), meta | Modifiers::SHIFT), Some(Command::DocumentStart));
    assert_eq!(editor(&named(NamedKey::Home), meta), Some(Command::DocumentStart));
    assert_eq!(editor(&named(NamedKey::End), meta), Some(Command::DocumentEnd));
    // ⌥⌫ deletes a word, ⌘⌫ deletes to the start of the line
    assert_eq!(editor(&named(NamedKey::Backspace), alt), Some(Command::DeleteWord));
    assert_eq!(editor(&named(NamedKey::Backspace), Modifiers::CONTROL), Some(Command::DeleteWord));
    assert_eq!(editor(&named(NamedKey::Backspace), meta), Some(Command::LineDeleteBackward));
    // ⌥⌦ deletes a word forward
    assert_eq!(editor(&named(NamedKey::Delete), alt), Some(Command::DeleteWordForward));
    assert_eq!(editor(&named(NamedKey::Delete), Modifiers::CONTROL), Some(Command::DeleteWordForward));
    // ⌘ does not replace Ctrl for switching tabs
    assert_eq!(command(&[Scope::Global], &named(NamedKey::Tab), Modifiers::CONTROL), Some(Command::NextTab));
    assert_eq!(command(&[Scope::Global], &named(NamedKey::Tab), meta), None);
    // Shortcut labels on Mac
    assert_eq!(super::label(Scope::Global, Command::FocusMode), "⌘ + Shift + F");
    assert_eq!(super::label(Scope::Navigation, Command::WordLeft), "⌥ + ←");
    assert_eq!(super::label(Scope::Navigation, Command::DocumentStart), "⌘ + ↑");
}

mod custom_shortcuts {
    use masonry::core::keyboard::{Key, Modifiers, NamedKey};

    use super::super::{Command, Scope, command, custom, label};

    fn ctrl_shift_j() -> super::super::chord::Chord {
        let mods = if cfg!(target_os = "macos") { Modifiers::META | Modifiers::SHIFT } else { Modifiers::CONTROL | Modifiers::SHIFT };
        custom::chord_from(&Key::Character("j".into()), mods).expect("chord")
    }

    fn action_mod() -> Modifiers {
        if cfg!(target_os = "macos") { Modifiers::META } else { Modifiers::CONTROL }
    }

    #[test]
    fn a_changed_shortcut_replaces_the_default() {
        custom::forget();
        let global = [Scope::Global];
        assert_eq!(command(&global, &Key::Character("n".into()), action_mod()), Some(Command::NewNote));
        custom::assign("newNote", ctrl_shift_j());
        let mods = action_mod() | Modifiers::SHIFT;
        assert_eq!(command(&global, &Key::Character("j".into()), mods), Some(Command::NewNote));
        assert_eq!(command(&global, &Key::Character("n".into()), action_mod()), None, "the old shortcut still works");
        assert!(label(Scope::Global, Command::NewNote).contains('J'));
        custom::reset("newNote");
        assert_eq!(command(&global, &Key::Character("n".into()), action_mod()), Some(Command::NewNote));
        assert!(!custom::customized("newNote"));
    }

    #[test]
    fn shortcuts_already_in_use_are_refused() {
        custom::forget();
        let new_tab = custom::chord_from(&Key::Character("t".into()), action_mod()).unwrap();
        assert_eq!(custom::conflicting("newNote", &new_tab), Some("newTab"));
        let copy = custom::chord_from(&Key::Character("c".into()), action_mod()).unwrap();
        assert_eq!(custom::conflicting("newNote", &copy), Some("copy"), "editing shortcuts also count");
        assert_eq!(custom::conflicting("newNote", &ctrl_shift_j()), None);
        assert_eq!(custom::conflicting("newTab", &new_tab), None, "a binding does not conflict with itself");
    }

    #[test]
    fn plain_letters_and_lone_modifiers_cannot_be_shortcuts() {
        assert!(custom::chord_from(&Key::Character("j".into()), Modifiers::empty()).is_none());
        assert!(custom::chord_from(&Key::Character("J".into()), Modifiers::SHIFT).is_none());
        assert!(custom::chord_from(&Key::Named(NamedKey::Control), Modifiers::CONTROL).is_none());
        assert!(custom::chord_from(&Key::Named(NamedKey::F5), Modifiers::empty()).is_some(), "function keys work alone");
    }

    #[test]
    fn chords_survive_encoding_and_a_restart() {
        custom::forget();
        let dir = crate::test_dir::tempdir();
        custom::load(dir.path());
        custom::assign("zoomIn", ctrl_shift_j());
        custom::forget();
        assert!(!custom::customized("zoomIn"));
        custom::load(dir.path());
        assert!(custom::customized("zoomIn"), "the change was not saved");
        assert_eq!(command(&[Scope::Global], &Key::Character("j".into()), action_mod() | Modifiers::SHIFT), Some(Command::ZoomIn));
        for text in ["action shift p", "alt ArrowLeft", "F12", "ctrl action +"] {
            let chord = custom::decode(text).unwrap_or_else(|| panic!("cannot decode {text}"));
            assert_eq!(custom::decode(&custom::encode(&chord)), Some(chord));
        }
        assert!(custom::decode("").is_none());
        custom::forget();
    }

    #[test]
    fn only_app_and_formatting_shortcuts_are_changeable() {
        assert!(custom::rebindable("newNote") && custom::rebindable("bold"));
        assert!(!custom::rebindable("copy") && !custom::rebindable("undo") && !custom::rebindable("nextCell"));
        let mut seen = std::collections::HashSet::new();
        for binding in super::super::table::BINDINGS.iter().filter(|b| custom::can_rebind(b)) {
            assert!(seen.insert(binding.label), "two changeable shortcuts share the label {}", binding.label);
        }
    }
}
