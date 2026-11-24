//! The calendar of the sidebar: one month with a dot under the days that have a daily note, and,
//! optionally, how far through the year (in days and in weeks) today is.

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, ChildrenIds, CursorIcon, EventCtx, LayoutCtx, MeasureCtx, PaintCtx, PointerButton, PointerEvent, PropertiesMut, PropertiesRef, QueryCtx, RegisterCtx, Update, UpdateCtx,
    Widget, WidgetMut, render_text,
};
use masonry::imaging::Painter;
use masonry::kurbo::{Affine, Axis, Circle, Point, Rect, RoundedRect, Size};
use masonry::layout::{LenReq, Length};

use super::text::{Line, Style};
use super::tokens::{number, size};
use super::{icons, theme};
use crate::calendar::{self, Progress, WEEKS};
use crate::i18n::{self, t, t_with};

#[derive(Clone, Debug, PartialEq)]
pub enum CalendarAction {
    /// A day was clicked: open its daily note, creating it first when there is none.
    Day(NaiveDate),
    /// The month on show changed; the days with notes are those of the new one.
    Month { year: i32, month: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Previous,
    Next,
    /// The title: back to the current month.
    Title,
    Day(NaiveDate),
}

const PAD: f64 = size::SPACE_8;
const HEADER: f64 = 28.0;
const CELL: f64 = 26.0;
const BAR: f64 = 6.0;
const ARROW: f64 = 14.0;

pub struct CalendarPanel {
    year: i32,
    month: u32,
    today: NaiveDate,
    shown: bool,
    progress: bool,
    weeks: bool,
    marked: BTreeSet<u32>,
    hover: Option<Hit>,
    size: Size,
    labels: HashMap<&'static str, Line>,
}

/// What the settings and the files say about the calendar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalendarState {
    pub shown: bool,
    pub progress: bool,
    /// Also the progress of the year in weeks, under the days.
    pub weeks: bool,
    /// Days of the month on show that have a daily note.
    pub marked: BTreeSet<u32>,
}

impl CalendarPanel {
    /// Opens on the current month.
    pub fn new(state: CalendarState) -> Self {
        let today = calendar::today();
        CalendarPanel { year: today.year(), month: today.month(), today, shown: state.shown, progress: state.progress, weeks: state.weeks, marked: state.marked, hover: None, size: Size::ZERO, labels: HashMap::new() }
    }

    /// The month on show.
    pub fn month(&self) -> (i32, u32) {
        (self.year, self.month)
    }

    /// Centre of a day of the month on show, in the widget's coordinates.
    #[cfg(test)]
    pub fn day_center(&self, date: NaiveDate) -> Option<Point> {
        let grid = calendar::grid(self.year, self.month);
        (0..WEEKS).find_map(|row| (0..7).find_map(|column| (grid[row][column] == Some(date)).then(|| self.cell(row, column).center())))
    }

    /// Centre of the arrow to the previous or the next month, in the widget's coordinates.
    #[cfg(test)]
    pub fn arrow_center(&self, next: bool) -> Point {
        self.arrow(next).center()
    }

    #[cfg(test)]
    pub fn title_center(&self) -> Point {
        self.title_area().center()
    }

    pub fn set_state(this: &mut WidgetMut<'_, Self>, state: CalendarState, today: NaiveDate) {
        let widget = &mut *this.widget;
        if widget.shown != state.shown || widget.progress != state.progress || widget.weeks != state.weeks {
            this.ctx.request_layout();
        }
        widget.shown = state.shown;
        widget.progress = state.progress;
        widget.weeks = state.weeks;
        widget.today = today;
        widget.marked = state.marked;
        this.ctx.request_paint_only();
    }

    fn line() -> f64 {
        (f64::from(theme::ui_font_settings().size) * number::FONT_LINE_HEIGHT_NORMAL).round()
    }

    /// Bars shown: the days, and the weeks too when asked.
    fn bars(&self) -> f64 {
        if self.weeks { 2.0 } else { 1.0 }
    }

    fn progress_height(&self) -> f64 {
        // A rule, then a row of a label and a bar for each.
        PAD + self.bars() * (Self::line() + 4.0 + BAR + PAD)
    }

    fn height(&self) -> f64 {
        if !self.shown {
            return 0.0;
        }
        let month = PAD + HEADER + CELL + CELL * WEEKS as f64 + PAD;
        month + if self.progress { self.progress_height() } else { 0.0 }
    }

    fn header(&self) -> Rect {
        Rect::new(PAD, PAD, self.size.width - PAD, PAD + HEADER)
    }

    fn arrow(&self, next: bool) -> Rect {
        let header = self.header();
        let x = if next { header.x1 - HEADER } else { header.x1 - HEADER * 2.0 };
        Rect::new(x, header.y0, x + HEADER, header.y1)
    }

    fn title_area(&self) -> Rect {
        Rect::new(self.header().x0, self.header().y0, self.arrow(false).x0, self.header().y1)
    }

