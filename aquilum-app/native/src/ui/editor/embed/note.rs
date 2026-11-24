//! The card of a note shown inside a note: a title that opens it, and the text below.

use std::sync::Arc;

use masonry::core::BrushIndex;
use masonry::imaging::record::Scene;
use masonry::kurbo::{Point, Rect};
use masonry::parley::Layout;

use super::drawing::{Drawing, Item, Tone, link_hits};

const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;

pub enum Body {
    /// The note's text, drawn `height` tall.
    Content(Arc<Scene>, f64),
    /// A line instead of the text: loading, a loop, too many levels.
    Notice(Layout<BrushIndex>),
}

/// `title` is laid out already and links to `link`; `width` is the room the card has.
pub fn note(width: f64, title: Layout<BrushIndex>, title_text: &str, link: &str, body: Body) -> Drawing {
    let header = f64::from(title.height()) + PAD_Y * 2.0;
    let top = Point::new(PAD_X, PAD_Y);
    let hits = link_hits(&title, top, &[(0..title_text.len(), link.to_owned())]);
    let mut items = vec![Item::Text(top, title, title_text.to_owned()), Item::Fill(Rect::new(0.0, header, width, header + 1.0).to_rounded_rect(0.0), Tone::Divider)];
    let at = Point::new(PAD_X, header + PAD_Y);
    let below = match body {
        Body::Content(scene, height) => {
            items.push(Item::Scene(at, scene));
            height
        }
        Body::Notice(layout) => {
            let height = f64::from(layout.height());
            items.push(Item::Text(at, layout, String::new()));
            height
        }
    };
    let mut card = Drawing::card(width, header + PAD_Y + below + PAD_Y, items);
    card.hits = hits;
    card
}
