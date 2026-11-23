use super::*;

const OBSIDIAN: &str = r##"{
	"nodes":[
		{"id":"8a1f0c2b3d4e5f60","type":"text","text":"# Hello\n\nSee [[Note]]","x":-120,"y":-60,"width":250,"height":120,"color":"4"},
		{"id":"1b2c3d4e5f607182","type":"file","file":"Folder/Note.md","subpath":"#Part","x":200,"y":-60,"width":400,"height":400},
		{"id":"aa00bb11cc22dd33","type":"link","url":"https://example.com","x":0,"y":400,"width":300,"height":200},
		{"id":"g1","type":"group","label":"Ideas","background":"bg.png","backgroundStyle":"cover","x":-200,"y":-200,"width":900,"height":900,"color":"#ff8800"},
		{"id":"weird","type":"plugin-thing","x":1.5,"y":2,"width":10,"height":10,"custom":{"a":1}}
	],
	"edges":[
		{"id":"e1","fromNode":"8a1f0c2b3d4e5f60","fromSide":"right","toNode":"1b2c3d4e5f607182","toSide":"left","toEnd":"none","label":"rel"},
		{"id":"e2","fromNode":"1b2c3d4e5f607182","toNode":"aa00bb11cc22dd33","fromEnd":"arrow","pluginData":true}
	],
	"metadata":{"version":"1.0"}
}"##;

#[test]
fn obsidian_file_round_trips_with_unknown_fields() {
    let canvas = Canvas::parse(OBSIDIAN).unwrap();
    assert_eq!(canvas.nodes.len(), 5);
    assert_eq!(canvas.nodes[0].kind(), NodeKind::Text);
    assert_eq!(canvas.nodes[1].subpath.as_deref(), Some("#Part"));
    assert_eq!(canvas.nodes[3].background_style.as_deref(), Some("cover"));
    assert_eq!(canvas.nodes[4].kind(), NodeKind::Other);
    assert_eq!(canvas.nodes[4].extra["custom"]["a"], 1);
    assert_eq!(canvas.edges[0].from_side, Some(Side::Right));
    assert!(!canvas.edges[0].to_arrow());
    assert!(canvas.edges[1].from_arrow() && canvas.edges[1].to_arrow());
    assert_eq!(canvas.edges[1].extra["pluginData"], true);
    assert_eq!(canvas.extra["metadata"]["version"], "1.0");

    let written = canvas.to_json();
    assert!(written.contains("\"x\": -120,"), "integers stay integers: {written}");
    assert!(written.contains("\"x\": 1.5,"));
    assert!(written.contains("\n\t\"nodes\""), "tab indentation");
    assert_eq!(Canvas::parse(&written).unwrap(), canvas);
}

#[test]
fn index_text_turns_cards_into_markdown_links() {
    let canvas = Canvas::parse(OBSIDIAN).unwrap();
    let text = index_text(&canvas);
    assert_eq!(text, "# Hello\n\nSee [[Note]]\n\n[[Folder/Note.md#Part]]\n\nhttps://example.com\n\nIdeas\n\n");
}

#[test]
fn empty_and_minimal_files_parse() {
    assert_eq!(Canvas::parse("").unwrap(), Canvas::default());
    assert_eq!(Canvas::parse("{}").unwrap(), Canvas::default());
    assert_eq!(Canvas::parse(EMPTY).unwrap(), Canvas::default());
    assert!(Canvas::parse("{nope").is_err());
}

#[test]
fn removing_nodes_drops_their_edges() {
    let mut canvas = Canvas::parse(OBSIDIAN).unwrap();
    canvas.remove_nodes(&["1b2c3d4e5f607182".to_owned()]);
    assert_eq!(canvas.nodes.len(), 4);
    assert!(canvas.edges.is_empty());
}

#[test]
fn helpers_build_valid_nodes() {
    let text = Node::text("hi", 0.0, 0.0, 250.0, 60.0);
    let group = Node::group(Some("G"), -10.0, -10.0, 300.0, 100.0);
    assert_eq!(text.id.len(), 16);
    assert!(group.contains(&text));
    let mut canvas = Canvas::default();
    canvas.nodes.push(text);
    canvas.nodes.push(Node::file("a.md", 400.0, 0.0, 100.0, 100.0));
    assert_eq!(canvas.files().collect::<Vec<_>>(), ["a.md"]);
    assert_eq!(canvas.bounds(), Some((0.0, 0.0, 500.0, 100.0)));
    assert_ne!(new_id(), new_id());
}
