//! Cornell notes: a note whose properties say `type: cornell` keeps its notes in a column at the right, leaving room at the left
//! for the cues (`::question text`) and takes the whole width again for the closing summary (`::summary text`). A cue or a
//! summary is a paragraph that starts with its marker; callouts are not touched.

use masonry::core::{BrushIndex, StyleSet};
use masonry::parley::{Alignment, FontContext, Layout, LayoutContext};

use super::super::markdown::Block;
use super::{Document, text};

/// Narrower windows keep the usual single column.
const MIN_WIDTH: f32 = 700.0;
const GUTTER: f32 = 44.0;
/// Room for the column headers over the first row, and for the title of the summary over its text.
pub const HEADER: f64 = 44.0;

/// Air under the last line of a row, before the line that closes it.
pub(super) const ROW_PAD: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Properties, headings: the whole width.
    Full,
    /// The closing summary: the whole width, as plain text under the last row.
    Summary,
    /// A question in the left column.
    Cue,
    /// Everything else, in the right column.
    Notes,
}

impl Document {
    /// The note's own properties say so; when they are hidden from the editor the page says so (`set_cornell`).
    pub fn set_cornell(&mut self, cornell: bool) {
        if self.told != cornell {
            self.told = cornell;
            self.invalidate_all();
        }
    }

    /// Whether the note is a Cornell note, however wide the window is.
    pub fn cornell_note(&self) -> bool {
        if self.front > 0 { self.cornell } else { self.told }
    }

    pub fn is_cornell(&self) -> bool {
        self.cornell_note() && self.width.is_some_and(|width| width >= MIN_WIDTH)
    }

    /// Width of the left column, without the gutter.
    pub fn cue_width(&self) -> f32 {
        self.width.map_or(0.0, |width| (width * 0.27).clamp(180.0, 280.0))
    }

    /// Whether the text of a line is in code or properties, where a marker means nothing.
    fn literal(&self, line: usize) -> bool {
        matches!(self.block(line), Block::Code { .. } | Block::Meta)
    }

    /// The marker a line starts with, if it is one: where its cue or summary begins.
    fn marker_of(&self, line: usize) -> Option<(Lane, usize)> {
        if self.literal(line) {
            return None;
        }
        marker(self.line(line))
    }

    pub fn lane(&self, line: usize) -> Lane {
        if !self.is_cornell() || self.start(line) < self.front || matches!(self.block(line), Block::Heading { .. }) {
            return Lane::Full;
        }
        self.cornell_kind(line).unwrap_or(Lane::Notes)
    }

    /// `Cue` or `Summary` when the line is in the paragraph a marker opens, whatever the width of the window.
    pub fn cornell_kind(&self, line: usize) -> Option<Lane> {
        if !self.cornell_note() || self.start(line) < self.front || matches!(self.block(line), Block::Heading { .. }) || self.line(line).trim().is_empty() {
            return None;
        }
        // A cue or a summary is the paragraph its marker opens: look up to the first line of the paragraph.
        let mut j = line;
        loop {
            if let Some((lane, _)) = self.marker_of(j) {
                return Some(lane);
            }
            if j == 0 || self.start(j - 1) < self.front || self.line(j - 1).trim().is_empty() {
                return None;
            }
            j -= 1;
        }
    }

    /// A line of hint text, laid out like the rest of the page.
    pub fn hint(&self, fcx: &mut FontContext, lcx: &mut LayoutContext<BrushIndex>, styles: &StyleSet, hint: &str) -> Layout<BrushIndex> {
        text::build(fcx, lcx, styles, hint, None, None, Alignment::Start, None).0
    }

    /// The kind of a cue or a summary that has no text yet, to show a hint in its place.
    pub fn placeholder(&self, line: usize) -> Option<Lane> {
        let (lane, cut) = self.marker_of(line).filter(|_| self.is_cornell())?;
        (self.bare(line).is_some() && self.line(line)[cut..].trim().is_empty()).then_some(lane)
    }

    /// Whether the line is one that opens a cue or a summary.
    pub(super) fn is_marker_line(&self, line: usize) -> bool {
        self.is_cornell() && self.marker_of(line).is_some()
    }

