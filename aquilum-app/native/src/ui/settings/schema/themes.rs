use super::*;
use crate::ui::theme;
use crate::ui::tokens::Builtin;

/// Theme colors edited by the user (`tokens::derive` inputs after the three accent colors).
const COLORS: [&str; 14] = [
    "bg-canvas",
    "sidebar-bg",
    "tabs-bg",
    "calendar-bg",
    "calendar-accent",
    "editor-bg",
    "text-primary",
    "text-secondary",
    "text-tertiary",
    "border-default",
    "text-link",
    "editor-code-bg",
    "editor-strong-color",
    "editor-em-color",
];
const HEADING_COLORS: [&str; 6] = ["editor-heading-1-color", "editor-heading-2-color", "editor-heading-3-color", "editor-heading-4-color", "editor-heading-5-color", "editor-heading-6-color"];
const HEADING_SCALES: [&str; 6] = ["editor-heading-1-scale", "editor-heading-2-scale", "editor-heading-3-scale", "editor-heading-4-scale", "editor-heading-5-scale", "editor-heading-6-scale"];
const HEADING_WEIGHTS: [&str; 6] = ["editor-heading-1-weight", "editor-heading-2-weight", "editor-heading-3-weight", "editor-heading-4-weight", "editor-heading-5-weight", "editor-heading-6-weight"];
const WEIGHTS: [u32; 5] = [400, 500, 600, 700, 800];

fn weight_label(weight: u32) -> String {
    t(match weight {
        400 => "settings.font.weightRegular",
        500 => "settings.font.weightMedium",
        600 => "settings.font.weightSemibold",
        700 => "settings.themes.weightBold",
        _ => "settings.themes.weightHeavy",
    })
}

fn chooser(dark: bool, selected: &str, set: fn(&mut AppConfig, &Value)) -> Row {
    let key = if dark { "dark" } else { "light" };
    let list: Vec<&Builtin> = theme::builtins(dark).collect();
    let options = list.iter().map(|builtin| builtin.name.to_owned()).collect();
    let selected = list.iter().position(|builtin| builtin.id == selected).unwrap_or(0);
    row(t(&format!("settings.themes.{key}")), Some(t(&format!("settings.themes.{key}Hint"))), Control::Dropdown { options, selected }, set)
}

fn pick(dark: bool, value: &Value) -> String {
    theme::builtins(dark).nth(index(value)).map_or(if dark { "dark" } else { "light" }, |builtin| builtin.id).to_owned()
}

pub(super) fn themes(config: &AppConfig) -> Vec<Block> {
    let ui = &config.ui;
    let active = theme::active();
    let mut colors: Vec<Row> = COLORS
        .iter()
        .map(|key| bound(t(&format!("settings.themes.color.{key}")), None, Control::Color(theme::input_hex(key).unwrap_or_default()), Setter::ThemeValue(key)))
        .collect();
    let edited = ui.theme_overrides.get(active.id).is_some_and(|edits| !edits.is_empty());
    colors.push(info(
        t("settings.themes.reset"),
        Some(t_with("settings.themes.resetHint", &[("theme", active.name)])),
        Control::Buttons(vec![Button::new(t("settings.themes.resetButton"), Action::ResetTheme).enabled(edited)]),
    ));
    let mut blocks = vec![
        Block {
            title: t("settings.themes.section"),
            rows: vec![
                chooser(false, &ui.light_theme, |c, v| c.ui.light_theme = pick(false, v)),
                chooser(true, &ui.dark_theme, |c, v| c.ui.dark_theme = pick(true, v)),
            ],
        },
        Block { title: t_with("settings.themes.colors", &[("theme", active.name)]), rows: colors },
    ];
    for level in 0..6 {
        let scale = theme::number(HEADING_SCALES[level]).unwrap_or(1.0);
        let weight = theme::number(HEADING_WEIGHTS[level]).unwrap_or(700.0);
        let selected = WEIGHTS.iter().position(|w| *w as f32 >= weight).unwrap_or(WEIGHTS.len() - 1);
        let size = Number { stepper: true, ..Number::new((f64::from(scale) * 100.0).round(), 80.0, 300.0, 5.0) };
        blocks.push(Block {
            title: t_with("settings.themes.heading", &[("level", &(level + 1).to_string()), ("marks", &"#".repeat(level + 1))]),
            rows: vec![
                bound(t("settings.themes.headingColor"), None, Control::Color(theme::input_hex(HEADING_COLORS[level]).unwrap_or_default()), Setter::ThemeValue(HEADING_COLORS[level])),
                bound(t("settings.themes.headingSize"), Some(t("settings.themes.headingSizeHint")), Control::Number(size), Setter::ThemeValue(HEADING_SCALES[level])),
                bound(
                    t("settings.themes.headingWeight"),
                    None,
                    Control::Dropdown { options: WEIGHTS.iter().map(|w| weight_label(*w)).collect(), selected },
                    Setter::ThemeValue(HEADING_WEIGHTS[level]),
                ),
            ],
        });
    }
    blocks
}

/// Edits `key` of theme `theme` with a value from a settings field; `false` means the value did not fit.
pub fn set_theme_value(config: &mut AppConfig, theme: &str, key: &str, value: &Value) -> bool {
    let stored = if key.ends_with("-scale") {
        let Value::Number(percent) = value else { return false };
        format_number(percent / 100.0)
    } else if key.ends_with("-weight") {
        WEIGHTS[index(value).min(WEIGHTS.len() - 1)].to_string()
    } else {
        let Some([r, g, b]) = theme::parse_hex(&text(value)) else { return false };
        format!("#{r:02x}{g:02x}{b:02x}")
    };
    config.ui.theme_overrides.entry(theme.to_owned()).or_default().insert(key.to_owned(), stored);
    true
}
