//! A note shown inside another note: the same Markdown editor, read-only, drawn once into a recorded scene.

use std::sync::Arc;

use masonry::core::render_text;
use masonry::imaging::Painter;
use masonry::imaging::record::Scene;
use masonry::kurbo::Affine;
use masonry::parley::{FontContext, LayoutContext};
use masonry::peniko::Brush;

use super::*;
use crate::ui::editor::resolve::Resolver;

impl TextEditor {
    /// Lays out `text` as a Markdown note `width` wide and draws it with the origin at its top left corner.
    ///
    /// Returns the drawing and its height. The editor lives only for this call: the host keeps the recorded scene.
    pub fn render_nested(text: crop::Rope, resolver: Option<Arc<dyn Resolver>>, smart: Smart, width: f64, fcx: &mut FontContext, lcx: &mut LayoutContext<BrushIndex>) -> (Scene, f64) {
        let mut editor = TextEditor::entire(text).printed().styled(crate::ui::style::editor_text).markdown().with_resolver(resolver).with_smart(smart);
        editor.ensure_layout(fcx, lcx, width);
        let last = editor.doc.line_count().saturating_sub(1);
        editor.doc.ensure(0..=last, fcx, lcx, &editor.styles, None);
        let height = editor.doc.height();
        let tm = theme::current();
        let palette = markdown::palette(&tm, tm.editor_text);
        let mut scene = Scene::new();
        {
            let mut recorder = Painter::new(&mut scene);
            editor.paint_nested(&mut recorder.as_dyn(), last, &palette);
        }
        (scene, height)
    }

    /// What `do_paint` draws for a note that is only read: blocks, backdrops, embedded cards, marks and text.
    fn paint_nested(&mut self, painter: &mut Painter<'_>, last: usize, palette: &[Brush]) {
        let texts: Vec<usize> = (0..=last)
            .filter(|&i| {
                let p = self.doc.paragraph(i);
                p.laid().is_some() && !matches!((p.fold, &p.embed), (Fold::Member, _) | (Fold::Head, Some(_)))
            })
            .collect();
        self.clips = self.paint_blocks(painter, 0, last, palette);
        for &i in &texts {
            self.paint_backdrop(painter, i);
        }
        for (i, top, _) in self.doc.placed(0..=last) {
            let p = self.doc.paragraph(i);
            let (Fold::Head | Fold::Above, Some(embed)) = (p.fold, &p.embed) else { continue };
            let overlay = super::super::table::Overlay { print: true, scroll_x: 0.0, ..Default::default() };
            let media = self.media_overlay(p.start);
            if media.video.is_none() {
                embed.view.paint(painter, self.embed_x(i), top, palette, &overlay, &media, None);
            }
        }
        for i in texts {
            self.paint_marks(painter, i, palette);
            self.clipped(painter, i, |painter| render_text(painter, Affine::translate(self.shift() + self.doc.origin(i)), self.doc.paragraph(i).layout(), palette, true));
        }
    }
}
