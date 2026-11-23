use masonry::core::WidgetId;
use masonry::kurbo::Size;

use super::super::adaptive::{at_width, bounds, hidden};
use super::*;
use crate::app::MenuCommand;
use crate::ui::book::{CoverAction, HeroIds};
use crate::ui::menu::MenuChoice;
use crate::ui::tokens::size;
use crate::ui::widgets::Pressed;

const BOOK: &str = "---\ncover: true\ntype: book\nPage_cover: pattern:polka\nBook_cover: wide.png\n---\nтекст";
const PHOTO: &str = "---\ncover: true\nPage_cover: photo.png\n---\nтекст";

fn open(width: f64, text: &str) -> Harness {
    let mut h = at_width(width);
    image::RgbaImage::from_pixel(300, 200, image::Rgba([200, 60, 60, 255])).save(h.vault.path().join("wide.png")).unwrap();
    image::RgbaImage::from_pixel(1200, 600, image::Rgba([60, 120, 200, 255])).save(h.vault.path().join("photo.png")).unwrap();
    let note = h.write_note("Книга.md", text);
    h.open(note);
    h.settle();
    h
}

fn ids(h: &Harness) -> &HeroIds {
    &h.app.note_ids.as_ref().expect("note open").hero.1
}

fn button(h: &Harness, wanted: CoverAction) -> WidgetId {
    ids(h).actions.iter().find(|&&(_, action)| action == wanted).map(|&(id, _)| id).expect("action button")
}

fn hover_cover(h: &mut Harness) {
    let center = bounds(h, ids(h).cover.unwrap().id()).center();
    h.pointer_move(center.x, center.y);
    h.settle();
}

fn shot(h: &mut Harness, name: &str, pattern: bool) {
    if std::env::var_os("AQ_OUT").is_some() {
        h.settle();
        h.wait_cover(pattern);
        dump(name, &h.frame(h.app.window.width as u16, H as u16, 1.0));
    }
}

fn book_sizes(width: f64) -> (f64, Size) {
    let mut h = open(width, BOOK);
    shot(&mut h, &format!("cover-book-{width}.png"), true);
    let ids = ids(&h);
    assert_eq!(ids.replace_book.is_some(), width > 600.0, "upload button on the book cover only on desktop; on a phone it is an ellipsis menu item");
    (bounds(&h, ids.cover.unwrap().id()).height(), bounds(&h, ids.thumbnail.unwrap().id()).size())
}

#[test]
fn narrow_screen_lowers_covers_and_keeps_the_book_proportions() {
    let (wide_cover, wide_book) = book_sizes(1000.0);
    let (narrow_cover, narrow_book) = book_sizes(360.0);
    assert_eq!(narrow_cover, wide_cover - size::BOOK_COVER_COMPACT_SHRINK, "page cover is 40 px lower");
    assert_eq!(narrow_book.height, size::BOOK_THUMB_HEIGHT_COMPACT, "book cover has constant height");
    assert!(narrow_book.height < wide_book.height, "{narrow_book:?} is not smaller than {wide_book:?}");
    for book in [wide_book, narrow_book] {
        assert_eq!(book.width, (book.height * 1.5).round(), "width follows the 300x200 image proportions: {book:?}");
    }
}

#[test]
fn cover_controls_sit_as_far_from_the_top_as_from_the_right() {
    let h = open(1000.0, BOOK);
    let cover = bounds(&h, ids(&h).cover.unwrap().id());
    let control = bounds(&h, ids(&h).book_button.expect("book button on the cover").id()).union(bounds(&h, ids(&h).book_menu.expect("book file menu")));
    let (top, right) = (control.y0 - cover.y0, cover.x1 - control.x1);
    assert!((top - size::BOOK_CONTROLS_INSET).abs() < 0.5 && (right - size::BOOK_CONTROLS_INSET).abs() < 0.5, "button insets: top {top}, right {right}");
}

