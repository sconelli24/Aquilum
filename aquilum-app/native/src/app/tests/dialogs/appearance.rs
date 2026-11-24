use super::*;
use crate::ui::editor::TextAction;
use crate::ui::settings::Binding;

fn color_field(h: &Harness) -> masonry::core::WidgetId {
    h.binding("primary color field", |b| matches!(b, Binding::Text { value, .. } if value.starts_with('#')))
}

/// The accent follows the theme by default: tests of the hand-picked color switch that off first.
fn manual_accent(h: &mut Harness) {
    let mut config = h.config();
    config.ui.accent_auto = false;
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
}

#[test]
fn primary_color_field_updates_settings() {
    let mut h = Harness::new();
    manual_accent(&mut h);
    h.click(242.0, 578.0);
    assert_eq!(h.section(), Some(Section::Ui));
    let field = color_field(&h);
    h.send(field, TextAction::Changed("#E05A".into()));
    assert_eq!(h.config().ui.primary_color, AppConfig::default().ui.primary_color);
    h.send(field, TextAction::Changed("#E05A2B".into()));
    assert_eq!(h.config().ui.primary_color, "#e05a2b");
    assert_eq!(h.section(), Some(Section::Ui));
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0xe0, 0x5a, 0x2b, 0xff]);
}

#[test]
fn system_appearance_updates_live_without_rebuilding_the_editor() {
    let mut h = Harness::new();
    let note = h.write_note("Тема.md", "Текст заметки");
    h.open(note);
    let editor = h.app.note_ids.as_ref().unwrap().body.id();
    let mut config = h.config();
    config.ui.theme = "system".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert!(!theme::is_dark());
    h.app.system_theme_changed(&mut h.root, true);
    assert!(theme::is_dark());
    let generation = theme::generation();
    h.app.system_theme_changed(&mut h.root, true);
    assert_eq!(theme::generation(), generation, "an unchanged desktop theme restyled the app");
    h.app.system_theme_changed(&mut h.root, false);
    assert!(!theme::is_dark());
    assert_eq!(h.config().ui.theme, "system", "desktop appearance overwrote the user's setting");
    assert_eq!(h.app.note_ids.as_ref().unwrap().body.id(), editor);
}

#[test]
fn explicit_themes_ignore_desktop_changes_but_remember_the_latest_preference() {
    let mut h = Harness::new();
    for setting in ["light", "dark"] {
        let mut config = h.config();
        config.ui.theme = setting.into();
        h.app.workspace.core.settings.update_config(config).unwrap();
        h.app.restyle(&mut h.root, false, None);
        let generation = theme::generation();
        h.app.system_theme_changed(&mut h.root, true);
        h.app.system_theme_changed(&mut h.root, false);
        assert_eq!(theme::is_dark(), setting != "light");
        assert_eq!(theme::generation(), generation);
        assert!(!h.app.system_dark);
    }
    let mut config = h.config();
    config.ui.theme = "system".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert!(!theme::is_dark());
}

#[test]
fn primary_color_typed_into_field() {
    let mut h = Harness::new();
    manual_accent(&mut h);
    h.click(242.0, 578.0);
    let field = color_field(&h);
    h.root.focus_on(Some(field));
    h.key(Key::Character("a".into()), crate::test_dir::command());
    h.type_text("#e05a2b");
    assert_eq!(h.config().ui.primary_color, "#e05a2b");
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0xe0, 0x5a, 0x2b, 0xff]);
}

#[test]
fn the_theme_mode_has_three_choices_and_lives_with_the_themes() {
    use crate::ui::settings::controls::Segmented;
    let mut h = Harness::new();
    h.click(242.0, 578.0);
    h.open_section(Section::Themes);
    let themes = h.binding("theme switch", |b| matches!(b, Binding::Choice(_)));
    let widget = h.root.get_widget(themes).unwrap();
    let segmented = widget.downcast::<Segmented>().unwrap().inner();
    assert!(segmented.segment(2).is_some(), "system, light and dark");
    assert!(segmented.segment(3).is_none(), "there is no night mode any more");
    let dark = segmented.segment(2).unwrap().center() + widget.ctx().bounding_box().origin().to_vec2();
    h.click(dark.x, dark.y);
    assert_eq!(h.config().ui.theme, "dark");
}

#[test]
fn restyle_matches_fresh_build() {
    let mut h = Harness::new();
    let note = h.write_note("заметка.md", "Текст заметки с [[ссылкой]] и **жирным**.\n\n- пункт\n- [ ] задача");
    h.open(note);
    h.app.open_settings(&mut h.root);
    let templates = h.nav(Section::Data);
    h.act(templates, crate::ui::widgets::Pressed);
    h.settle();
    let mut config = h.config();
    config.ui.theme = "light".into();
    config.ui.primary_color = "#e0457b".into();
    config.ui.font.font_size_base = 16;
    config.ui.font.font_weight = 500;
    config.editor.font.font_size_base = 19;
    config.editor.line_height = 1.8;
    config.editor.max_width_ch = 60;
    h.app.workspace.core.settings.update_config(config).unwrap();
    let ids_before: Vec<_> = h.app.note_ids.as_ref().map(|ids| ids.body.id()).into_iter().collect();
    h.app.restyle(&mut h.root, true, None);
    h.settle();
    let live = h.frame(W as u16, H as u16, 1.0);
    assert_eq!(h.app.note_ids.as_ref().map(|ids| ids.body.id()).into_iter().collect::<Vec<_>>(), ids_before, "restyle recreated the editor");
    h.app.rebuild(&mut h.root);
    h.settle();
    let fresh = h.frame(W as u16, H as u16, 1.0);
    dump("restyle-live.png", &live);
    dump("restyle-fresh.png", &fresh);
    let differ = differing_pixels(live.frame_pixels(), fresh.frame_pixels());
    assert_eq!(differ, 0, "after restyle the frame differs from one built from scratch in {differ} pixels");
}

