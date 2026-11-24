//! Shortcuts the user has changed. They replace the default chords of one binding and live in
//! `shortcuts.json` inside the data directory, as `{ "newNote": ["action n"] }`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use masonry::core::keyboard::{Key, Modifiers, NamedKey};

use super::chord::{Chord, MAC, Name, Os, Shift, action};
use super::table::{BINDINGS, Binding};
use super::Scope;

const FILE: &str = "shortcuts.json";

/// Keys that can be part of a shortcut besides letters, digits and symbols.
const NAMED: &[(&str, NamedKey)] = &[
    ("Enter", NamedKey::Enter),
    ("Tab", NamedKey::Tab),
    ("ArrowUp", NamedKey::ArrowUp),
    ("ArrowDown", NamedKey::ArrowDown),
    ("ArrowLeft", NamedKey::ArrowLeft),
    ("ArrowRight", NamedKey::ArrowRight),
    ("PageUp", NamedKey::PageUp),
    ("PageDown", NamedKey::PageDown),
    ("Home", NamedKey::Home),
    ("End", NamedKey::End),
    ("Delete", NamedKey::Delete),
    ("Insert", NamedKey::Insert),
    ("Backspace", NamedKey::Backspace),
    ("F1", NamedKey::F1),
    ("F2", NamedKey::F2),
    ("F3", NamedKey::F3),
    ("F4", NamedKey::F4),
    ("F5", NamedKey::F5),
    ("F6", NamedKey::F6),
    ("F7", NamedKey::F7),
    ("F8", NamedKey::F8),
    ("F9", NamedKey::F9),
    ("F10", NamedKey::F10),
    ("F11", NamedKey::F11),
    ("F12", NamedKey::F12),
];

#[derive(Default)]
struct State {
    file: Option<PathBuf>,
    chosen: Vec<(&'static str, Vec<Chord>)>,
    capturing: Option<&'static str>,
    conflict: Option<&'static str>,
}

// The interface runs on one thread; a per-thread state also keeps tests apart from each other.
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|state| f(&mut state.borrow_mut()))
}

/// Only these bindings can be changed: app-level and formatting commands.
pub fn can_rebind(binding: &Binding) -> bool {
    matches!(binding.scope, Scope::Global | Scope::Editor) && !binding.chords.is_empty()
}

pub fn rebindable(label: &str) -> bool {
    BINDINGS.iter().any(|binding| binding.label == label && can_rebind(binding))
}

pub fn customized(label: &str) -> bool {
    with(|state| state.chosen.iter().any(|(chosen, _)| *chosen == label))
}

/// The chords that currently trigger a binding: the user's, or the defaults.
pub fn chords(binding: &Binding) -> Vec<Chord> {
    with(|state| state.chosen.iter().find(|(label, _)| *label == binding.label).map(|(_, chords)| chords.clone())).unwrap_or_else(|| binding.chords.to_vec())
}

pub fn matches(binding: &Binding, key: &Key, modifiers: Modifiers) -> bool {
    let chosen = with(|state| state.chosen.iter().find(|(label, _)| *label == binding.label).map(|(_, chords)| chords.clone()));
    match chosen {
        Some(chords) => chords.iter().any(|chord| chord.matches(key, modifiers)),
        None => binding.chords.iter().any(|chord| chord.matches(key, modifiers)),
    }
}

/// Starts listening for the next key combination to assign to `label`.
pub fn start_capture(label: &str) {
    let label = BINDINGS.iter().find(|binding| binding.label == label && can_rebind(binding)).map(|binding| binding.label);
    with(|state| {
        state.capturing = label;
        state.conflict = None;
    });
}

pub fn stop_capture() {
    with(|state| {
        state.capturing = None;
        state.conflict = None;
    });
}

pub fn capturing() -> Option<&'static str> {
    with(|state| state.capturing)
}

/// Label of the binding that blocked the last attempt, if any.
pub fn conflict() -> Option<&'static str> {
    with(|state| state.conflict)
}

pub fn set_conflict(label: Option<&'static str>) {
    with(|state| state.conflict = label);
}

/// The chord for a key press, or `None` when it cannot be a shortcut (a modifier alone, or a plain
/// letter without Ctrl/Alt, which would break typing).
pub fn chord_from(key: &Key, modifiers: Modifiers) -> Option<Chord> {
    let name = match key {
        Key::Character(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() || c.is_whitespace() || c.is_control() {
                return None;
            }
            Name::Char(c.to_lowercase().next()?)
        }
        Key::Named(named) if NAMED.iter().any(|(_, known)| known == named) => Name::Named(*named),
        _ => return None,
    };
    let function = matches!(name, Name::Named(named) if NAMED.iter().any(|(label, known)| *known == named && label.starts_with('F')));
    let (action, ctrl) = (action(modifiers), MAC && modifiers.ctrl());
    if !(action || ctrl || modifiers.alt() || function) {
        return None;
    }
    Some(Chord { name, action, ctrl, alt: modifiers.alt(), shift: if modifiers.shift() { Shift::Yes } else { Shift::No }, os: Os::All })
}

