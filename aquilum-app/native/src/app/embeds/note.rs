//! Notes embedded in other notes (`![[Note]]`, `![[Note#Heading]]`, `![[Note#^block-id]]`): which part of the source text is shown.

use std::path::PathBuf;
use std::sync::Arc;

use aquilum_core::search::error::SearchError;
use aquilum_core::search::headings::{headings, matching};
use aquilum_core::search::paths::{is_markdown, same_path, strip_markdown_extension};

use super::Inner;
use crate::ui::editor::resolve::{Blocked, Fetch, NoteText};

/// How many notes deep embeds go: a note shown inside a note shown inside a note.
pub(super) const MAX_DEPTH: u8 = 4;

/// The part of `source` an anchor points to, and where it starts in `source`.
///
/// No anchor is the whole note without its properties, `Heading` is that section with everything under it (the heading line
/// included, like Obsidian) and `^id` is one block: a paragraph that ends with ` ^id`, or the block above a line holding only `^id`.
pub(super) fn extract(source: &str, anchor: Option<&str>) -> Option<(String, usize)> {
    let body = crate::frontmatter::end(source).unwrap_or(0);
    let Some(anchor) = anchor.map(str::trim).filter(|anchor| !anchor.is_empty()) else {
        let rest = &source[body..];
        let skipped = rest.len() - rest.trim_start_matches(['\r', '\n']).len();
        return Some((rest.trim_matches(['\r', '\n']).to_owned(), body + skipped));
    };
    match anchor.strip_prefix('^') {
        Some(id) => block(source, body, id.trim()),
        None => {
            // `Note#Part#Subpart` points at the last part.
            let wanted = anchor.rsplit('#').next().unwrap_or(anchor);
            let list = headings(source);
            let found = *matching(&list, wanted).first()?;
            let section = &source[found.start..found.content.end.max(found.start)];
            Some((section.trim_end().to_owned(), found.start))
        }
    }
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// Lines of `source` with the byte offset where each starts.
fn lines(source: &str) -> Vec<(usize, &str)> {
    let mut offset = 0;
    source
        .split_inclusive('\n')
        .map(|line| {
            let at = offset;
            offset += line.len();
            (at, line.trim_end_matches(['\r', '\n']))
        })
        .collect()
}

fn block(source: &str, body: usize, id: &str) -> Option<(String, usize)> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let marker = format!("^{id}");
    let all = lines(source);
    let at = all.iter().position(|(start, line)| {
        *start >= body && (line.trim_end() == marker || line.trim_end().strip_suffix(marker.as_str()).is_some_and(|before| before.ends_with(char::is_whitespace)))
    })?;
    let alone = all[at].1.trim() == marker;
    // The block is the run of non-blank lines that holds the identifier; with the identifier alone on its line, the run above it.
    let end = if alone {
        let above = all[..at].iter().rposition(|(_, line)| !is_blank(line))?;
        above
    } else {
        at
    };
    let mut first = end;
    while first > 0 && !is_blank(all[first - 1].1) && all[first - 1].0 >= body {
        first -= 1;
    }
    let mut last = end;
    while !alone && last + 1 < all.len() && !is_blank(all[last + 1].1) {
        last += 1;
    }
    let text: Vec<String> = (first..=last)
        .map(|k| {
            let line = all[k].1;
            match line.trim_end().strip_suffix(marker.as_str()) {
                Some(before) => before.trim_end().to_owned(),
                None => line.to_owned(),
            }
        })
        .collect();
    Some((text.join("\n").trim().to_owned(), all[first].0))
}

/// Where the note a link target names is. `Err(None)` means the index is not ready yet; `Err(Some(_))`, that there is no such note.
pub(super) fn locate(inner: &Inner, target: &str) -> Result<PathBuf, Option<String>> {
    let (workspace, document) = (inner.workspace(), inner.source());
    // The index may still be getting ready: ask again for a moment before giving up until it announces itself.
    let mut found = inner.core.search.resolve_wiki_links(&workspace, &document, &[target.to_owned()]);
    for _ in 0..30 {
        if !matches!(found, Err(SearchError::Unavailable { .. })) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        found = inner.core.search.resolve_wiki_links(&workspace, &document, &[target.to_owned()]);
    }
    match found {
        Err(SearchError::Unavailable { .. }) => Err(None),
        Err(error) => Err(Some(error.to_string())),
        Ok(found) => found.paths.into_iter().next().flatten().map(PathBuf::from).filter(|path| is_markdown(path)).ok_or_else(|| Some(target.to_owned())),
    }
}

