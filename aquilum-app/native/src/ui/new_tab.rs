use masonry::core::{NewWidget, PropertySet, WidgetId};
use masonry::layout::Length;
use masonry::properties::Gap;
use masonry::properties::types::{CrossAxisAlignment, MainAxisAlignment};
use masonry::widgets::Flex;

use super::tokens::size;
use super::widgets::{TextButton, Variant};
use crate::i18n;

pub(super) struct NewTabView {
    pub widget: NewWidget<Flex>,
    pub create_note: WidgetId,
    pub create_canvas: WidgetId,
    pub open_file: WidgetId,
    pub close_tab: WidgetId,
}

pub(super) fn new_tab_view() -> NewTabView {
    let create = NewWidget::new(TextButton::new(i18n::t("editor.createNote"), Variant::Link));
    let canvas = NewWidget::new(TextButton::new(i18n::t("editor.createCanvas"), Variant::Link));
    let open = NewWidget::new(TextButton::new(i18n::t("editor.openFile"), Variant::Link));
    let close = NewWidget::new(TextButton::new(i18n::t("editor.close"), Variant::Link));
    let (create_id, canvas_id, open_id, close_id) = (create.id(), canvas.id(), open.id(), close.id());
    let column = Flex::column()
        .main_axis_alignment(MainAxisAlignment::Center)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .with_fixed(create)
        .with_fixed(canvas)
        .with_fixed(open)
        .with_fixed(close);
    let column = NewWidget::new(column).with_props(PropertySet::new().with(Gap::new(Length::px(size::SPACE_6))));
    NewTabView { widget: column, create_note: create_id, create_canvas: canvas_id, open_file: open_id, close_tab: close_id }
}