#[test]
fn without_a_page_cover_the_book_starts_at_the_gutter() {
    let h = open(1000.0, "---\ntype: book\nBook_cover: wide.png\n---\nтекст");
    let hero = bounds(&h, h.app.note_ids.as_ref().unwrap().hero.0.id());
    let book = bounds(&h, ids(&h).thumbnail.expect("book cover").id());
    assert!((book.y0 - hero.y0 - size::BOOK_GUTTER).abs() < 0.5, "above the book cover {} instead of {}", book.y0 - hero.y0, size::BOOK_GUTTER);
}

#[test]
fn narrow_cover_keeps_its_actions_in_one_menu() {
    let mut h = open(360.0, PHOTO);
    let more = ids(&h).more.expect("ellipsis button on a narrow screen");
    hover_cover(&mut h);
    assert!(!hidden(&h, more) && hidden(&h, button(&h, CoverAction::Replace)), "hover does not open the panel");
    h.act(more, Pressed);
    let Some(Popup::Menu { layer, commands }) = &h.app.popup else { panic!("cover menu not open") };
    let expected: Vec<MenuCommand> = CoverAction::list(Some(true), false).into_iter().map(MenuCommand::Cover).collect();
    assert!(*commands == expected, "menu has the same actions as the panel");
    let (layer, reposition) = (*layer, expected.iter().position(|&c| c == MenuCommand::Cover(CoverAction::Reposition)).unwrap());
    let (menu, trigger) = (bounds(&h, layer), bounds(&h, more));
    assert!(menu.x0 >= 0.0 && menu.x1 <= 360.0 && menu.y0 >= trigger.y1, "menu is below the button and inside the window: {menu:?}");
    shot(&mut h, "cover-menu-360.png", false);
    h.act(layer, MenuChoice::Item(reposition));
    let done = button(&h, CoverAction::Reposition);
    assert!(hidden(&h, more) && !hidden(&h, done), "while the cover is being moved, the Done button is visible");
    shot(&mut h, "cover-reposition-360.png", false);
    h.act(done, Pressed);
    assert!(!hidden(&h, more) && hidden(&h, done), "after Done there is a single ellipsis button again");
    h.act(more, Pressed);
    let Some(Popup::Menu { layer, .. }) = &h.app.popup else { panic!("cover menu not open") };
    let (layer, random) = (*layer, expected.iter().position(|&c| c == MenuCommand::Cover(CoverAction::Random)).unwrap());
    h.act(layer, MenuChoice::Item(random));
    assert!(h.app.text.to_string().contains("Page_cover: pattern:"), "menu item does the same as the button");
}

fn menu(h: &mut Harness) -> (WidgetId, Vec<CoverAction>) {
    let more = ids(h).more.expect("ellipsis button in the header");
    assert!(!hidden(h, more), "ellipsis is visible without hover");
    h.act(more, Pressed);
    let Some(Popup::Menu { layer, commands }) = &h.app.popup else { panic!("cover menu not open") };
    (*layer, commands.iter().filter_map(|c| if let MenuCommand::Cover(action) = c { Some(*action) } else { None }).collect())
}

fn note_menu(h: &mut Harness) -> (WidgetId, Vec<CoverAction>) {
    h.act(h.app.note_ids.as_ref().unwrap().more, Pressed);
    let Some(Popup::Menu { layer, commands }) = &h.app.popup else { panic!("note menu not open") };
    let frame = bounds(h, *layer);
    assert!(frame.x0 >= size::SPACE_8 && frame.x1 <= h.app.window.width - size::SPACE_8 + 0.5, "ellipsis menu went off the screen edge: {frame:?}");
    let covers: Vec<CoverAction> = commands.iter().map_while(|c| if let MenuCommand::Cover(action) = c { Some(*action) } else { None }).collect();
    let mut list = h.root.get_widget(*layer).unwrap();
    while list.children().len() == 1 {
        list = list.children()[0];
    }
    let rule = list.children().get(covers.len()).is_some_and(|child| child.downcast::<crate::ui::widgets::Rule>().is_some());
    assert!(covers.is_empty() || rule, "cover items are not separated by a divider");
    (*layer, covers)
}

