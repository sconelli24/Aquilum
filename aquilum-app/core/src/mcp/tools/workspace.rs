use super::super::bridge;
use super::super::vault::Vault;
use super::workspace_locator::{
    address, known_paths, known_workspace, open_guest, wait_until,
};
pub use super::workspace_locator::workspace_vault;
use super::{optional_text, text, tool, workspace_property};
use crate::search::paths::{canonical_path, same_path};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use crate::app_core::Core;

const SWITCH_TIMEOUT: Duration = Duration::from_secs(20);

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "get_workspace",
            "Current workspace: path, name, the note currently open (activeNote) and index readiness.",
            json!({}),
            &[],
        ),
        tool(
            "list_workspaces",
            "All workspaces that have been opened in the app. address is the shortest unambiguous path tail: pass it to other tools as the workspace argument.",
            json!({}),
            &[],
        ),
        tool(
            "switch_workspace",
            "Switch the visible workspace in the user's interface window. WARNING: this is a visual switch of the app window! Call ONLY if the user directly asked to switch the workspace on screen. For reading, searching, creating and editing notes in another workspace do NOT call switch_workspace; pass the workspace argument to the needed tool (read_note, update_note, search_notes, etc.), which work in the background without switching the user's screen.",
            json!({
                "path": { "type": "string", "description": "Path to the workspace folder or its address from list_workspaces" },
                "workspace": { "type": "string", "description": "Synonym of path: path to the workspace folder or its address from list_workspaces" },
                "note": { "type": "string", "description": "Note to open after switching" },
            }),
            &[],
        ),
        tool(
            "open_note",
            "Open a note in the app interface and make it the active tab. WARNING: this is a visual switch of the user's screen! Call ONLY at the user's direct request to open the note on screen. A note from another workspace (workspace) will switch the app to that workspace.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
    ]
}

pub fn call(
    core: &Core,
    name: &str,
    arguments: &Value,
) -> Option<Result<Value, String>> {
    Some(match name {
        "get_workspace" => get_workspace(core),
        "list_workspaces" => list_workspaces(core),
        "switch_workspace" => switch_workspace(core, arguments),
        "open_note" => open_note(core, arguments),
        _ => return None,
    })
}

pub fn requested_vault(core: &Core, arguments: &Value) -> Result<Vault, String> {
    match optional_text(arguments, "workspace") {
        Some(input) => workspace_vault(core, &input),
        None => Vault::active(core),
    }
}

pub fn indexed_vault(core: &Core, arguments: &Value) -> Result<Vault, String> {
    let vault = requested_vault(core, arguments)?;
    if !vault.visible {
        open_guest(core, &vault.root)?;
    }
    Ok(vault)
}

fn get_workspace(core: &Core) -> Result<Value, String> {
    let vault = Vault::active(core)?;
    let status = core.search.status(None);
    let active = core.active_note.get();
    Ok(json!({
        "path": vault.root.to_string_lossy(),
        "name": vault.root.file_name().map(|name| name.to_string_lossy().into_owned()),
        "activeNote": active.as_deref().map(|note| vault.relative(note)),
        "indexedDocuments": status.indexed_documents,
        "indexing": status.updating,
    }))
}

fn list_workspaces(core: &Core) -> Result<Value, String> {
    let active = core.search.active_root();
    let known = known_paths(core)?;
    let items = known
        .iter()
        .map(|root| {
            json!({
                "path": root.to_string_lossy(),
                "address": address(&known, root),
                "active": active.as_ref().is_some_and(|active| same_path(active, root)),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "workspaces": items }))
}

fn switch_workspace(core: &Core, arguments: &Value) -> Result<Value, String> {
    let input = optional_text(arguments, "path")
        .or_else(|| optional_text(arguments, "workspace"))
        .or_else(|| optional_text(arguments, "address"))
        .ok_or_else(|| "No workspace specified (parameter “path” or “workspace”)".to_string())?;
    let path = known_workspace(core, &input)?;
    switch_to(core, &path, optional_text(arguments, "note").as_deref())
}

fn open_note(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let note = text(arguments, "note")?;
    if !vault.visible {
        return switch_to(core, &vault.root, Some(&note));
    }
    let target = vault.note(&note)?;
    bridge::open_note(core, &target, true);
    Ok(json!({ "opened": vault.relative(&target) }))
}

fn switch_to(core: &Core, path: &Path, note: Option<&str>) -> Result<Value, String> {
    bridge::switch_workspace(core, path);
    let root = canonical_path(path);
    let service = &core.search;
    let switched = wait_until(SWITCH_TIMEOUT, || {
        service
            .active_root()
            .is_some_and(|active| same_path(&active, &root))
    });
    if !switched {
        return Err("The app did not manage to switch to this workspace".to_owned());
    }

    let vault = Vault::active(core)?;
    let opened = match note {
        Some(note) => {
            let target = vault.note(note)?;
            bridge::open_note(core, &target, false);
            Some(vault.relative(&target))
        }
        None => None,
    };
    Ok(json!({
        "workspace": vault.root.to_string_lossy(),
        "opened": opened,
    }))
}