    fn column(&self, index: usize) -> Rect {
        let width = (self.size.width - PAD * 2.0) / 7.0;
        let y = PAD + HEADER;
        Rect::new(PAD + width * index as f64, y, PAD + width * (index + 1) as f64, y + CELL)
    }

    fn cell(&self, row: usize, column: usize) -> Rect {
        let top = PAD + HEADER + CELL * (row + 1) as f64;
        let area = self.column(column);
        Rect::new(area.x0, top, area.x1, top + CELL)
    }

    fn hit(&self, at: Point) -> Option<Hit> {
        if !self.shown {
            return None;
        }
        if self.arrow(false).contains(at) {
            return Some(Hit::Previous);
        }
        if self.arrow(true).contains(at) {
            return Some(Hit::Next);
        }
        if self.title_area().contains(at) {
            return Some(Hit::Title);
        }
        let grid = calendar::grid(self.year, self.month);
        (0..WEEKS).find_map(|row| (0..7).find_map(|column| grid[row][column].filter(|_| self.cell(row, column).contains(at)).map(Hit::Day)))
    }

    fn go_to(&mut self, ctx: &mut EventCtx<'_>, year: i32, month: u32) {
        if (year, month) == (self.year, self.month) {
            return;
        }
        (self.year, self.month) = (year, month);
        self.marked.clear();
        ctx.request_paint_only();
        ctx.submit_action::<CalendarAction>(CalendarAction::Month { year, month });
    }

    fn text(&mut self, ctx: &mut PaintCtx<'_>, painter: &mut Painter<'_>, key: &'static str, text: &str, area: Rect, align: Align, color: masonry::peniko::Color) {
        if text.is_empty() {
            return;
        }
        let height = Self::line();
        let layout = self.labels.entry(key).or_default().layout(ctx, text, area.width().max(1.0), Style::ui(height));
        let width = f64::from(layout.width());
        let x = match align {
            Align::Start => area.x0,
            Align::Center => area.x0 + (area.width() - width) / 2.0,
            Align::End => area.x1 - width,
        };
        let y = area.y0 + (area.height() - height) / 2.0;
        render_text(painter, Affine::translate((x, y)), layout, &[color.into()], true);
    }
}

#[derive(Clone, Copy)]
enum Align {
    Start,
    Center,
    End,
}

impl Widget for CalendarPanel {
    type Action = CalendarAction;