fn same_keys(a: &Chord, b: &Chord) -> bool {
    a.name == b.name && a.alt == b.alt && (a.action || a.ctrl) == (b.action || b.ctrl) && (a.shift == b.shift || a.shift == Shift::Any || b.shift == Shift::Any)
}

/// Another binding that would also fire on `chord`, if there is one. Scopes that are active at
/// the same time as the editor count; dialogs and lists do not.
pub fn conflicting(label: &str, chord: &Chord) -> Option<&'static str> {
    BINDINGS
        .iter()
        .filter(|binding| binding.label != label && matches!(binding.scope, Scope::Global | Scope::Text | Scope::Editor | Scope::Navigation))
        .find(|binding| chords(binding).iter().chain(binding.hidden).any(|other| other.here() && same_keys(other, chord)))
        .map(|binding| binding.label)
}

pub fn assign(label: &str, chord: Chord) {
    let Some(label) = BINDINGS.iter().find(|binding| binding.label == label && can_rebind(binding)).map(|binding| binding.label) else { return };
    with(|state| {
        state.chosen.retain(|(chosen, _)| *chosen != label);
        state.chosen.push((label, vec![chord]));
        state.capturing = None;
        state.conflict = None;
    });
    save();
}

pub fn reset(label: &str) {
    with(|state| state.chosen.retain(|(chosen, _)| *chosen != label));
    save();
}

pub fn encode(chord: &Chord) -> String {
    let mut parts: Vec<String> = Vec::new();
    if chord.action {
        parts.push("action".into());
    }
    if chord.ctrl {
        parts.push("ctrl".into());
    }
    if chord.alt {
        parts.push("alt".into());
    }
    if chord.shift == Shift::Yes {
        parts.push("shift".into());
    }
    parts.push(match chord.name {
        Name::Char(c) => c.to_string(),
        Name::Named(named) => NAMED.iter().find(|(_, known)| *known == named).map_or("?", |(label, _)| label).to_owned(),
    });
    parts.join(" ")
}

pub fn decode(text: &str) -> Option<Chord> {
    let mut chord = Chord { name: Name::Char(' '), action: false, ctrl: false, alt: false, shift: Shift::No, os: Os::All };
    let mut key = None;
    for part in text.split(' ') {
        match part {
            "action" => chord.action = true,
            "ctrl" => chord.ctrl = true,
            "alt" => chord.alt = true,
            "shift" => chord.shift = Shift::Yes,
            other => {
                if key.is_some() {
                    return None;
                }
                key = Some(match NAMED.iter().find(|(label, _)| *label == other) {
                    Some((_, named)) => Name::Named(*named),
                    None => {
                        let mut chars = other.chars();
                        let c = chars.next()?;
                        if chars.next().is_some() {
                            return None;
                        }
                        Name::Char(c)
                    }
                });
            }
        }
    }
    chord.name = key?;
    Some(chord)
}

/// Reads the saved shortcuts and remembers where to save the next change.
pub fn load(data_dir: &Path) {
    let file = data_dir.join(FILE);
    let saved: BTreeMap<String, Vec<String>> = std::fs::read_to_string(&file).ok().and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
    let mut chosen = Vec::new();
    for (label, chords) in saved {
        let Some(binding) = BINDINGS.iter().find(|binding| binding.label == label && can_rebind(binding)) else { continue };
        let chords: Vec<Chord> = chords.iter().filter_map(|text| decode(text)).collect();
        if !chords.is_empty() {
            chosen.push((binding.label, chords));
        }
    }
    with(|state| *state = State { file: Some(file), chosen, ..State::default() });
}

fn save() {
    let (file, saved) = with(|state| {
        let saved: BTreeMap<&str, Vec<String>> = state.chosen.iter().map(|(label, chords)| (*label, chords.iter().map(encode).collect())).collect();
        (state.file.clone(), saved)
    });
    let Some(file) = file else { return };
    match serde_json::to_string_pretty(&saved) {
        Ok(json) => {
            if let Err(error) = std::fs::write(&file, json) {
                report!("shortcuts not saved: {error}");
            }
        }
        Err(error) => report!("shortcuts not saved: {error}"),
    }
}

#[cfg(test)]
pub fn forget() {
    with(|state| *state = State::default());
}
