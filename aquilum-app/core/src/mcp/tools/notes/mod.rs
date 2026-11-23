use super::super::vault::Vault;
use super::workspace::{indexed_vault, requested_vault, workspace_vault};
use super::{listing, offset, text_window,
    content, content_or_empty, flag, limit, open_after_write, optional_content, optional_names, optional_text,
    require_write, shorten, text, tool, version_name_property, workspace_property,
};
use crate::files::document::read_head;
use crate::search::pages::{read_chars, Pages};
use crate::files::gate;
use crate::history::Source;
use crate::search::headings::{self, Heading};
use crate::text::matcher::{Matcher, IGNORE_CASE};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use crate::app_core::Core;

const READ_LIMIT: usize = 8_000;
const MANY_READ_LIMIT: usize = 2_000;
const LIST_LIMIT: usize = 200;
const BATCH_LIMIT: usize = 100;

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "read_note",
            "Read a whole note or one of its sections (argument heading). Several notes can be read at once via notes: the response is a list, and each note is truncated to 2000 characters by default. Frontmatter fields are returned as a separate frontmatter object, list values in it come back as arrays, and they must be changed via set_frontmatter. Long text is returned in parts: see the truncated and nextOffset fields.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "notes": { "type": "array", "items": { "type": "string" }, "description": "Several notes in one call, at most 100; instead of note" },
                "heading": { "type": "string", "description": "Read only this section: the heading and the text up to the next heading of the same or a higher level" },
                "offset": { "type": "integer", "description": "Character offset to start reading from" },
                "limit": { "type": "integer", "description": "How many characters to return, default 8000" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "create_note",
            "Create a note. Folders are created automatically. If a file already exists at that path, the call returns an error and changes nothing, so there is no need to check existence first. For import, pass notes: the list of notes is created in one call, an error on one does not cancel the others, and no tabs are opened.",
            json!({
                "path": { "type": "string", "description": "Path from the workspace root, for example \"Projects/Idea.md\"" },
                "content": { "type": "string", "description": "Initial content" },
                "notes": { "type": "array", "items": { "type": "object" }, "description": "Batch of notes: a list of {path, content} objects, at most 100; instead of path and content" },
                "workspace": workspace_property(),
                "open": { "type": "boolean", "description": "Open the note in a tab in the app: default true for a single note and false for a batch" },
                "versionName": version_name_property(),
            }),
            &[],
        ),
        tool(
            "update_note",
            "Change a note's text. Edit by section (underHeading, replaceSection) or by exact replacement (findReplace); mode=replace overwrites the whole note including frontmatter, so change metadata via set_frontmatter, not with mode replace.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "mode": {
                    "type": "string",
                    "enum": ["append", "prepend", "replace", "findReplace", "underHeading", "replaceSection"],
                    "description": "Edit mode, default append. underHeading inserts the text as the first paragraph of the section, replaceSection replaces the section text, keeping the heading",
                },
                "content": { "type": "string", "description": "New text or replacement; in findReplace and replaceSection modes it may be empty, in which case the found text or the section text is deleted" },
                "find": { "type": "string", "description": "What to replace in findReplace mode" },
                "heading": { "type": "string", "description": "Section heading for underHeading and replaceSection modes" },
                "all": { "type": "boolean", "description": "In findReplace mode, replace all occurrences; default false, and with several occurrences an error with their count is returned" },
                "open": { "type": "boolean", "description": "Open the note in a tab in the app, default true; no tab is opened for another workspace" },
                "versionName": version_name_property(),
                "workspace": workspace_property(),
            }),
            &["note", "content"],
        ),
        tool(
            "rename_note",
            "Rename a note, updating all incoming [[...]] links.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "title": { "type": "string", "description": "New title without the extension" },
                "workspace": workspace_property(),
            }),
            &["note", "title"],
        ),
        tool(
            "move_note",
            "Move a note or a batch of notes (notes) to another folder or another workspace. When moving between workspaces, embedded images and files travel with the notes: those used only by the moved notes are moved, those shared with other notes are copied. brokenLinks lists notes in the source workspace whose links to the moved notes will break (links within the batch do not count).",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "notes": { "type": "array", "items": { "type": "string" }, "description": "Several notes in one call; instead of note" },
                "folder": { "type": "string", "description": "Destination folder; empty means the workspace root" },
                "workspace": workspace_property(),
                "toWorkspace": { "type": "string", "description": "Destination workspace for moving between workspaces: path or name from list_workspaces" },
            }),
            &[],
        ),
        tool(
            "delete_note",
            "Delete a note: the file is moved to the workspace trash at the same location inside .trash and kept there for the period set in settings. It can be restored via restore_note and listed via list_trash.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "list_notes",
            "All notes of the workspace, paginated: total is the overall count, nextOffset is where to continue. A cheap way to see the whole workspace. With sort=links notes come as {path, links} objects, most linked first: a map of the hubs of an unfamiliar workspace.",
            json!({
                "folder": { "type": "string", "description": "Folder; empty means the whole workspace" },
                "query": { "type": "string", "description": "Substring in the title" },
                "offset": { "type": "integer", "description": "How many paths to skip to get the next page" },
                "limit": { "type": "integer", "description": "How many paths to return, default 100, maximum 200" },
                "sort": { "type": "string", "enum": ["path", "links"], "description": "path sorts by path (default), links sorts by the number of notes that link to this one" },
                "workspace": workspace_property(),
            }),
            &[],
        ),
        tool(
            "note_outline",
            "Headings of a note with the level and size of each section. The section you need can then be read via read_note with the heading argument.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "list_folders",
            "Folder structure of the workspace.",
            json!({ "workspace": workspace_property() }),
            &[],
        ),
    ]
}

pub fn call(
    core: &Core,
    name: &str,
    arguments: &Value,
) -> Option<Result<Value, String>> {
    Some(match name {
        "read_note" => read_note(core, arguments),
        "create_note" => create_note(core, arguments),
        "update_note" => update_note(core, arguments),
        "rename_note" => rename_note(core, arguments),
        "move_note" => move_note(core, arguments),
        "delete_note" => delete_note(core, arguments),
        "list_notes" => list_notes(core, arguments),
        "note_outline" => note_outline(core, arguments),
        "list_folders" => list_folders(core, arguments),
        _ => return None,
    })
}


mod create;
mod edit;
mod list;
mod moves;
mod read;
#[cfg(test)]
mod tests;
mod transfer;
#[cfg(test)]
mod transfer_tests;

use create::*;
use edit::*;
use list::*;
use moves::*;
use read::*;
