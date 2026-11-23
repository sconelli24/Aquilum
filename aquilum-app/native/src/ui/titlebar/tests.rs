use super::*;

#[test]
fn the_close_button_sits_four_pixels_inside_the_hover_and_the_active_tab() {
    let bounds = Rect::new(0.0, 0.0, 160.0, size::TABS_HEIGHT);
    for active in [false, true] {
        let tab = TabButton::new(Uuid::nil(), "Заметка", active, true, None).widget;
        let close = tab.close_rect(bounds);
        let body = tab_body(bounds);
        let inner = if active { Rect::new(body.x0, body.y0 + size::BORDER_1, body.x1 - size::BORDER_1, body.y1) } else { hover(body) };
        assert_eq!((close.y0 - inner.y0, inner.x1 - close.x1), (size::PADDING_2XS, size::PADDING_2XS), "active: {active}");
        assert_eq!(close.center().y, content_y(bounds), "the close icon is not on the common line, active: {active}");
    }
    let pill = hover(tab_body(bounds));
    assert_eq!((pill.center().y, pill.height()), (content_y(bounds), size::TAB_CONTENT_HEIGHT + size::PADDING_2XS * 2.0), "the highlight is not around the text line");
    assert_eq!(size::TAB_RADIUS - size::PADDING_2XS, size::ROUNDED_LG, "the corners of the close icon and the tab are not concentric");
}
