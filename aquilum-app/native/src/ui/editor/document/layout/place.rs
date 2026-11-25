use std::ops::Range;

use masonry::core::{BrushIndex, StyleSet};
use masonry::parley::{FontContext, LayoutContext};

use super::super::super::highlight::Lang;
use super::super::super::markdown::{Block, Reveal};
use super::super::cornell::Lane;
use super::super::{Document, Fold};

impl Document {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn place(&mut self, from: usize, until: usize, fcx: &mut FontContext, lcx: &mut LayoutContext<BrushIndex>, styles: &StyleSet, width: Option<f32>, selection: Option<&Range<usize>>) {
        let all = !self.placed;
        let len = self.lines.count();
        let (mut i, mut until) = if all { (0, len) } else { (from.min(len.saturating_sub(1)), until.min(len)) };
        if self.is_cornell() && !all && until > i {
            i = self.row_start(i);
            until = self.row_end(until - 1) + 1;
        }
        while i > 0 && (self.heights.fold(i) == Fold::Member || self.inside(i) && matches!(self.blocks.get(i).1, Block::Code { first: false, lang: Some(Lang::Math), .. })) {
            i -= 1;
        }
        let mut top = self.top(i);
        let keep = self.keep;
        let put = |doc: &mut Document, k: usize, height: f64, fold: Fold| {
            if all {
                doc.heights.assign(k, height, fold);
            } else {
                doc.heights.set(k, height, fold);
            }
        };
        // A cue hangs in the left column and takes no room: the lines of its row must be as high as it is (cue height, row top).
        let mut row: Option<(f64, f64)> = None;
        while i < until {
            if self.is_cornell() && (matches!(self.lane(i), Lane::Full | Lane::Summary) || self.is_cue_head(i)) {
                Self::close_row(self, &mut row, &mut top, i, &put);
            }
            if self.is_cornell() && self.is_cue_head(i) {
                let end = self.cue_end(i);
                row = Some(((i..=end).map(|k| self.heights.text(k)).sum::<f64>() + self.cue_offset(i), top));
                for k in i..=end {
                    put(self, k, 0.0, Fold::None);
                }
                i = end + 1;
                continue;
            }
            if let Some((end, reveal)) = self.run_end(i) {
                let span = self.start(i)..self.base + self.lines.end(end);
                let open = reveal.opened(selection, &span) || reveal == Reveal::Above && self.source == Some(span.start);
                let held = self.pins.iter().any(|&pin| span.start <= pin && pin <= span.end);
                let known = match self.heights.fold(i) {
                    Fold::Head => Some(self.heights.get(i)),
                    Fold::Above => Some(self.heights.get(i) - self.heights.text(i)),
                    _ => None,
                };
                let guess = known.unwrap_or_else(|| (i..=end).map(|k| self.heights.text(k)).sum());
                let far = !held && (top + guess < keep.0 || top > keep.1);
                let built = if far {
                    self.embeds.remove(i);
                    Some(guess)
                } else {
                    (!(open && reveal == Reveal::Block)).then(|| self.embed(i, end, fcx, lcx, styles, width)).flatten()
                };
                if let Some(height) = built {
                    let beside = match reveal {
                        Reveal::Above if open => Some((i, Fold::Above)),
                        Reveal::Below if open => Some((end, Fold::Below)),
                        _ => None,
                    };
                    if let Some((at, fold)) = beside {
                        for k in i..at {
                            let text = self.heights.text(k);
                            put(self, k, text, Fold::None);
                            top += text;
                        }
                        let total = height + self.heights.text(at);
                        put(self, at, total, fold);
                        top += total;
                        i = at + 1;
                        continue;
                    }
                    put(self, i, height, Fold::Head);
                    for k in i + 1..=end {
                        put(self, k, 0.0, Fold::Member);
                    }
                    top += height;
                    i = end + 1;
                    continue;
                }
            }
            let height = self.heights.text(i);
            put(self, i, height, Fold::None);
            top += height;
            i += 1;
        }
        Self::close_row(self, &mut row, &mut top, i, &put);
        if all {
            self.heights.rebuild();
        }
        self.placed = true;
    }

    /// Gives the last line of a row the height its cue is taller by, so the next row starts below the cue.
    fn close_row(doc: &mut Document, row: &mut Option<(f64, f64)>, top: &mut f64, next: usize, put: &dyn Fn(&mut Document, usize, f64, Fold)) {
        let Some((cue, started)) = row.take() else { return };
        let missing = cue - (*top - started);
        let mut at = next.saturating_sub(1);
        while at > 0 && doc.heights.fold(at) == Fold::Member {
            at -= 1;
        }
        if missing > 0.5 && matches!(doc.heights.fold(at), Fold::None | Fold::Head) {
            let height = doc.heights.get(at) + missing;
            let fold = doc.heights.fold(at);
            put(doc, at, height, fold);
            *top += missing;
        }
    }
}
