//! Cornell notes: a note whose properties say `type: cornell` keeps its notes in a column at the right, leaving room at the left
//! for the cues (the `[!question]` callouts) and takes the whole width again for the closing `[!summary]`.

use super::super::markdown::{Block, Callout};
use super::Document;
use crate::ui::editor::embed::panel;

/// Narrower windows keep the usual single column.
const MIN_WIDTH: f32 = 700.0;
const GUTTER: f32 = 20.0;
/// Air between the line that closes a row and the cue of the next one.
const ROW_AIR: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Properties, headings, the summary: the whole width.
    Full,
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

    pub fn is_cornell(&self) -> bool {
        (if self.front > 0 { self.cornell } else { self.told }) && self.width.is_some_and(|width| width >= MIN_WIDTH)
    }

    /// Width of the left column, without the gutter.
    pub fn cue_width(&self) -> f32 {
        self.width.map_or(0.0, |width| (width * 0.27).clamp(180.0, 280.0))
    }

    pub fn lane(&self, line: usize) -> Lane {
        if !self.is_cornell() || self.start(line) < self.front {
            return Lane::Full;
        }
        let quote = |j: usize| self.frame(j).quote;
        if quote(line) == 0 {
            return match self.block(line) {
                Block::Heading { .. } => Lane::Full,
                _ => Lane::Notes,
            };
        }
        let mut head = line;
        while !matches!(self.block(head), Block::Callout { .. }) {
            if head == 0 || quote(head - 1) == 0 {
                return Lane::Notes;
            }
            head -= 1;
        }
        match self.block(head) {
            Block::Callout { kind: Callout::Typed(kind), .. } if quote(head) == 1 => match *kind {
                kind if kind == panel::kind("summary") => Lane::Full,
                kind if kind == panel::kind("question") => Lane::Cue,
                _ => Lane::Notes,
            },
            _ => Lane::Notes,
        }
    }

    /// How far a line is moved right to make room for the cues.
    pub fn lane_x(&self, line: usize) -> f64 {
        self.lane_box(line).0
    }

    /// Where the text of a line starts and how wide it may be.
    pub(super) fn lane_box(&self, line: usize) -> (f64, Option<f32>) {
        match self.lane(line) {
            Lane::Notes => {
                let room = self.cue_width() + GUTTER;
                (f64::from(room), self.width.map(|width| (width - room).max(0.0)))
            }
            Lane::Cue => (0.0, Some(self.cue_width())),
            Lane::Full => (0.0, self.width),
        }
    }
}

/// Cues take no room in the flow of the page: they hang in the left column beside their notes. These are the rows that make
/// that work: a row is a cue and the lines up to the next cue or the next line that takes the whole width.
impl Document {
    pub(super) fn is_cue_head(&self, line: usize) -> bool {
        self.lane(line) == Lane::Cue && matches!(self.block(line), Block::Callout { .. })
    }

    /// The first line of the row `line` is in, or `line` itself when no cue opens it.
    pub(super) fn row_start(&self, line: usize) -> usize {
        if !self.is_cornell() {
            return line;
        }
        let mut j = line;
        loop {
            match self.lane(j) {
                Lane::Full => return line,
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
        while self.is_cornell() && j + 1 < self.lines.count() && self.lane(j + 1) != Lane::Full && !self.is_cue_head(j + 1) {
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

    /// Air under the line that closes the row above, and the blank lines between the cue and the first text of its notes.
    pub(super) fn cue_offset(&self, head: usize) -> f64 {
        let mut offset = ROW_AIR;
        let mut k = self.cue_end(head) + 1;
        while k < self.lines.count() && self.lane(k) == Lane::Notes && self.line(k).trim().is_empty() {
            offset += self.heights.text(k);
            k += 1;
        }
        offset
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

    /// Where the line between the columns is, from the left edge of the text.
    pub fn divider_x(&self) -> f64 {
        f64::from(self.cue_width() + GUTTER / 2.0)
    }

    /// Where a cue panel starts: its row top, and the offset that puts it level with the first text of its notes.
    pub fn cue_top(&self, head: usize) -> f64 {
        self.top(head) + self.cue_offset(head)
    }

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
                Lane::Full => return None,
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
