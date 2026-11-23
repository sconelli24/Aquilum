use super::workspace::requested_vault;
use super::{open_after_write, optional_text, require_write, tool, version_name_property, workspace_property};
use crate::app_core::Core;
use crate::files::document::read_head;
use crate::files::error::FileCommandError;
use crate::files::gate;
use crate::history::Source;
use crate::search::analyzer::frontmatter::{same_key, set as frontmatter_set};
use crate::search::fields::{as_object, note_fields};
use serde_json::{json, Value};

mod find;

use find::find_notes;

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "set_frontmatter",
            "Change a note's frontmatter fields: the listed keys are created or overwritten; other fields, their order and the note text are left untouched. An empty string clears the value but keeps the key. If the note has no frontmatter, the block is created at the start. List fields come back as arrays in the response.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "fields": { "type": "object", "description": "\"key: value\" pairs, for example {\"status\": \"read\", \"rating\": 5}" },
                "open": { "type": "boolean", "description": "Open the note in a tab in the app, default true; no tab is opened for another workspace" },
                "versionName": version_name_property(),
                "workspace": workspace_property(),
            }),
            &["note", "fields"],
        ),
        tool(
            "find_notes",
            "Select notes by frontmatter fields: exact match (match), filled (has) and unfilled (missing). This is not full-text search: search_notes searches note contents, while this searches metadata, e.g. \"all notes with type: book\", \"books without an author\". Field values are returned only for the keys listed in fields. List fields (`tags: [a, b]` or a dash list) come back as arrays, an empty list counts as an unfilled field, and match compares the whole value, not individual list items.",
            json!({
                "match": { "type": "object", "description": "Fields that must match exactly (case-insensitive), for example {\"type\": \"book\"}" },
                "has": { "type": "array", "items": { "type": "string" }, "description": "These fields must be filled" },
                "missing": { "type": "array", "items": { "type": "string" }, "description": "These fields must be absent or empty" },
                "fields": { "type": "array", "items": { "type": "string" }, "description": "Which fields to return; without it only paths are returned" },
                "folder": { "type": "string", "description": "Search only in this folder; empty means the whole workspace" },
                "sort": { "type": "string", "description": "Field to sort by; defaults to sorting by path" },
                "offset": { "type": "integer", "description": "How many notes to skip to get the next page" },
                "limit": { "type": "integer", "description": "How many notes to return, default 100, maximum 200" },
                "workspace": workspace_property(),
            }),
            &[],
        ),
    ]
}

pub fn call(core: &Core, name: &str, arguments: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "set_frontmatter" => set_frontmatter(core, arguments),
        "find_notes" => find_notes(core, arguments),
        _ => return None,
    })
}

pub fn map(text: &str) -> Value {
    Value::Object(as_object(&note_fields(text)))
}

fn set_frontmatter(core: &Core, arguments: &Value) -> Result<Value, String> {
    require_write(core)?;
    let vault = requested_vault(core, arguments)?;
    let note = vault.note_argument(arguments)?;
    let updates = field_updates(arguments)?;
    let updates = updates.iter().map(|(key, value)| (key.as_str(), Some(value.as_str()))).collect::<Vec<_>>();
    let version_name = optional_text(arguments, "versionName");
    let changed = gate::retrying(&note, |hash| {
        let head = read_head(&note)?;
        let change = frontmatter_set(&head, &updates, same_key).map_err(|message| FileCommandError::Task { message })?;
        match change {
            Some(change) => gate::edit(core, &note, &[change], Some(hash), Source::Agent, version_name.as_deref()).map(|_| true),
            None => Ok(false),
        }
    })
    .map_err(|error| error.to_string())?;
    if changed {
        open_after_write(core, &vault, &note, arguments);
    }
    let head = read_head(&note).map_err(|error| error.to_string())?;
    Ok(json!({
        "path": vault.relative(&note),
        "changed": changed,
        "frontmatter": map(&head),
    }))
}

fn field_updates(arguments: &Value) -> Result<Vec<(String, String)>, String> {
    let fields = arguments
        .get("fields")
        .and_then(Value::as_object)
        .ok_or("Argument “fields” is missing: an object of the form {“key”: “value”} is required")?;
    if fields.is_empty() {
        return Err("The field list is empty: nothing to change".to_owned());
    }
    fields
        .iter()
        .map(|(key, value)| {
            if key.trim().is_empty() || key.contains(':') || key.contains('\n') {
                return Err(format!("Invalid field name “{key}”: colons and line breaks are not allowed in a key"));
            }
            let value = scalar(key, value)?;
            if value.contains('\n') {
                return Err(format!("Field “{key}”: the value must not be multiline"));
            }
            Ok((key.trim().to_owned(), value))
        })
        .collect()
}

fn scalar(key: &str, value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(_) | Value::Bool(_) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        _ => Err(format!("Field “{key}”: the value must be a string, number or boolean")),
    }
}