    /// What tells a laid line it must be laid again: the lane it is in and whether its marker is shown.
    pub(super) fn lane_key(&self, line: usize) -> u8 {
        if !self.is_cornell() {
            return 0;
        }
        let lane = match self.lane(line) {
            Lane::Full => 1,
            Lane::Summary => 2,
            Lane::Cue => 3,
            Lane::Notes => 4,
        };
        lane * 2 + u8::from(self.bare(line).is_some())
    }

    /// Where the notes column starts, from the left edge of the text.
    pub fn notes_x(&self) -> f64 {
        f64::from(self.cue_width() + GUTTER)
    }

    /// How far a line is moved right to make room for the cues.
    pub fn lane_x(&self, line: usize) -> f64 {
        self.lane_box(line).0
    }

    /// Cues and the summary are plain text: how much of their line is the marker, hidden unless the caret is in the line.
    pub(super) fn bare(&self, line: usize) -> Option<super::super::markdown::Bare> {
        if !matches!(self.lane(line), Lane::Cue | Lane::Summary) || self.inside(line) {
            return None;
        }
        let cut = self.marker_of(line).map_or(0, |(_, cut)| cut.min(self.line(line).len()));
        Some(super::super::markdown::Bare { cut, strong: self.lane(line) == Lane::Cue })
    }

    /// Where the text of a line starts and how wide it may be.
    pub(super) fn lane_box(&self, line: usize) -> (f64, Option<f32>) {
        match self.lane(line) {
            Lane::Notes => {
                let room = self.cue_width() + GUTTER;
                (f64::from(room), self.width.map(|width| (width - room).max(0.0)))
            }
            Lane::Cue => (0.0, Some(self.cue_width())),
            Lane::Full | Lane::Summary => (0.0, self.width),
        }
    }
}

/// Cues take no room in the flow of the page: they hang in the left column beside their notes. These are the rows that make
/// that work: a row is a cue and the lines up to the next cue or the next line that takes the whole width.
impl Document {
    pub(super) fn is_cue_head(&self, line: usize) -> bool {
        self.is_cornell() && matches!(self.marker_of(line), Some((Lane::Cue, _)))
    }

    /// The first line of the row `line` is in, or `line` itself when no cue opens it.
    pub(super) fn row_start(&self, line: usize) -> usize {
        if !self.is_cornell() {
            return line;
        }
        let mut j = line;
        loop {
            match self.lane(j) {
                Lane::Full | Lane::Summary => return line,
                Lane::Cue if self.is_cue_head(j) => return j,
                _ => {}
            }
            if j == 0 {
                return line;
            }
            j -= 1;
        }
    }

    /// The last line of the row `line` is in.
    pub(super) fn row_end(&self, line: usize) -> usize {
        let mut j = line;
        while self.is_cornell() && j + 1 < self.lines.count() && !matches!(self.lane(j + 1), Lane::Full | Lane::Summary) && !self.is_cue_head(j + 1) {
            j += 1;
        }
        j
    }

    /// The last line of the cue that starts at `head`.
    pub(super) fn cue_end(&self, head: usize) -> usize {
        let mut j = head;
        while j + 1 < self.lines.count() && self.lane(j + 1) == Lane::Cue && !self.is_cue_head(j + 1) {
            j += 1;
        }
        j
    }

    /// How far below the top of its row a cue line hangs: the cue lines before it take the room.
    pub(super) fn cue_lift(&self, line: usize) -> f64 {
        if self.lane(line) != Lane::Cue {
            return 0.0;
        }
        let mut lift = 0.0;
        let mut j = line;
        while j > 0 && self.lane(j - 1) == Lane::Cue {
            j -= 1;
            lift += self.heights.text(j);
        }
        // The cue is level with the first text of its notes, not with the blank line that often sits between them.
        let mut head = j;
        while !self.is_cue_head(head) && head > 0 {
            head -= 1;
        }
        lift + self.cue_offset(head)
    }

    /// The line of the first cue: the column headers (Key points, Notes) are over it.
    pub fn header_row(&self) -> Option<usize> {
        self.header_row.filter(|_| self.is_cornell())
    }

    /// The first line that opens a cue, if the note has one (and is laid out whole).
    pub(super) fn first_cue(&self) -> Option<usize> {
        if !self.is_cornell() || self.base != 0 {
            return None;
        }
        (0..self.lines.count()).find(|&line| self.start(line) >= self.front && self.is_cue_head(line))
    }

    /// The blank line after the first cue, which carries the room of the column headers: the notes start below them.
    pub(super) fn header_carrier(&self) -> Option<usize> {
        let line = self.cue_end(self.header_row?) + 1;
        (line < self.lines.count() && self.line(line).trim().is_empty()).then_some(line)
    }