    fn on_pointer_event(&mut self, ctx: &mut EventCtx<'_>, _props: &mut PropertiesMut<'_>, event: &PointerEvent) {
        if !self.shown {
            return;
        }
        match event {
            PointerEvent::Move(update) => {
                let hit = self.hit(ctx.local_position(update.current.position));
                if hit != self.hover {
                    self.hover = hit;
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Leave(_) => {
                if self.hover.take().is_some() {
                    ctx.request_paint_only();
                }
            }
            PointerEvent::Down(down) if down.button.is_none() || down.button == Some(PointerButton::Primary) => {
                let Some(hit) = self.hit(ctx.local_position(down.state.position)) else { return };
                match hit {
                    Hit::Previous => {
                        let (year, month) = calendar::shifted(self.year, self.month, -1);
                        self.go_to(ctx, year, month);
                    }
                    Hit::Next => {
                        let (year, month) = calendar::shifted(self.year, self.month, 1);
                        self.go_to(ctx, year, month);
                    }
                    Hit::Title => {
                        let today = calendar::today();
                        self.go_to(ctx, today.year(), today.month());
                    }
                    Hit::Day(date) => ctx.submit_action::<CalendarAction>(CalendarAction::Day(date)),
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if let Update::FontsChanged = event {
            // Line heights and every laid-out text follow the font.
            self.labels.clear();
            ctx.request_layout();
            ctx.request_paint_only();
        }
    }

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn measure(&mut self, ctx: &mut MeasureCtx<'_>, _props: &PropertiesRef<'_>, axis: Axis, _len_req: LenReq, _cross: Option<Length>) -> Length {
        match axis {
            Axis::Horizontal => ctx.context_size().length(axis).unwrap_or(Length::ZERO),
            Axis::Vertical => Length::px(self.height()),
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _props: &PropertiesRef<'_>, size: Size) {
        self.size = size;
        ctx.set_clip_path(size.to_rect());
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, painter: &mut Painter<'_>) {
        if !self.shown || self.size.width <= 0.0 {
            return;
        }
        let tm = theme::current();
        let language = i18n::language();
        let hover = tm.border_default.multiply_alpha(0.5);
        painter.fill(Rect::new(0.0, 0.0, self.size.width, self.size.height), tm.calendar_bg).draw();
        painter.fill(Rect::new(0.0, 0.0, self.size.width, 1.0), tm.border_default).draw();

        // Title and arrows.
        let title = format!("{} {}", calendar::month_name(language, self.month), self.year);
        let title_area = self.title_area();
        if self.hover == Some(Hit::Title) {
            painter.fill(RoundedRect::from_rect(title_area, size::ROUNDED_SM), hover).draw();
        }
        self.text(ctx, painter, "title", &title, Rect::new(title_area.x0 + size::SPACE_6, title_area.y0, title_area.x1, title_area.y1), Align::Start, tm.text_primary);
        for (next, hit) in [(false, Hit::Previous), (true, Hit::Next)] {
            let area = self.arrow(next);
            if self.hover == Some(hit) {
                painter.fill(RoundedRect::from_rect(area, size::ROUNDED_SM), hover).draw();
            }
            let icon = if next { icons::CHEVRON_RIGHT } else { icons::CHEVRON_LEFT };
            icon.draw(painter, Point::new(area.x0 + (area.width() - ARROW) / 2.0, area.y0 + (area.height() - ARROW) / 2.0), ARROW, 2.0, tm.text_secondary);
        }

        // Weekday initials.
        for (column, initial) in calendar::weekdays(language).into_iter().enumerate() {
            let area = self.column(column);
            self.text(ctx, painter, WEEKDAY_KEYS[column], initial, area, Align::Center, tm.text_muted);
        }

        // Days.
        let grid = calendar::grid(self.year, self.month);
        for row in 0..WEEKS {
            for column in 0..7 {
                let Some(date) = grid[row][column] else { continue };
                let area = self.cell(row, column);
                let center = area.center();
                let is_today = date == self.today;
                if self.hover == Some(Hit::Day(date)) {
                    painter.fill(Circle::new(center, CELL / 2.0 - 1.0), hover).draw();
                }
                if is_today {
                    painter.fill(Circle::new(center, CELL / 2.0 - 1.0), tm.calendar_accent.multiply_alpha(0.25)).draw();
                }
                let key = DAY_KEYS[date.day0() as usize];
                let text_color = if is_today { tm.text_primary } else { tm.text_secondary };
                let number = date.day().to_string();
                self.text(ctx, painter, key, &number, Rect::new(area.x0, area.y0 - 2.0, area.x1, area.y1 - 2.0), Align::Center, text_color);
                if self.marked.contains(&date.day()) {
                    painter.fill(Circle::new(Point::new(center.x, area.y1 - 4.0), 1.75), tm.calendar_accent).draw();
                }
            }
        }

        // Progress of the year.
        if self.progress {
            let progress = Progress::of(self.today);
            let top = PAD + HEADER + CELL + CELL * WEEKS as f64 + PAD;
            painter.fill(Rect::new(PAD, top, self.size.width - PAD, top + 1.0), tm.border_default).draw();
            let mut rows = vec![("progress.days", t_with("calendar.days", &[("done", &progress.day.to_string()), ("total", &progress.days.to_string())]), progress.day_fraction())];
            if self.weeks {
                rows.push(("progress.weeks", t_with("calendar.weeks", &[("done", &progress.week.to_string()), ("total", &progress.weeks.to_string())]), progress.week_fraction()));
            }
            let line = Self::line();
            let mut y = top + PAD;
            for (key, label, fraction) in rows {
                let width = self.size.width - PAD * 2.0;
                self.text(ctx, painter, key, &label, Rect::new(PAD, y, PAD + width * 0.7, y + line), Align::Start, tm.text_secondary);
                let percent = format!("{}%", (fraction * 100.0).round() as u32);
                self.text(ctx, painter, if key == "progress.days" { "percent.days" } else { "percent.weeks" }, &percent, Rect::new(PAD + width * 0.7, y, PAD + width, y + line), Align::End, tm.text_muted);
                let track = Rect::new(PAD, y + line + 4.0, PAD + width, y + line + 4.0 + BAR);
                painter.fill(RoundedRect::from_rect(track, BAR / 2.0), tm.border_default.multiply_alpha(0.6)).draw();
                let fill = Rect::new(track.x0, track.y0, track.x0 + (track.width() * fraction.clamp(0.0, 1.0)).max(BAR), track.y1);
                painter.fill(RoundedRect::from_rect(fill, BAR / 2.0), tm.border_accent).draw();
                y += line + 4.0 + BAR + PAD;
            }
        }
    }

    fn get_cursor(&self, _ctx: &QueryCtx<'_>, pos: Point) -> CursorIcon {
        if self.hit(pos).is_some() { CursorIcon::Pointer } else { CursorIcon::Default }
    }

    fn accessibility_role(&self) -> Role {
        Role::Group
    }

    fn accessibility(&mut self, _ctx: &mut AccessCtx<'_>, _props: &PropertiesRef<'_>, node: &mut Node) {
        node.set_label(t("calendar.title"));
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Cache keys of the text of each weekday and day, so a layout is kept per cell.
const WEEKDAY_KEYS: [&str; 7] = ["wd0", "wd1", "wd2", "wd3", "wd4", "wd5", "wd6"];
const DAY_KEYS: [&str; 31] = [
    "d1", "d2", "d3", "d4", "d5", "d6", "d7", "d8", "d9", "d10", "d11", "d12", "d13", "d14", "d15", "d16", "d17", "d18", "d19", "d20", "d21", "d22", "d23", "d24", "d25", "d26", "d27", "d28",
    "d29", "d30", "d31",
];
