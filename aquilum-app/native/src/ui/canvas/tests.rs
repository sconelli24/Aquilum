use aquilum_core::canvas::Node;

use super::paint::title;

#[test]
fn tiles_show_a_short_title() {
    assert_eq!(title(&Node::text("\n\n## Big idea\nmore", 0.0, 0.0, 1.0, 1.0)), "Big idea");
    assert_eq!(title(&Node::file("Folder/My note.md", 0.0, 0.0, 1.0, 1.0)), "My note");
    assert_eq!(title(&Node::link("https://example.com/x", 0.0, 0.0, 1.0, 1.0)), "https://example.com/x");
    assert_eq!(title(&Node::group(Some("G"), 0.0, 0.0, 1.0, 1.0)), "G");
}
