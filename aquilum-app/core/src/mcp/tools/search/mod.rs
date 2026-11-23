mod links;
mod query;
mod related;

use super::{tool, workspace_property};
use crate::app_core::Core;
use crate::search::analysis::models::AnalysisMethod;
use serde_json::{json, Value};

const SNIPPET_LIMIT: usize = 300;

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "search_notes",
            "Full-text search over the workspace with stemming and typo tolerance (one typo from 5 letters, two from 9, the first letter must be exact). Ranking: first notes that match more query words, then those without typos, then with words in the title, with a title equal to the query, with words in headings, and by BM25. Partial matches are also returned: see matchedTerms. Syntax: \"exact phrase\" means words adjacent (required), -word and -\"phrase\" exclude notes, #word means only in section headings (required); the last word is also matched as a word prefix. Each result has heading (the section with the match), matches (number of matches) and backlinks (how many notes link to it). hasMore tells whether anything is left beyond the returned results. frontmatter is not part of the index; use find_notes for its fields.",
            json!({
                "query": { "type": "string", "description": "Search query: words, \"phrase\", -exclusion, #heading" },
                "folder": { "type": "string", "description": "Search only in this folder; empty means the whole workspace" },
                "limit": { "type": "integer", "description": "How many notes to return, default 10" },
                "workspace": workspace_property(),
            }),
            &["query"],
        ),
        tool(
            "dataview_syntax",
            "The ```dataview block language: query forms, clauses, fields and functions, with examples. Call without arguments to get the reference, or pass query to validate a query before inserting it into a note.",
            json!({
                "query": { "type": "string", "description": "Query text to validate; empty returns the reference" },
            }),
            &[],
        ),
        tool(
            "related_notes",
            "Notes similar by text (BM25F): what to write next to it, what is duplicated, what to expand.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "limit": { "type": "integer", "description": "How many notes to return, default 10" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "linked_notes",
            "Related notes by the [[...]] link graph (Adamic-Adar): shared neighbors, not shared text.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "limit": { "type": "integer", "description": "How many notes to return, default 10" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "backlinks",
            "Notes that link to the given one: one row per note, mentions is how many times the link occurs, heading and context are the section and line of the first mention.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "outgoing_links",
            "[[...]] links from the given note, including not-yet-created targets, one row per target; mentions is how many times it is mentioned.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "wiki_sources",
            "Wikipedia articles on the topic of the note.",
            json!({
                "note": { "type": "string", "description": "Note path or title" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "get_index_status",
            "Search index status: whether it is ready and how many notes are indexed.",
            json!({
                "workspace": workspace_property(),
            }),
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
        "search_notes" => query::search_notes(core, arguments),
        "dataview_syntax" => query::dataview_syntax(arguments),
        "related_notes" => related::analyze(core, arguments, AnalysisMethod::Bm25f),
        "linked_notes" => related::analyze(core, arguments, AnalysisMethod::AdamicAdar),
        "backlinks" => links::backlinks(core, arguments),
        "outgoing_links" => links::outgoing_links(core, arguments),
        "wiki_sources" => related::wiki_sources(core, arguments),
        "get_index_status" => query::index_status(core, arguments),
        _ => return None,
    })
}
