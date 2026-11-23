mod frontmatter;
mod history;
mod notes;
mod search;
mod trash;
mod workspace;
mod workspace_locator;
mod arguments;

pub use arguments::*;

use super::vault::Vault;
use std::path::Path;
use serde_json::{json, Value};
use crate::app_core::Core;


pub const INSTRUCTIONS: &str = concat!(
    "Aquilum is a local knowledge base of markdown files. ",
    "Notes are addressed by a path relative to the workspace root (e.g. \"Projects/Aquilum.md\") ",
    "or by exact title. Absolute paths are not needed. ",
    "If the user says \"the current note\" or \"this note\", first call ",
    "get_workspace and take the path from the activeNote field. ",
    "Before drawing conclusions about the workspace contents, check the indexing field: while the index is being built, ",
    "search may return incomplete results.\n",
    "Workflow: start an unfamiliar workspace with list_notes sort=links, which puts the hubs on top, ",
    "the notes most other notes link to; then search instead of browsing at random; ",
    "read a long note not in full but via note_outline and the section you need; edit ",
    "by section, not the whole note.\n",
    "Background work with another workspace: pass its address from list_workspaces as the workspace argument; ",
    "search, reading and editing will run in a background guest index and nothing will switch on the user's screen. ",
    "Do NOT call switch_workspace or open_note for reading or writing: they switch the user's visible window! ",
    "switch_workspace and open_note are called ONLY at the user's direct request to show/open a workspace or note on screen."
);

pub fn definitions() -> Vec<Value> {
    let mut all = search::definitions();
    all.extend(notes::definitions());
    all.extend(frontmatter::definitions());
    all.extend(trash::definitions());
    all.extend(history::definitions());
    all.extend(workspace::definitions());
    all
}

pub fn call(core: &Core, name: &str, arguments: &Value) -> Result<Value, String> {
    search::call(core, name, arguments)
        .or_else(|| notes::call(core, name, arguments))
        .or_else(|| frontmatter::call(core, name, arguments))
        .or_else(|| trash::call(core, name, arguments))
        .or_else(|| history::call(core, name, arguments))
        .or_else(|| workspace::call(core, name, arguments))
        .unwrap_or_else(|| Err(format!("Unknown tool: {name}")))
}

pub fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

pub fn version_name_property() -> Value {
    json!({ "type": "string", "description": "Name of the version this edit creates: it will appear under this name in the note history right away" })
}

pub fn open_after_write(core: &Core, vault: &Vault, note: &Path, arguments: &Value) {
    if vault.visible && flag(arguments, "open", true) && !core.active_note.is(note) {
        super::bridge::open_note_after_write(core, note);
    }
}

pub fn workspace_property() -> Value {
    json!({ "type": "string", "description": "Workspace, if not the current one: address from list_workspaces or a full path" })
}

impl super::vault::Vault {
    pub fn note_argument(&self, arguments: &Value) -> Result<std::path::PathBuf, String> {
        self.note(&text(arguments, "note")?)
    }

    pub fn folder_argument(&self, arguments: &Value) -> Result<std::path::PathBuf, String> {
        let folder = self.folder(&optional_text(arguments, "folder").unwrap_or_default())?;
        match folder.is_dir() {
            true => Ok(folder),
            false => Err(format!("Folder not found: {}", self.relative(&folder))),
        }
    }
}

pub fn shorten(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }
    let head = value.chars().take(maximum).collect::<String>();
    format!("{head}…")
}

pub fn require_write(core: &Core) -> Result<(), String> {
    if core.settings.get_config().mcp.allow_write {
        return Ok(());
    }
    Err("Editing notes is disabled in Aquilum settings".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{optional_names, text};
    use serde_json::json;

    #[test]
    fn path_from_a_search_result_works_as_note() {
        let found = json!({ "path": "Base/Заметка.md" });
        assert_eq!(text(&found, "note").unwrap(), "Base/Заметка.md");
        assert_eq!(text(&json!({ "note": "Заметка" }), "path").unwrap(), "Заметка");
    }

    #[test]
    fn the_named_argument_wins_over_its_alias() {
        let both = json!({ "note": "Своя.md", "path": "Чужая.md" });
        assert_eq!(text(&both, "note").unwrap(), "Своя.md");
    }

    #[test]
    fn the_error_lists_the_arguments_that_did_arrive() {
        let error = text(&json!({ "heading": "Раздел" }), "note").unwrap_err();
        assert!(error.contains("note"), "the missing field is named: {error}");
        assert!(error.contains("heading"), "the passed field is named: {error}");
        assert!(text(&json!({}), "note").unwrap_err().contains("no arguments were passed"));
    }

    #[test]
    fn a_list_argument_is_absent_empty_or_valid() {
        assert!(optional_names(&json!({}), "notes").unwrap().is_none());
        assert!(optional_names(&json!({ "notes": null }), "notes").unwrap().is_none());
        assert!(optional_names(&json!({ "notes": [] }), "notes").is_err());
        assert!(optional_names(&json!({ "notes": "одна" }), "notes").is_err());
        assert!(optional_names(&json!({ "notes": ["a", " "] }), "notes").is_err());
        assert_eq!(
            optional_names(&json!({ "notes": ["a", " b "] }), "notes").unwrap(),
            Some(vec!["a".to_owned(), "b".to_owned()])
        );
    }
}
