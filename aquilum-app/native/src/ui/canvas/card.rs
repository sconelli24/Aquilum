//! One card on the board. Its content (a Markdown editor, an image) is a real child widget laid
//! out at the card's world size; the board scales it with a transform.

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, LayoutCtx, MeasureCtx, NewWidget, PaintCtx, PropertiesRef, QueryCtx, RegisterCtx, Widget, WidgetMut, WidgetPod, WidgetRef,
    find_widget_under_pointer,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Axis, Point, RoundedRect, Size, Stroke};
use masonry::layout::{LenReq, Length};
use masonry::peniko::Color;

use crate::ui::theme;

pub const RADIUS: f64 = 8.0;
pub const BORDER: f64 = 2.0;
/// Inner margin between the frame and the content, in world units.
pub const INSET: f64 = 4.0;

pub struct Card {
    content: Option<WidgetPod<dyn Widget>>,
    /// While editing, pointer input reaches the content; otherwise the whole card is one target
    /// so the board can select and drag it.
    editing: bool,
    color: Option<Color>,
}

impl Card {
    pub fn new(content: Option<NewWidget<dyn Widget>>, color: Option<Color>) -> NewWidget<Self> {
        NewWidget::new(Card { content: content.map(NewWidget::to_pod), editing: false, color })
    }

    /// A frame whose content always takes the pointer, for in-place prompts.
    pub fn field(content: NewWidget<dyn Widget>) -> NewWidget<Self> {
        NewWidget::new(Card { content: Some(content.to_pod()), editing: true, color: None })
    }

    pub fn set_editing(this: &mut WidgetMut<'_, Self>, editing: bool) {
        if this.widget.editing != editing {
            this.widget.editing = editing;
            this.ctx.request_paint_only();
        }
    }

    pub fn set_color(this: &mut WidgetMut<'_, Self>, color: Option<Color>) {
        if this.widget.color != color {
            this.widget.color = color;
            this.ctx.request_paint_only();
        }
    }
}

impl Widget for Card {
    type Action = masonry::core::NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        if let Some(content) = &mut self.content {
            ctx.register_child(content);
        }
    }

    fn measure(&mut self, ctx: &mut MeasureCtx<'_>, _props: &PropertiesRef<'_>, axis: Axis, _len_req: LenReq, _cross: Option<Length>) -> Length {
        ctx.context_size().length(axis).unwrap_or(Length::ZERO)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        ctx.set_clip_path(size.to_rect());
        if let Some(content) = &mut self.content {
            let inner = Size::new((size.width - INSET * 2.0).max(1.0), (size.height - INSET * 2.0).max(1.0));
            ctx.run_layout(content, inner);
            ctx.place_child(content, Point::new(INSET, INSET));
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        let tm = theme::current();
        let shape = RoundedRect::from_rect(ctx.content_box(), RADIUS);
        painter.fill(shape, tm.bg_surface).draw();
        if let Some(color) = self.color {
            painter.fill(shape, color.multiply_alpha(0.08)).draw();
        }
        let border = self.color.unwrap_or(tm.border_default);
        let inner = RoundedRect::from_rect(ctx.content_box().inset(-BORDER / 2.0), RADIUS - BORDER / 2.0);
        painter.stroke(inner, &Stroke::new(BORDER), border).draw();
    }

    fn find_widget_under_pointer<'c>(&'c self, ctx: QueryCtx<'c>, pos: Point) -> Option<WidgetRef<'c, dyn Widget>> {
        if self.editing {
            return find_widget_under_pointer(self, ctx, pos);
        }
        let inside = !ctx.is_stashed() && ctx.bounding_box().contains(pos) && ctx.border_box().contains(ctx.to_local(pos));
        inside.then(|| WidgetRef::new(self as &dyn Widget, ctx))
    }

    fn accessibility_role(&self) -> Role {
        Role::Group
    }

    fn accessibility(&mut self, _ctx: &mut AccessCtx<'_>, _props: &PropertiesRef<'_>, _node: &mut Node) {}

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&self.content.iter().map(WidgetPod::id).collect::<Vec<_>>())
    }
}
