use std::sync::Arc;

use masonry::peniko::{Blob, Color};

use super::colors::DEFAULT_PRIMARY;
use super::fonts::bundled;
use super::parse_hex;
use crate::ui::tokens;

#[test]
fn bundled_fonts_answer_to_the_names_styles_ask_for() {
    use masonry::parley::fontique::{Collection, CollectionOptions};
    let mut collection = Collection::new(CollectionOptions { system_fonts: false, ..CollectionOptions::default() });
    let mut names = Vec::new();
    for data in bundled::FONTS {
        for (id, infos) in collection.register_fonts(Blob::new(Arc::new(*data)), None) {
            let italic = infos.iter().any(|info| info.style() != masonry::parley::fontique::FontStyle::Normal);
            names.extend(collection.family_name(id).map(|name| (name.to_owned(), italic)));
        }
    }
    for wanted in ["Inter", "Inter Cyrillic", "iA Writer Mono", "iA Writer Quattro"] {
        for italic in [false, true] {
            assert!(names.iter().any(|(name, i)| name == wanted && *i == italic), "missing {wanted} (italic: {italic}), found {names:?}");
        }
    }
}

#[cfg(target_os = "windows")]
fn has_table(data: &[u8], index: u32, tag: &[u8; 4]) -> bool {
    let read = |at: usize| u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize;
    let base = if &data[..4] == b"ttcf" { read(12 + 4 * index as usize) } else { 0 };
    let tables = u16::from_be_bytes([data[base + 4], data[base + 5]]) as usize;
    (0..tables).any(|i| &data[base + 12 + 16 * i..base + 16 + 16 * i] == tag)
}