    /// Room a line takes besides its text: the column headers, or the title of the summary, which hangs from the line (`summary_lift`).
    pub(super) fn extra_height(&self, line: usize, carrier: Option<usize>) -> f64 {
        if Some(line) == carrier || self.summary_lift(line) > 0.0 { HEADER } else { 0.0 }
    }

    /// How far the text of the line that opens the summary is moved down, to leave room for the title over it.
    pub(super) fn summary_lift(&self, line: usize) -> f64 {
        if self.is_cornell() && matches!(self.marker_of(line), Some((Lane::Summary, _))) { HEADER } else { 0.0 }
    }

    /// The blank lines between a cue and the first text of its notes: the cue is level with that text.
    pub(super) fn cue_offset(&self, head: usize) -> f64 {
        let mut blanks = 0.0;
        let mut k = self.cue_end(head) + 1;
        while k < self.lines.count() && self.lane(k) == Lane::Notes && self.line(k).trim().is_empty() {
            blanks += self.heights.text(k);
            k += 1;
        }
        // Only when the row has text of its own is the cue level with it; an empty row keeps it at the top. The first row starts below the headers.
        let has_text = k < self.lines.count() && self.lane(k) == Lane::Notes;
        (if has_text { blanks } else { 0.0 }) + if self.header_row == Some(head) { HEADER } else { 0.0 }
    }

    /// The rows from `first` to `last` (a cue and the lines up to the next cue), as first and last line.
    pub fn rows(&self, first: usize, last: usize) -> Vec<(usize, usize)> {
        let mut rows = Vec::new();
        if !self.is_cornell() {
            return rows;
        }
        let mut k = self.row_start(first);
        while k <= last && k < self.lines.count() {
            if self.is_cue_head(k) {
                let end = self.row_end(k);
                rows.push((k, end));
                k = end + 1;
            } else {
                k += 1;
            }
        }
        rows
    }

    /// The lines from `first` to `last` that open the summary.
    pub fn summaries(&self, first: usize, last: usize) -> Vec<usize> {
        if !self.is_cornell() {
            return Vec::new();
        }
        (first..=last.min(self.lines.count().saturating_sub(1))).filter(|&k| matches!(self.marker_of(k), Some((Lane::Summary, _)))).collect()
    }

    /// Where the line between the columns is, from the left edge of the text.
    pub fn divider_x(&self) -> f64 {
        f64::from(self.cue_width() + GUTTER / 2.0)
    }

    #[cfg(test)]
    /// Where a cue starts: its row top, and the offset that puts it level with the first text of its notes.
    pub fn cue_top(&self, head: usize) -> f64 {
        self.top(head) + self.cue_offset(head)
    }

    #[cfg(test)]
    /// The room the text of lines `first..=last` takes, one under the other.
    pub fn text_height(&self, first: usize, last: usize) -> f64 {
        (first..=last).map(|k| self.heights.text(k)).sum()
    }

    /// The cue line at a point of the left column (`x` from the left edge of the text), if there is one.
    pub fn cue_at(&self, x: f64, y: f64) -> Option<usize> {
        if !self.is_cornell() || x < 0.0 || x >= f64::from(self.cue_width()) {
            return None;
        }
        let mut j = self.at_y(y);
        loop {
            match self.lane(j) {
                Lane::Full | Lane::Summary => return None,
                Lane::Cue => {
                    let top = self.top(j) + self.cue_lift(j);
                    if (top..top + self.heights.text(j)).contains(&y) {
                        return Some(j);
                    }
                    if self.is_cue_head(j) {
                        return None;
                    }
                }
                Lane::Notes => {}
            }
            if j == 0 {
                return None;
            }
            j -= 1;
        }
    }
}

/// `::question text` or `::summary text`: what the line opens and how long the marker is, with the spaces after it.
fn marker(text: &str) -> Option<(Lane, usize)> {
    let rest = text.strip_prefix("::")?;
    let word = rest.split(char::is_whitespace).next()?;
    let lane = match word {
        "question" | "cue" => Lane::Cue,
        "summary" | "resume" | "resumen" => Lane::Summary,
        _ => return None,
    };
    let after = &rest[word.len()..];
    Some((lane, 2 + word.len() + after.len() - after.trim_start().len()))
}
