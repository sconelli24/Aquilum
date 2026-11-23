use super::workspace::requested_vault;
use super::{listing, offset, text_window,
    limit, open_after_write, optional_text, require_write, text, tool, version_name_property,
    workspace_property,
};
use crate::files::gate;
use crate::history::{version_at, Source};
use crate::search::pages::Pages;
use serde_json::{json, Value};
use crate::app_core::Core;

const LIST_LIMIT: usize = 200;
const READ_LIMIT: usize = 8_000;

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "note_history",
            "Versions of a note from its history, newest first. id is for read_version, restore_version and name_version; atMs is the Unix time in milliseconds; source is who made the edit: me is the user, agent is the agent (each agent call is a separate version), external is an edit from outside, links is links updated after another note was renamed, start is the initial text, restore and revert are restoring a version and undoing its changes (fromMs is the time of that version); name is the version name.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "offset": { "type": "integer", "description": "How many versions to skip to get the next page" },
                "limit": { "type": "integer", "description": "How many versions to return, default 50, maximum 200" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "read_version",
            "Text of one note version by id from note_history. Long text is returned in parts: see the truncated and nextOffset fields.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "version": { "type": "string", "description": "Version id from note_history" },
                "offset": { "type": "integer", "description": "Character offset to start reading from" },
                "limit": { "type": "integer", "description": "How many characters to return, default 8000" },
                "workspace": workspace_property(),
            }),
            &["note", "version"],
        ),
        tool(
            "restore_version",
            "Restore a note version: its text becomes the current text. The text from before the restore stays in history, so restoring is reversible. Call only at the user's request.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "version": { "type": "string", "description": "Version id from note_history" },
                "versionName": version_name_property(),
                "open": { "type": "boolean", "description": "Open the note in a tab in the app, default true; no tab is opened for another workspace" },
                "workspace": workspace_property(),
            }),
            &["note", "version"],
        ),
        tool(
            "name_version",
            "Name a note version, rename it, or remove the name with an empty name. Without version, the latest version of the note is named. Named versions are not deleted when the history retention period expires.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "version": { "type": "string", "description": "Version id from note_history; without it, the latest version of the note" },
                "name": { "type": "string", "description": "Name; an empty string removes it" },
                "workspace": workspace_property(),
            }),
            &["note", "name"],
        ),
    ]
}

pub fn call(core: &Core, name: &str, arguments: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "note_history" => note_history(core, arguments),
        "read_version" => read_version(core, arguments),
        "restore_version" => restore_version(core, arguments),
        "name_version" => name_version(core, arguments),
        _ => return None,
    })
}

fn missing_version(id: &str) -> String {
    format!("The note has no version {id}: take an id from note_history")
}

fn note_history(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let note = vault.note_argument(arguments)?;
    let offset = offset(arguments);
    let page = core.history.page(&|| core.known_roots(), &note, None, offset, limit(arguments, 50, LIST_LIMIT));
    let mut listing = listing("versions", page.total, offset, page.versions);
    listing["path"] = json!(vault.relative(&note));
    Ok(listing)
}

fn read_version(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let note = vault.note_argument(arguments)?;
    let id = text(arguments, "version")?;
    let version = core.history.content(&|| core.known_roots(), &note, &id).ok_or_else(|| missing_version(&id))?;
    let mut window = text_window(&mut Pages::of(&version), 0..usize::MAX, arguments, limit(arguments, READ_LIMIT, 40_000))?;
    window["path"] = json!(vault.relative(&note));
    window["version"] = json!(id);
    Ok(window)
}

fn restore_version(core: &Core, arguments: &Value) -> Result<Value, String> {
    require_write(core)?;
    let vault = requested_vault(core, arguments)?;
    let note = vault.note_argument(arguments)?;
    let id = text(arguments, "version")?;
    let from_ms = version_at(&id).ok_or_else(|| missing_version(&id))?;
    let version_name = optional_text(arguments, "versionName");
    let changed = gate::restore_version(core, &note, &id, Source::Restore(from_ms), version_name.as_deref()).map_err(|error| error.to_string())?;
    if changed {
        open_after_write(core, &vault, &note, arguments);
    }
    Ok(json!({ "path": vault.relative(&note), "version": id, "changed": changed }))
}

fn name_version(core: &Core, arguments: &Value) -> Result<Value, String> {
    require_write(core)?;
    let vault = requested_vault(core, arguments)?;
    let note = vault.note_argument(arguments)?;
    let version = optional_text(arguments, "version");
    let name = optional_text(arguments, "name").unwrap_or_default();
    let named = gate::name_version(core, &note, version.as_deref(), &name)
        .ok_or_else(|| match &version {
            Some(id) => missing_version(id),
            None => "The note has no versions yet: nothing to name".to_owned(),
        })?;
    Ok(json!({ "path": vault.relative(&note), "version": named, "name": name.trim() }))
}