#[cfg(target_os = "windows")]
#[test]
fn symbols_and_emoji_find_a_font() {
    use super::{editor_font, ui_font};
    use masonry::parley::{FontContext, LayoutContext, PositionedLayoutItem, StyleProperty};
    let mut fcx = FontContext::default();
    for data in bundled::FONTS {
        fcx.collection.register_fonts(Blob::new(Arc::new(*data)), None);
    }
    let mut lcx = LayoutContext::<masonry::core::BrushIndex>::new();
    for family in [editor_font("iA Writer Mono"), editor_font("iA Writer Quattro"), ui_font("Inter")] {
        for text in ["∈ ∉ ⊂ ∪ ⇒ ⇔ ∑ ∏ ∫", "😀🚀❤️👍"] {
            let mut builder = lcx.ranged_builder(&mut fcx, text, 1.0, true);
            builder.push_default(StyleProperty::FontFamily(family.clone()));
            let mut layout = builder.build(text);
            layout.break_all_lines(None);
            for line in layout.lines() {
                for item in line.items() {
                    if let PositionedLayoutItem::GlyphRun(run) = item {
                        let font = run.run().font();
                        assert!(run.glyphs().all(|g| g.id != 0), "{text}: empty glyphs in the stack {family:?}");
                        if text.starts_with('😀') {
                            assert!(has_table(font.data.data(), font.index, b"COLR"), "emoji were taken from a monochrome font");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn default_primary_recolors_to_original_tokens() {
    let close = |a: Color, b: Color| a.to_rgba8().to_u8_array().iter().zip(b.to_rgba8().to_u8_array()).all(|(x, y)| x.abs_diff(y) <= 1);
    for (dark, original) in [(false, tokens::LIGHT), (true, tokens::DARK)] {
        let [r, g, b] = DEFAULT_PRIMARY.map(|c| f32::from(c) / 255.0);
        let mut theme = original;
        tokens::recolor(&mut theme, dark, [[r, g, b, 1.0], [r, g, b, 1.0], [r * 0.78, g * 0.78, b * 0.78, 1.0]]);
        assert!(close(theme.bg_accent, original.bg_accent));
        assert!(close(theme.bg_accent_subtle, original.bg_accent_subtle));
        assert!(close(theme.text_link, original.text_link));
        assert!(close(theme.border_focus_ring, original.border_focus_ring));
    }
    let mut theme = tokens::LIGHT;
    tokens::recolor(&mut theme, false, [[1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0], [0.78, 0.0, 0.0, 1.0]]);
    assert_eq!(theme.bg_accent.to_rgba8().to_u8_array(), [255, 0, 0, 255]);
    assert_eq!(parse_hex("#E05A2B"), Some([0xe0, 0x5a, 0x2b]));
    assert_eq!(parse_hex("e05a2"), None);
}

#[test]
fn graph_backdrop_is_the_note_background_in_both_themes() {
    let _lock = super::test_lock();
    let before = super::is_dark();
    for dark in [true, false] {
        super::apply_setting(if dark { "dark" } else { "light" }, dark, "#1471eb");
        let theme = super::current();
        assert_eq!(theme.graph_backdrop, theme.editor_bg, "the graph background does not match the note background (dark theme: {dark})");
    }
    super::apply_setting(if before { "dark" } else { "light" }, before, "#1471eb");
}

#[test]
fn a_system_family_is_asked_for_by_its_own_name_with_a_fallback() {
    let masonry::parley::FontFamily::Source(stack) = super::editor_font("Georgia") else { panic!("a list of names was expected") };
    assert!(stack.starts_with("\"Georgia\", \"iA Writer Quattro\""), "{stack}");
    let masonry::parley::FontFamily::Source(mono) = super::editor_font("iA Writer Mono") else { panic!("a list of names was expected") };
    assert!(mono.contains("monospace"));
}

#[test]
fn installed_families_are_sorted_unique_and_skip_the_bundled_ones() {
    let known = ["Inter", "iA Writer Quattro", "iA Writer Mono"];
    let names = super::installed_families(&known);
    assert!(names.windows(2).all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase()));
    assert!(names.iter().all(|name| !known.contains(&name.as_str()) && !name.starts_with(['.', '@'])));
}

fn rgba(color: Color) -> [u8; 4] {
    color.to_rgba8().to_u8_array()
}

fn builtin(id: &str) -> (usize, &'static tokens::Builtin) {
    tokens::THEMES.iter().enumerate().find(|(_, b)| b.id == id).unwrap_or_else(|| panic!("no theme {id}"))
}

#[test]
fn builtin_themes_start_with_aquilum_and_have_unique_ids() {
    let ids: Vec<&str> = tokens::THEMES.iter().map(|b| b.id).collect();
    assert_eq!(&ids[..2], ["light", "dark"]);
    assert!(ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id)), "{ids:?}");
    assert_eq!(super::builtins(false).map(|b| b.id).collect::<Vec<_>>(), ["light", "catppuccin-latte", "paper-notebook", "paper-legal", "paper-kraft", "paper-washi", "paper-pastel"]);
    assert_eq!(super::builtins(true).map(|b| b.id).collect::<Vec<_>>(), ["dark", "catppuccin-frappe", "catppuccin-macchiato", "catppuccin-mocha", "paper-chalkboard", "paper-moleskine"]);
    let light = tokens::LIGHT;
    assert!((light.editor_heading_1_scale - 24.0 / 14.0).abs() < 1e-6);
    assert_eq!((light.editor_heading_3_scale, light.editor_heading_3_weight), (1.0, 700.0));
    assert_eq!(rgba(light.editor_heading_4_color), rgba(light.editor_text), "in the Aquilum theme headings use the text color");
}

#[test]
fn catppuccin_takes_its_palette_and_anuppuccin_heading_colors() {
    let mocha = builtin("catppuccin-mocha").1.theme;
    assert_eq!(rgba(mocha.editor_bg), [0x1e, 0x1e, 0x2e, 0xff]);
    assert_eq!(rgba(mocha.sidebar_bg), [0x18, 0x18, 0x25, 0xff]);
    assert_eq!(rgba(mocha.editor_code_bg), [0x11, 0x11, 0x1b, 0xff]);
    let headings = [mocha.editor_heading_1_color, mocha.editor_heading_2_color, mocha.editor_heading_3_color, mocha.editor_heading_4_color];
    assert_eq!(headings.map(rgba), [[0xf3, 0x8b, 0xa8, 0xff], [0xfa, 0xb3, 0x87, 0xff], [0xa6, 0xe3, 0xa1, 0xff], [0x94, 0xe2, 0xd5, 0xff]]);
    assert_eq!((rgba(mocha.editor_strong_color), rgba(mocha.editor_em_color)), ([0xf3, 0x8b, 0xa8, 0xff], [0xa6, 0xe3, 0xa1, 0xff]));
    assert_eq!([mocha.editor_heading_1_scale, mocha.editor_heading_4_scale], [1.8, 1.27]);
    assert_eq!(rgba(mocha.text_primary)[..3], [0xcd, 0xd6, 0xf4], "text is the palette's Text");
    let latte = builtin("catppuccin-latte").1.theme;
    assert_eq!(rgba(latte.editor_bg), [0xef, 0xf1, 0xf5, 0xff]);
    assert_eq!(rgba(latte.bg_accent), [0x1e, 0x66, 0xf5, 0xff]);
    assert_eq!(rgba(latte.editor_heading_1_color), [0xd2, 0x0f, 0x39, 0xff]);
}

#[test]
fn derive_without_edits_reproduces_every_theme() {
    let close = |a: Color, b: Color| rgba(a).iter().zip(rgba(b)).all(|(x, y)| x.abs_diff(y) <= 1);
    for (index, builtin) in tokens::THEMES.iter().enumerate() {
        let mut theme = builtin.theme;
        tokens::derive(index, &mut theme, &[None; tokens::INPUT_COUNT]);
        for (at, name) in tokens::INPUTS.iter().enumerate() {
            assert!(close(tokens::input_color(&theme, at), tokens::input_color(&builtin.theme, at)), "{}: {name}", builtin.id);
        }
        for (name, derived, original) in [
            ("editor-text", theme.editor_text, builtin.theme.editor_text),
            ("tab-bg-active", theme.tab_bg_active, builtin.theme.tab_bg_active),
            ("selection-bg", theme.selection_bg, builtin.theme.selection_bg),
            ("graph-backdrop", theme.graph_backdrop, builtin.theme.graph_backdrop),
        ] {
            assert!(close(derived, original), "{}: {name}", builtin.id);
        }
    }
}

#[test]
fn an_edited_color_reaches_the_tokens_built_from_it() {
    let (index, mocha) = builtin("catppuccin-mocha");
    let mut set = [None; tokens::INPUT_COUNT];
    let at = |name: &str| tokens::INPUTS.iter().position(|input| *input == name).unwrap();
    set[at("editor-bg")] = Some([0.0, 0.0, 0.0, 1.0]);
    set[at("text-primary")] = Some([1.0, 0.0, 0.0, 1.0]);
    let mut theme = mocha.theme;
    tokens::derive(index, &mut theme, &set);
    assert_eq!(rgba(theme.editor_bg), [0, 0, 0, 255]);
    assert_eq!(rgba(theme.tab_bg_active), [0, 0, 0, 255], "the active tab uses the note colors");
    assert_eq!(rgba(theme.graph_backdrop), [0, 0, 0, 255]);
    assert_eq!(rgba(theme.editor_text), [255, 0, 0, 230], "editor text is 90% of the text color");
    assert_eq!(rgba(theme.editor_heading_1_color), rgba(mocha.theme.editor_heading_1_color), "a custom heading color is kept");
    let (light, aquilum) = builtin("light");
    let mut theme = aquilum.theme;
    tokens::derive(light, &mut theme, &set);
    assert_eq!(rgba(theme.editor_heading_2_color), rgba(theme.editor_text), "in Aquilum the heading follows the text");
}

#[test]
fn the_mode_picks_the_chosen_theme_and_its_edits() {
    let _lock = super::test_lock();
    let before = super::is_dark();
    let edits = [("editor-bg".to_owned(), "#000000".to_owned()), ("editor-heading-1-scale".to_owned(), "2".to_owned())];
    super::set_choice(super::Choice {
        light: "catppuccin-latte".into(),
        dark: "catppuccin-mocha".into(),
        overrides: [("catppuccin-mocha".to_owned(), edits.into_iter().collect())].into_iter().collect(),
    });
    super::apply_setting("dark", false, "#1471eb");
    assert_eq!(super::active().id, "catppuccin-mocha");
    assert_eq!(rgba(super::current().editor_bg), [0, 0, 0, 255]);
    assert_eq!(super::current().editor_heading_1_scale, 2.0);
    assert_eq!(super::number("editor-heading-1-scale"), Some(2.0));
    assert_eq!(super::input_hex("sidebar-bg").as_deref(), Some("#181825"));
    super::apply_setting("light", true, "#1471eb");
    assert_eq!(super::active().id, "catppuccin-latte");
    assert_eq!(super::input_hex("editor-bg").as_deref(), Some("#eff1f5"));
    super::set_choice(super::Choice { light: "catppuccin-mocha".into(), ..super::Choice::default() });
    super::apply_setting("light", false, "#1471eb");
    assert_eq!(super::active().id, "light", "a dark theme is not applied in light mode");
    super::set_choice(super::Choice::default());
    super::apply_setting(if before { "dark" } else { "light" }, before, "#1471eb");
}

#[test]
fn the_calendar_has_its_own_color_that_follows_the_sidebar_unless_the_theme_sets_it() {
    let (index, latte) = builtin("catppuccin-latte");
    assert_eq!(rgba(latte.theme.calendar_bg), rgba(latte.theme.sidebar_bg), "without its own color the calendar sits on the sidebar");
    let notebook = builtin("paper-notebook").1.theme;
    assert_eq!(rgba(notebook.calendar_bg), [0xff, 0xfd, 0xf4, 0xff]);
    assert_ne!(rgba(notebook.calendar_bg), rgba(notebook.sidebar_bg));
    let mut set = [None; tokens::INPUT_COUNT];
    set[tokens::INPUTS.iter().position(|input| *input == "calendar-bg").unwrap()] = Some([1.0, 0.0, 0.0, 1.0]);
    let mut theme = latte.theme;
    tokens::derive(index, &mut theme, &set);
    assert_eq!(rgba(theme.calendar_bg), [255, 0, 0, 255]);
    assert_eq!(rgba(theme.sidebar_bg), rgba(latte.theme.sidebar_bg), "editing the calendar leaves the sidebar alone");
}