#[test]
fn without_a_cover_its_actions_move_to_the_note_menu() {
    use CoverAction::*;
    {
        let mut h = open(360.0, BOOK);
        assert!(note_menu(&mut h).1.is_empty(), "with a cover, its actions are in the ellipsis on the cover");
        h.escape();
        let (layer, actions) = menu(&mut h);
        assert_eq!(actions, [Replace, Random, Remove, UploadBook]);
        h.act(layer, MenuChoice::Item(2));
        assert!(ids(&h).more.is_none(), "without a cover no ellipsis is left in the header");
        assert_eq!(note_menu(&mut h).1, [Add, Random, UploadBook], "on a phone the book cover is uploaded from the note menu");
    }
    for width in [360.0, 1000.0] {
        let mut h = open(width, "текст без свойств");
        let hero = h.app.note_ids.as_ref().unwrap().hero.0.id();
        assert!(ids(&h).more.is_none() && bounds(&h, hero).height() == 0.0, "empty header took up space ({width} px)");
        let (layer, actions) = note_menu(&mut h);
        assert_eq!(actions, [Add, Random], "a cover can be added from the note menu ({width} px)");
        h.act(layer, MenuChoice::Item(1));
        assert!(ids(&h).cover.is_some() && h.app.text.to_string().ends_with("---\nтекст без свойств"), "a random cover creates the header");
        assert_eq!(ids(&h).more.is_some(), width < 600.0, "on desktop with a cover, the previous hover panel");
    }
}

#[test]
fn removed_cover_comes_back_with_undo() {
    use crate::app::note_page::menu::NoteCommand;
    let mut h = open(360.0, PHOTO);
    h.act(ids(&h).more.unwrap(), Pressed);
    let Some(Popup::Menu { layer, commands }) = &h.app.popup else { panic!("cover menu not open") };
    let (layer, remove) = (*layer, commands.iter().position(|&c| c == MenuCommand::Cover(CoverAction::Remove)).unwrap());
    h.act(layer, MenuChoice::Item(remove));
    let removed = h.app.text.to_string();
    assert!(!removed.contains("Page_cover"));
    h.focus_editor();
    h.type_text("!");
    let menu = |h: &mut Harness, command| {
        super::super::wide::choose(h, command);
        h.deliver();
        h.app.text.to_string()
    };
    assert_eq!(menu(&mut h, NoteCommand::Undo), removed, "typing is undone first");
    assert_eq!(menu(&mut h, NoteCommand::Undo), PHOTO, "then the cover comes back");
    assert!(ids(&h).cover.is_some(), "cover is back on the page");
    assert_eq!(menu(&mut h, NoteCommand::Redo), removed);
    menu(&mut h, NoteCommand::Undo);
    let page = h.app.note.as_ref().unwrap().path.clone();
    h.app.write_fields(&mut h.root, &page, aquilum_core::history::Source::Reading, &[(crate::frontmatter::PAGES, "3/10".into())]);
    let body = h.app.note_ids.as_ref().unwrap().body;
    assert!(!body.get(&h.root).unwrap().inner().can_undo(false), "reading progress is not undoable and clears old steps");
}

#[test]
fn wide_cover_shows_its_actions_on_hover() {
    let mut h = open(1000.0, PHOTO);
    assert!(ids(&h).more.is_none(), "no ellipsis button on a wide screen");
    let replace = button(&h, CoverAction::Replace);
    assert!(hidden(&h, replace));
    hover_cover(&mut h);
    assert!(!hidden(&h, replace), "hover shows the action panel");
    shot(&mut h, "cover-hover-1000.png", false);
}