/// The note `target` (a wiki link target) shown from the note that holds the embed, or why it cannot be shown.
pub(super) fn load(inner: &Inner, target: &str, anchor: Option<&str>) -> Fetch<NoteText> {
    let path = match locate(inner, target) {
        Ok(path) => path,
        Err(None) => return Fetch::Pending,
        Err(Some(reason)) => return Fetch::Missing(reason),
    };
    let title = path.file_stem().map_or_else(|| strip_markdown_extension(target).to_owned(), |stem| stem.to_string_lossy().into_owned());
    let label = anchor.map(str::trim).filter(|a| !a.is_empty()).map(|a| a.trim_start_matches('^').to_owned());
    let blocked = if inner.chain.iter().any(|ancestor| same_path(ancestor, &path)) {
        Some(Blocked::Loop)
    } else if inner.depth >= MAX_DEPTH {
        Some(Blocked::Depth)
    } else {
        None
    };
    if blocked.is_some() {
        return Fetch::Ready(NoteText { path, title, label, text: Arc::from(""), start: 0, blocked });
    }
    let source = match crate::note::read(&path) {
        Ok(loaded) => loaded.text.to_string(),
        Err(error) => return Fetch::Missing(error.to_string()),
    };
    match extract(&source, anchor) {
        Some((text, start)) => Fetch::Ready(NoteText { path, title, label, text: Arc::from(text), start, blocked: None }),
        None => Fetch::Missing(anchor.unwrap_or_default().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::extract;

    const NOTE: &str = "---\ntype: note\n---\n\n# Один\n\nвступление\n\n## Два\n\nтекст два ^abc-1\n\n### Три\n\nглубже\n\n## Четыре\n\nконец\n\n- пункт\n- ещё\n\n^lst\n";

    #[test]
    fn the_whole_note_comes_without_its_properties() {
        let (text, start) = extract(NOTE, None).unwrap();
        assert!(text.starts_with("# Один") && text.ends_with("^lst"), "{text:?}");
        assert!(!text.contains("type: note"));
        assert!(NOTE[start..].starts_with("# Один"));
        assert_eq!(extract("просто текст\n", Some("  ")).unwrap().0, "просто текст");
    }

    #[test]
    fn a_heading_brings_its_section_with_everything_under_it() {
        let (text, start) = extract(NOTE, Some("Два")).unwrap();
        assert!(text.starts_with("## Два") && text.contains("### Три") && text.contains("глубже"), "{text:?}");
        assert!(!text.contains("Четыре"), "the section stops at the next heading of its level");
        assert!(NOTE[start..].starts_with("## Два"));
        let (nested, _) = extract(NOTE, Some("Один#Два#Три")).unwrap();
        assert!(nested.starts_with("### Три") && !nested.contains("Четыре"));
        assert_eq!(extract(NOTE, Some("два")).unwrap().0, text, "titles match without regard to case");
        assert!(extract(NOTE, Some("Нет такого")).is_none());
    }

    #[test]
    fn a_block_is_one_paragraph_without_its_identifier() {
        let (text, start) = extract(NOTE, Some("^abc-1")).unwrap();
        assert_eq!(text, "текст два");
        assert!(NOTE[start..].starts_with("текст два ^abc-1"));
        assert!(extract(NOTE, Some("^nope")).is_none());
        assert!(extract(NOTE, Some("^bad id")).is_none(), "identifiers are letters, digits and dashes");
    }

    #[test]
    fn an_identifier_alone_on_its_line_names_the_block_above_it() {
        let (text, _) = extract(NOTE, Some("^lst")).unwrap();
        assert_eq!(text, "- пункт\n- ещё");
        let multi = "строка раз\nстрока два ^m1\n\nдругой абзац\n";
        assert_eq!(extract(multi, Some("^m1")).unwrap().0, "строка раз\nстрока два");
    }
}