#[test]
fn color_picker_previews_live_and_saves_on_close() {
    let mut h = Harness::new();
    manual_accent(&mut h);
    h.app.open_settings(&mut h.root);
    h.settle();
    let swatch = h.binding("color swatch", |b| matches!(b, Binding::Color(_)));
    let rect = h.root.get_widget(swatch).unwrap().ctx().bounding_box();
    h.click(rect.center().x, rect.center().y);
    assert!(h.menu_open(), "color picker did not open");
    let before = h.config().ui.primary_color;
    let accent = theme::current().bg_accent;
    let picker = h.root.focused_widget().expect("focus in the color picker");
    let area = h.root.get_widget(picker).unwrap().ctx().bounding_box();
    let mut s = pointer_at(area.x0 + 30.0, area.y0 + 70.0, 1.0);
    s.buttons.insert(PointerButton::Primary);
    h.root.handle_pointer_event(PointerEvent::Down(primary(s.clone())));
    s.position = PhysicalPosition::new(area.x0 + 150.0, area.y0 + 100.0);
    h.root.handle_pointer_event(moved(MOUSE, s.clone()));
    s.buttons.remove(PointerButton::Primary);
    h.root.handle_pointer_event(PointerEvent::Up(primary(s)));
    h.deliver();
    assert_ne!(theme::current().bg_accent, accent, "color was not applied live");
    assert_eq!(h.config().ui.primary_color, before, "settings written before closing");
    h.snapshot("picker.png");
    h.escape();
    assert!(!h.menu_open(), "color picker did not close");
    assert_ne!(h.config().ui.primary_color, before, "color was not saved on close");
}

#[test]
fn a_preset_swatch_sets_the_accent_at_once_and_is_saved_on_close() {
    let mut h = Harness::new();
    manual_accent(&mut h);
    h.app.open_settings(&mut h.root);
    h.settle();
    let swatch = h.binding("color swatch", |b| matches!(b, Binding::Color(_)));
    let rect = h.root.get_widget(swatch).unwrap().ctx().bounding_box();
    h.click(rect.center().x, rect.center().y);
    assert!(h.menu_open(), "color picker did not open");
    let picker = h.root.focused_widget().expect("focus in the color picker");
    let area = h.root.get_widget(picker).unwrap().ctx().bounding_box();
    h.click(area.x0 + 12.0 + 22.0 + 15.2 + 11.0, area.y0 + 12.0 + 11.0);
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0x93, 0x33, 0xea, 0xff], "second preset color was not applied");
    h.escape();
    assert_eq!(h.config().ui.primary_color, "#9333ea");
}

#[test]
fn themes_section_edits_the_shown_theme_and_resets_it() {
    use crate::ui::settings::schema::{Action, Setter};
    let mut h = Harness::new();
    h.click(242.0, 578.0);
    h.open_section(Section::Themes);
    assert_eq!(h.section(), Some(Section::Themes));
    let field = h.binding("note background field", |b| matches!(b, Binding::Text { setter: Setter::ThemeValue("editor-bg"), .. }));
    h.send(field, TextAction::Changed("#112233".into()));
    let id = theme::active().id;
    assert_eq!(h.config().ui.theme_overrides[id]["editor-bg"], "#112233");
    assert_eq!(theme::current().editor_bg.to_rgba8().to_u8_array(), [0x11, 0x22, 0x33, 0xff]);
    assert_eq!(h.section(), Some(Section::Themes));
    let reset = h.binding("theme reset button", |b| matches!(b, Binding::Button(Action::ResetTheme)));
    h.send(reset, crate::ui::widgets::Pressed);
    assert!(h.config().ui.theme_overrides.get(id).is_none_or(|edits| edits.is_empty()));
    assert_ne!(theme::current().editor_bg.to_rgba8().to_u8_array(), [0x11, 0x22, 0x33, 0xff]);
}

#[test]
fn a_chosen_dark_theme_applies_in_dark_mode() {
    let mut h = Harness::new();
    let mut config = h.config();
    config.ui.theme = "dark".into();
    config.ui.dark_theme = "catppuccin-mocha".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert_eq!(theme::active().id, "catppuccin-mocha");
    assert_eq!(theme::current().editor_bg.to_rgba8().to_u8_array(), [0x1e, 0x1e, 0x2e, 0xff]);
    let mut config = h.config();
    config.ui.theme = "light".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert_eq!(theme::active().id, "light");
}

#[test]
fn the_accent_follows_the_chosen_theme_until_it_is_picked_by_hand() {
    let mut h = Harness::new();
    assert!(AppConfig::default().ui.accent_auto);
    let mut config = h.config();
    config.ui.theme = "dark".into();
    config.ui.dark_theme = "catppuccin-mocha".into();
    config.ui.primary_color = "#e05a2b".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0x89, 0xb4, 0xfa, 0xff], "with the switch on the stored color is ignored");
    let mut config = h.config();
    config.ui.dark_theme = "catppuccin-macchiato".into();
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0x8a, 0xad, 0xf4, 0xff], "another theme brings its own accent");
    let mut config = h.config();
    config.ui.accent_auto = false;
    h.app.workspace.core.settings.update_config(config).unwrap();
    h.app.restyle(&mut h.root, false, None);
    assert_eq!(theme::current().bg_accent.to_rgba8().to_u8_array(), [0xe0, 0x5a, 0x2b, 0xff], "manual mode uses the stored color");
}

