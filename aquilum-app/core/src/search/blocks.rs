//! The blocks of a note a link can point at (`[[Note#^id]]`): paragraphs, list items, quotes and callouts, and tables.
//!
//! A block may already carry an identifier. A paragraph or a list item keeps it at the end of its last line (`text ^id`); a quote,
//! a callout or a table keeps it on a line of its own right after a blank line (`^id`), as Obsidian does.

use std::hash::{BuildHasher, Hasher};

const PREVIEW: usize = 80;

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The start of the block, without its markers, for a person to recognize it.
    pub preview: String,
    pub id: Option<String>,
    /// Byte offset just after the text of the last line of the block: where an identifier is written.
    pub end: usize,
    /// That last line, to make sure the note did not change before an identifier is written.
    pub line: String,
    /// Quotes, callouts and tables take their identifier on a line of its own.
    pub structured: bool,
    /// Another line follows right away, with no blank line between.
    pub followed: bool,
}

impl Block {
    /// Where to write the identifier \`id\` and what: at the end of the line, or on a line of its own after a blank one.
    pub fn id_edit(&self, id: &str) -> (usize, String) {
        if self.structured {
            (self.end, format!("\n\n^{id}{}", if self.followed { "\n" } else { "" }))
        } else {
            (self.end, format!(" ^{id}"))
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The identifier a line ends with (` ^id`), and where the space before it starts.
fn trailing_id(text: &str) -> Option<(usize, &str)> {
    let trimmed = text.trim_end();
    let caret = trimmed.rfind('^')?;
    let id = &trimmed[caret + 1..];
    let before = &trimmed[..caret];
    (valid_id(id) && before.ends_with(char::is_whitespace)).then(|| (before.trim_end().len(), id))
}

fn alone_id(text: &str) -> Option<&str> {
    let t = text.trim();
    t.strip_prefix('^').filter(|id| valid_id(id))
}

fn is_heading(t: &str) -> bool {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && t[hashes..].starts_with([' ', '\t']) || t.chars().all(|c| c == '#') && (1..=6).contains(&t.len())
}

fn is_rule(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3 && ['-', '*', '_'].iter().any(|&mark| compact.chars().all(|c| c == mark))
}

fn marker(t: &str) -> Option<usize> {
    let bytes = t.as_bytes();
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    let at = if digits > 0 && digits < 10 && matches!(bytes.get(digits), Some(b'.' | b')')) { digits + 1 } else if matches!(bytes.first(), Some(b'-' | b'*' | b'+')) { 1 } else { return None };
    matches!(bytes.get(at), Some(b' ' | b'\t')).then_some(at + 1)
}

fn indent(text: &str) -> usize {
    text.chars().take_while(|c| matches!(c, ' ' | '\t')).map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

fn starts_block(text: &str) -> bool {
    let t = text.trim_start();
    t.is_empty() || is_heading(t) || t.starts_with(['>', '|']) || t.starts_with("```") || t.starts_with("~~~") || t.starts_with("$$") || t.starts_with("<!--") || marker(t).is_some() || is_rule(t)
}

fn strip_task(t: &str) -> &str {
    for open in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = t.strip_prefix(open) {
            return rest;
        }
    }
    t
}

/// The first line of a block as a person would read it: no list marker, no quote bars, no \`[!kind]\` tag, no identifier.
fn preview(first: &str, structured_quote: bool) -> String {
    let mut t = first.trim();
    if structured_quote {
        t = t.trim_start_matches(['>', ' ']);
        if let Some(rest) = t.strip_prefix("[!") {
            let close = rest.find(']').unwrap_or(rest.len());
            let (kind, title) = (&rest[..close], rest[close..].trim_start_matches(']').trim_start_matches(['-', '+']).trim());
            let shown = if title.is_empty() { kind } else { title };
            return clip(&shown.to_owned());
        }
    } else if let Some(at) = marker(t) {
        t = strip_task(t[at..].trim_start());
    }
    let without_id = trailing_id(t).map_or(t, |(at, _)| &t[..at]);
    clip(&without_id.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn clip(text: &String) -> String {
    if text.chars().count() <= PREVIEW {
        return text.clone();
    }
    let cut: String = text.chars().take(PREVIEW).collect();
    format!("{}…", cut.trim_end())
}

pub fn blocks(source: &str) -> Vec<Block> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        lines.push((offset, line.trim_end_matches(['\r', '\n'])));
        offset += line.len();
    }
    let count = lines.len();
    let mut i = 0;
    // The properties at the very top are not part of the note's text.
    if lines.first().is_some_and(|(_, text)| text.trim_end() == "---") {
        if let Some(close) = (1..count).find(|&k| matches!(lines[k].1.trim_end(), "---" | "...")) {
            i = close + 1;
        }
    }
    let mut out: Vec<Block> = Vec::new();
    // Line after which only blank lines were seen since the last block ended (to attach a lone \`^id\`).
    let mut last_end = 0;
    let mut fence: Option<&str> = None;
    while i < count {
        let (_, text) = lines[i];
        let t = text.trim_start();
        if let Some(open) = fence {
            if t.starts_with(open) {
                fence = None;
            }
            i += 1;
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            i += 1;
            continue;
        }
        if t.starts_with("$$") {
            if !(t.len() >= 4 && t[2..].contains("$$")) {
                fence = Some("$$");
            }
            i += 1;
            continue;
        }
        if t.is_empty() || is_heading(t) || is_rule(t) {
            i += 1;
            continue;
        }
        if t.starts_with("<!--") {
            while i < count && !lines[i].1.contains("-->") {
                i += 1;
            }
            i += 1;
            continue;
        }
        if let Some(id) = alone_id(t) {
            let only_blanks = (last_end..i).all(|k| lines[k].1.trim().is_empty());
            if let Some(block) = out.last_mut().filter(|b| b.structured && b.id.is_none() && only_blanks) {
                block.id = Some(id.to_owned());
            }
            i += 1;
            continue;
        }
        let mut next = i + 1;
        let (structured, quote, next) = if t.starts_with('>') {
            while next < count && lines[next].1.trim_start().starts_with('>') {
                next += 1;
            }
            (true, true, next)
        } else if t.starts_with('|') {
            while next < count && lines[next].1.trim_start().starts_with('|') {
                next += 1;
            }
            (true, false, next)
        } else if marker(t).is_some() {
            let own = indent(text);
            while next < count && !lines[next].1.trim().is_empty() && indent(lines[next].1) > own && marker(lines[next].1.trim_start()).is_none() && !lines[next].1.trim_start().starts_with('>') {
                next += 1;
            }
            (false, false, next)
        } else {
            while next < count && !starts_block(lines[next].1) {
                next += 1;
            }
            (false, false, next)
        };
        let last = next - 1;
        let (last_offset, last_text) = lines[last];
        let id = trailing_id(last_text).map(|(_, id)| id.to_owned());
        let shown = preview(text, quote);
        let followed = lines.get(next).is_some_and(|(_, following)| !following.trim().is_empty());
        if !shown.is_empty() {
            out.push(Block { preview: shown, id, end: last_offset + last_text.trim_end().len(), line: last_text.to_owned(), structured, followed });
        }
        last_end = next;
        i = next;
    }
    out
}

/// A fresh identifier of six letters and digits that no block in \`taken\` has.
pub fn new_id(taken: &[Block]) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    loop {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        let mut bits = hasher.finish();
        let id: String = (0..6)
            .map(|_| {
                let digit = DIGITS[(bits % 36) as usize] as char;
                bits /= 36;
                digit
            })
            .collect();
        if !taken.iter().any(|block| block.id.as_deref() == Some(id.as_str())) {
            return id;
        }
    }
}

#[cfg(test)]
mod tests;
