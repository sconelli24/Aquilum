//! Cornell notes: a note whose properties say `type: cornell` keeps its notes in a column at the right, leaving room at the left
//! for the cues (the `[!question]` callouts) and takes the whole width again for the closing `[!summary]`.

use super::super::markdown::{Block, Callout};
use super::Document;
use crate::ui::editor::embed::panel;

/// Narrower windows keep the usual single column.
const MIN_WIDTH: f32 = 700.0;
const GUTTER: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lane {
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

    pub(super) fn is_cornell(&self) -> bool {
        (if self.front > 0 { self.cornell } else { self.told }) && self.width.is_some_and(|width| width >= MIN_WIDTH)
    }

    /// Width of the left column, without the gutter.
    pub(super) fn cue_width(&self) -> f32 {
        self.width.map_or(0.0, |width| (width * 0.27).clamp(180.0, 280.0))
    }

    pub(super) fn lane(&self, line: usize) -> Lane {
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
            // Until cues have their own column they keep the whole width.
            Lane::Full | Lane::Cue => (0.0, self.width),
        }
    }
}
