use crate::files::document::write_content_atomic;
use crate::settings::models::AppConfig;
use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct SettingsManager {
    config_path: PathBuf,
    current_config: RwLock<AppConfig>,
}

impl SettingsManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let config_path = app_data_dir.join("settings.json");
        let current_config = Self::load_or_default(&config_path);
        Self {
            config_path,
            current_config: RwLock::new(current_config),
        }
    }

    fn load_or_default(path: &Path) -> AppConfig {
        match fs::read_to_string(path) {
            Ok(content) => match parse(&content) {
                Ok(config) => return config,
                Err(error) => set_aside_unreadable(path, &error),
            },
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => eprintln!("[aquilum:settings] failed to read {}: {error}", path.display()),
        }
        let default_config = AppConfig::default();
        if let Err(error) = write_config(path, &default_config) {
            eprintln!("[aquilum:settings] failed to write default settings: {error}");
        }
        default_config
    }

    pub fn get_config(&self) -> AppConfig {
        self.current_config.read().unwrap().clone()
    }

    pub fn update_config(&self, new_config: AppConfig) -> Result<(), String> {
        write_config(&self.config_path, &new_config)?;
        *self.current_config.write().unwrap() = new_config;
        Ok(())
    }
}

fn overlay(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                overlay(base.entry(key).or_insert(Value::Null), value);
            }
        }
        (base, over) => *base = over,
    }
}

fn parse(content: &str) -> Result<AppConfig, serde_json::Error> {
    let mut merged = serde_json::to_value(AppConfig::default())?;
    let saved: Value = serde_json::from_str(content)?;
    // Before the accent followed the theme, a color chosen by hand was the accent: keep it manual.
    let chose_by_hand = saved.pointer("/ui/primaryColor").and_then(Value::as_str).is_some_and(|color| !color.eq_ignore_ascii_case(&AppConfig::default().ui.primary_color)) && saved.pointer("/ui/accentAuto").is_none();
    overlay(&mut merged, saved);
    let mut config: AppConfig = serde_json::from_value(merged)?;
    if chose_by_hand {
        config.ui.accent_auto = false;
    }
    // The red theme of 0.3.23 and the night mode no longer exist: both open as the dark theme.
    if matches!(config.ui.theme.as_str(), "red" | "night") {
        config.ui.theme = "dark".into();
    }
    Ok(config)
}

fn write_config(path: &Path, config: &AppConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    write_content_atomic(path, &json, None)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn set_aside_unreadable(path: &Path, error: &serde_json::Error) {
    let backup = path.with_extension("json.unreadable");
    eprintln!(
        "[aquilum:settings] {} could not be parsed ({error}), a copy was saved to {}",
        path.display(),
        backup.display()
    );
    if let Err(copy_error) = fs::copy(path, &backup) {
        eprintln!("[aquilum:settings] failed to save a copy: {copy_error}");
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsManager;

    #[test]
    fn partial_settings_keep_present_values_and_fill_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"mcp":{"token":"secret"},"editor":{"fontFamily":"iA Writer Quattro","saveDebounceMs":5000}}"#,
        )
        .unwrap();

        let config = SettingsManager::new(dir.path()).get_config();

        assert_eq!(config.mcp.token, "secret");
        assert_eq!(config.mcp.port, 8787);
        assert_eq!(config.editor.font.font_family, "iA Writer Quattro");
        assert!(config.analysis.enable_bm25f);
        assert!(config.ui.animations);
    }

    #[test]
    fn the_removed_red_and_night_themes_open_as_the_dark_theme() {
        for old in ["red", "night"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("settings.json"), format!(r#"{{"ui":{{"theme":"{old}","night":"warm"}}}}"#)).unwrap();

            let config = SettingsManager::new(dir.path()).get_config();

            assert_eq!(config.ui.theme, "dark");
        }
    }

    #[test]
    fn the_accent_follows_the_theme_unless_a_color_was_chosen_by_hand_before() {
        let open = |json: &str| {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("settings.json"), json).unwrap();
            SettingsManager::new(dir.path()).get_config()
        };
        assert!(open("{}").ui.accent_auto, "a fresh profile follows the theme");
        assert!(open(r##"{"ui":{"primaryColor":"#1471EB"}}"##).ui.accent_auto, "the default color was never a choice");
        let manual = open(r##"{"ui":{"primaryColor":"#e05a2b"}}"##);
        assert!(!manual.ui.accent_auto, "a color picked in an older version stays the accent");
        assert_eq!(manual.ui.primary_color, "#e05a2b");
        assert!(open(r##"{"ui":{"primaryColor":"#e05a2b","accentAuto":true}}"##).ui.accent_auto, "an explicit choice wins over the migration");
    }

    #[test]
    fn wikipedia_lookups_are_off_for_a_fresh_profile_but_a_saved_choice_is_kept() {
        let open = |json: &str| {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("settings.json"), json).unwrap();
            SettingsManager::new(dir.path()).get_config()
        };
        assert!(!open("{}").analysis.enable_wikixiv, "note-derived words never go to Wikipedia unless asked for");
        assert!(open(r#"{"analysis":{"enableWikixiv":true}}"#).analysis.enable_wikixiv, "turned on by the user");
    }

    #[test]
    fn a_fresh_profile_starts_with_small_fonts_and_a_65_character_column() {
        let dir = tempfile::tempdir().unwrap();
        let config = SettingsManager::new(dir.path()).get_config();
        assert_eq!((config.ui.font.font_size_base, config.editor.font.font_size_base, config.editor.max_width_ch), (14, 16, 65));
        assert_eq!((config.editor.font.font_family.as_str(), config.ui.font.font_family.as_str()), ("iA Writer Mono", "Inter"));
    }

    #[test]
    fn an_editor_section_without_font_keys_keeps_the_editor_font_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), r#"{"editor":{"maxWidthCh":70}}"#).unwrap();
        let config = SettingsManager::new(dir.path()).get_config();
        assert_eq!(config.editor.max_width_ch, 70);
        assert_eq!((config.editor.font.font_size_base, config.editor.font.font_family.as_str()), (16, "iA Writer Mono"));
    }

    #[test]
    fn unreadable_settings_are_set_aside_before_defaults_are_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ broken").unwrap();

        SettingsManager::new(dir.path());

        let backup = std::fs::read_to_string(dir.path().join("settings.json.unreadable")).unwrap();
        assert_eq!(backup, "{ broken");
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"mcp\""));
    }
}
