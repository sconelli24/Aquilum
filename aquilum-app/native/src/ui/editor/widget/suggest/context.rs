use super::*;

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

fn key_char(c: char) -> bool {
    word_char(c) || c == ' '
}

impl TextEditor {
    fn in_frontmatter(&self) -> bool {
        self.focus < self.doc.front()
    }

    pub(super) fn property_suggest(&self) -> Option<Suggest> {
        if self.single_line || !self.selection().is_empty() || !self.in_frontmatter() {
            return None;
        }
        let caret = self.focus;
        let Range { start: line_start, end: line_end } = self.doc.line_at(caret);
        let before = self.doc.slice(line_start..caret);
        let (from, to, items, source) = match before.split_once(':') {
            Some((key, value)) if crate::frontmatter::is_key(key, crate::frontmatter::PAGE_COVER) => {
                let from = line_start + key.len() + 1 + (value.len() - value.trim_start().len());
                (from, line_end, crate::cover::suggest_patterns(self.doc.slice(from..caret)), Source::Value)
            }
            Some(_) => return None,
            None => {
                let key = before.trim_start();
                if key.is_empty() || !key.starts_with(char::is_alphanumeric) || !key.chars().all(key_char) || !self.doc.slice(caret..line_end).trim().is_empty() {
                    return None;
                }
                let from = line_start + before.len() - key.len();
                (from, caret, crate::frontmatter::suggest_keys(key), Source::Key)
            }
        };
        Some(Suggest { from, to, query: self.doc.slice(from..caret).to_owned(), items, hints: Vec::new(), selected: 0, source, note: None, blocks: None })
    }

    /// `::` at the start of a line of a Cornell note lists what can be opened there: a cue or the summary.
    pub(super) fn cornell_suggest(&self) -> Option<Suggest> {
        use crate::i18n::t;
        if self.single_line || self.doc.markdown.is_none() || !self.selection().is_empty() || !self.doc.cornell_note() || self.in_frontmatter() {
            return None;
        }
        let caret = self.focus;
        let paragraph = self.doc.paragraph(self.doc.at_offset(caret));
        if paragraph.block.is_code() || !self.doc.slice(caret..paragraph.end()).trim().is_empty() {
            return None;
        }
        let typed = self.doc.slice(paragraph.start..caret).strip_prefix("::")?.to_owned();
        if !typed.chars().all(char::is_alphabetic) {
            return None;
        }
        let matcher = Matcher::new(&typed, IGNORE_CASE);
        let (items, hints): (Vec<String>, Vec<String>) = [("::question", t("editor.cornellQuestion")), ("::summary", t("editor.cornellSummary"))]
            .into_iter()
            .filter(|(marker, _)| typed.is_empty() || matcher.first(&marker[2..]).is_some())
            .map(|(marker, hint)| (marker.to_owned(), hint))
            .unzip();
        (!items.is_empty()).then(|| Suggest { from: paragraph.start, to: caret, query: typed, items, hints, selected: 0, source: Source::Cornell, note: None, blocks: None })
    }

    pub(super) fn suggest_context(&self) -> Option<Context> {
        if self.single_line || self.doc.markdown.is_none() || !self.selection().is_empty() {
            return None;
        }
        let caret = self.focus;
        let paragraph = self.doc.paragraph(self.doc.at_offset(caret));
        if paragraph.block.is_code() {
            return None;
        }
        let Range { start: line_start, end: line_end } = paragraph.range();
        let before = self.doc.slice(line_start..caret);
        if before.matches('`').count() % 2 == 1 {
            return None;
        }
        if let Some(open) = before.rfind(OPEN) {
            let query = &before[open + OPEN.len()..];
            if !query.contains(CLOSE) {
                if query.contains('|') {
                    return None;
                }
                let base = line_start + open + OPEN.len();
                let after = self.doc.slice(caret..line_end);
                if let Some(first) = query.find('#') {
                    // `[[Note#Part`: the headings of that note, from the moment the # is typed. `[[#` is the note itself.
                    let last = query.rfind('#').unwrap_or(first);
                    let heading = &query[last + 1..];
                    let stop = after.char_indices().find(|&(at, c)| c == '|' || after[at..].starts_with(CLOSE)).map(|(at, _)| at);
                    let close = stop.is_none();
                    // `[[Note#^` lists the blocks of the note instead; the ^ is part of what gets replaced.
                    return Some(match heading.strip_prefix('^') {
                        Some(block) => Context { from: base + last + 1, to: caret + stop.unwrap_or(0), query: block.to_owned(), source: Source::Block { close }, note: Some(query[..first].to_owned()) },
                        None => Context { from: base + last + 1, to: caret + stop.unwrap_or(0), query: heading.to_owned(), source: Source::Heading { close }, note: Some(query[..first].to_owned()) },
                    });
                }
                let stop = after.char_indices().find(|&(at, c)| matches!(c, '|' | '#') || after[at..].starts_with(CLOSE)).map(|(at, _)| at);
                return Some(Context { from: base, to: caret + stop.unwrap_or(0), query: query.to_owned(), source: Source::Link { wrap: false, close: stop.is_none() }, note: None });
            }
        }
        if self.in_frontmatter() {
            return None;
        }
        let start = before.char_indices().rev().take_while(|&(_, c)| word_char(c)).last().map(|(i, _)| i)?;
        if before[..start].ends_with('#') {
            return None;
        }
        Some(Context { from: line_start + start, to: caret, query: before[start..].to_owned(), source: Source::Link { wrap: true, close: true }, note: None })
    }
}

/// What the caret is in the middle of typing, and which text it would replace.
pub(super) struct Context {
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) query: String,
    pub(super) source: Source,
    /// For a heading: the note it is in (empty is the note being edited).
    pub(super) note: Option<String>,
}
