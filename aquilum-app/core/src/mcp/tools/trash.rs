use super::super::vault::Vault;
use super::workspace::requested_vault;
use super::{limit, listing, offset, require_write, text, tool, workspace_property};
use crate::files::gate;
use crate::files::trash::{empty_trash_impl, list_trash_impl, trash_page_impl, trash_path, TRASH_FOLDER};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use crate::app_core::Core;

const LIST_LIMIT: usize = 500;

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "list_trash",
            "Deleted notes in the workspace trash, newest first: original is where the note used to be, deletedAtMs is the deletion time in Unix milliseconds, path is the address in the trash for restore_note. The trash is cleaned automatically after the period set in settings.",
            json!({
                "offset": { "type": "integer", "description": "How many entries to skip to get the next page" },
                "limit": { "type": "integer", "description": "How many entries to return, default 100, maximum 500" },
                "workspace": workspace_property(),
            }),
            &[],
        ),
        tool(
            "restore_note",
            "Restore a note from the trash to its former place. Accepts a path from list_trash or the note's former path; of several copies with the same path the newest is restored. If the former place is taken, the note is restored next to it with the suffix \" (1)\".",
            json!({
                "note": { "type": "string", "description": "path from list_trash or the note's former path, for example \"Projects/Idea.md\"" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "empty_trash",
            "Empty the trash permanently. Call only at the user's direct request.",
            json!({ "workspace": workspace_property() }),
            &[],
        ),
    ]
}

pub fn call(core: &Core, name: &str, arguments: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "list_trash" => list_trash(core, arguments),
        "restore_note" => restore_note(core, arguments),
        "empty_trash" => empty_trash(core, arguments),
        _ => return None,
    })
}

fn list_trash(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let size = limit(arguments, 100, LIST_LIMIT);
    let offset = offset(arguments);
    let trash = trash_page_impl(&vault.root, offset, size).map_err(|error| error.to_string())?;
    let page = trash
        .files
        .iter()
        .map(|item| json!({ "path": vault.relative(Path::new(&item.path)), "original": item.original, "deletedAtMs": item.deleted_at_ms, "bytes": item.bytes }))
        .collect();
    Ok(listing("notes", trash.total, offset, page))
}

fn restore_note(core: &Core, arguments: &Value) -> Result<Value, String> {
    require_write(core)?;
    let vault = requested_vault(core, arguments)?;
    let trashed = trashed_note(&vault, &text(arguments, "note")?)?;
    let restored = gate::restore(core, &vault.root, &trashed).map_err(|error| error.to_string())?;
    Ok(json!({ "restored": vault.relative(&restored) }))
}

fn empty_trash(core: &Core, arguments: &Value) -> Result<Value, String> {
    require_write(core)?;
    let vault = requested_vault(core, arguments)?;
    let removed = empty_trash_impl(&vault.root).map_err(|error| error.to_string())?;
    Ok(json!({ "removed": removed }))
}

fn trashed_note(vault: &Vault, wanted: &str) -> Result<PathBuf, String> {
    let normalized = wanted.trim().replace('\\', "/");
    if let Some(inside) = normalized.strip_prefix(&format!("{TRASH_FOLDER}/")) {
        return Ok(trash_path(&vault.root).join(inside));
    }
    let original = normalized.trim_start_matches('/').to_lowercase();
    let with_extension = format!("{original}.md");
    list_trash_impl(&vault.root)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|item| {
            let candidate = item.original.to_lowercase();
            candidate == original || candidate == with_extension
        })
        .map(|item| PathBuf::from(item.path))
        .ok_or_else(|| format!("There is no note “{wanted}” in the trash: list_trash shows the list"))
}

#[cfg(test)]
mod tests {
    use super::trashed_note;
    use crate::files::trash::{move_to_trash_impl, trash_path};
    use crate::mcp::vault::Vault;
    use std::fs;

    #[test]
    fn a_note_is_found_in_the_trash_by_its_former_path_or_by_its_trash_address() {
        let directory = tempfile::tempdir().unwrap();
        let root = crate::search::paths::canonical_path(directory.path());
        let vault = Vault { root: root.clone(), visible: false };
        let note = root.join("Проекты").join("Идея.md");
        fs::create_dir_all(note.parent().unwrap()).unwrap();
        fs::write(&note, "первая").unwrap();
        let older = move_to_trash_impl(&root, &note).unwrap().root;
        let day_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 60 * 60);
        fs::File::options().write(true).open(&older).unwrap().set_modified(day_ago).unwrap();
        fs::write(&note, "вторая").unwrap();
        let newest = move_to_trash_impl(&root, &note).unwrap().root;

        assert_eq!(trashed_note(&vault, "Проекты/Идея").unwrap(), newest, "newest copy");
        assert_eq!(trashed_note(&vault, "проекты\\идея.md").unwrap(), newest);
        assert_eq!(
            trashed_note(&vault, ".trash/Проекты/Идея.md").unwrap(),
            trash_path(&root).join("Проекты/Идея.md")
        );
        assert!(trashed_note(&vault, "Нет такой").is_err());
    }
}
