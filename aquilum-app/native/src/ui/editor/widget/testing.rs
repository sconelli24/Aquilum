use super::*;

impl TextEditor {
    pub fn selected(&self) -> String {
        self.selected_text().unwrap_or_default()
    }

    pub fn table_marks(&self) -> Option<(bool, usize)> {
        self.table.as_ref().map(|s| (s.range.is_some(), s.caret.abs_diff(s.anchor)))
    }

    pub fn grips_at(&self) -> [Option<Point>; 2] {
        self.grips.at
    }

    pub fn image_source_shown(&self) -> bool {
        self.doc.source.is_some()
    }

    pub fn inline_boxes(&self, line: usize) -> usize {
        self.doc.paragraph(line).decor().boxes.len()
    }

    pub fn brush_at(&self, offset: usize) -> Option<BrushIndex> {
        let (_, local, layout) = self.local(offset);
        layout?.lines().flat_map(|line| line.items().collect::<Vec<_>>()).find_map(|item| match item {
            masonry::parley::PositionedLayoutItem::GlyphRun(run) if run.run().text_range().contains(&local) => Some(run.style().brush.clone()),
            _ => None,
        })
    }
}

impl TextEditor {
    /// Height and click targets of every card drawn in place of one or more lines (dataview, books, notes shown inside the note).
    pub fn cards(&self) -> Vec<(f64, Vec<String>)> {
        self.card_places().into_iter().map(|(_, height, links)| (height, links)).collect()
    }

    /// Where each card starts (x, y in the editor) with its height and click targets.
    pub fn card_places(&self) -> Vec<(Point, f64, Vec<String>)> {
        use crate::ui::editor::embed::{Action, View};
        self.doc
            .paragraphs()
            .enumerate()
            .filter(|(_, p)| matches!(p.fold, Fold::Head | Fold::Below | Fold::Above))
            .filter_map(|(i, p)| match p.embed.map(|e| &e.view) {
                Some(View::Drawing(drawing)) => Some((Point::new(self.embed_x(i), self.doc.top(i)), drawing.height, {
                    let mut targets: Vec<String> = drawing.hits.iter().filter_map(|(_, action, _)| if let Action::Link { target } = action { Some(target.clone()) } else { None }).collect();
                    targets.dedup();
                    targets
                })),
                _ => None,
            })
            .collect()
    }

    /// The click targets of the first card (rectangles in the card's own coordinates) with where they lead, and the card's width.
    pub fn card_hits(&self) -> Option<(Vec<(Rect, String)>, f64)> {
        use crate::ui::editor::embed::{Action, View};
        self.doc.paragraphs().filter(|p| matches!(p.fold, Fold::Head | Fold::Below | Fold::Above)).find_map(|p| match p.embed.map(|e| &e.view) {
            Some(View::Drawing(drawing)) => Some((drawing.hits.iter().filter_map(|(rect, action, _)| if let Action::Link { target } = action { Some((*rect, target.clone())) } else { None }).collect(), drawing.width())),
            _ => None,
        })
    }

    /// Height of the paragraph that holds a card (the line of its link too when it is shown).
    pub fn card_paragraph_height(&self) -> Option<f64> {
        self.doc.paragraphs().enumerate().find(|(_, p)| matches!(p.fold, Fold::Head | Fold::Below | Fold::Above) && p.embed.is_some()).map(|(i, _)| self.doc.bottom(i) - self.doc.top(i))
    }

    /// How many cards show the text of their link (the caret is on it) instead of only the card.
    pub fn cards_with_source(&self) -> usize {
        self.doc.paragraphs().filter(|p| matches!(p.fold, Fold::Below | Fold::Above) && p.embed.is_some()).count()
    }

    /// What the suggestion list shows: the items and their tags.
    pub fn suggestions(&self) -> Option<(Vec<String>, Vec<String>)> {
        self.suggest_shown.as_ref().map(|shown| (shown.items.clone(), shown.hints.clone()))
    }

    pub fn card_scrollbar(&self) -> Option<crate::ui::scroll::bar::Span> {
        self.doc.paragraphs().filter(|p| p.fold == Fold::Head).find_map(|p| self.block_span(p.start))
    }
}
