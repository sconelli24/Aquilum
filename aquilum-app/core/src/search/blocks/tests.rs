use super::{blocks, new_id};

const NOTE: &str = "---\ntipo: nota\n---\n\n# Título\n\nPrimer párrafo sin id.\n\nSegundo párrafo con id ^seg\n\n- punto uno\n- punto dos ^dos\n- punto tres\n  sigue en otra línea\n  - anidado\n\n> [!tip] Un consejo\n> contenido\n\n^cons\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nno es un bloque\n```\n\nÚltimo párrafo\n";

fn summary(source: &str) -> Vec<(String, Option<String>, bool)> {
    blocks(source).into_iter().map(|b| (b.preview, b.id, b.structured)).collect()
}

#[test]
fn paragraphs_list_items_quotes_and_tables_are_blocks_and_nothing_else() {
    let wanted: Vec<(String, Option<String>, bool)> = [
        ("Primer párrafo sin id.", None, false),
        ("Segundo párrafo con id", Some("seg"), false),
        ("punto uno", None, false),
        ("punto dos", Some("dos"), false),
        ("punto tres", None, false),
        ("anidado", None, false),
        ("Un consejo", Some("cons"), true),
        ("| a | b |", None, true),
        ("Último párrafo", None, false),
    ]
    .into_iter()
    .map(|(preview, id, structured)| (preview.to_owned(), id.map(str::to_owned), structured))
    .collect();
    assert_eq!(summary(NOTE), wanted, "properties, headings, code and blank lines are not blocks; a lone ^id belongs to the block above");
}

#[test]
fn a_block_says_where_its_identifier_goes() {
    let apply = |source: &str, index: usize, id: &str| {
        let block = &blocks(source)[index];
        let (at, text) = block.id_edit(id);
        assert!(source[..at].ends_with(&block.line.trim_end()), "the edit goes right after the last line of the block");
        format!("{}{text}{}", &source[..at], &source[at..])
    };
    assert_eq!(apply("Uno\n\nDos\n", 1, "k3x9ab"), "Uno\n\nDos ^k3x9ab\n");
    assert_eq!(apply("- a\n- b\n", 0, "x1"), "- a ^x1\n- b\n");
    assert_eq!(apply("> [!tip] T\n> cuerpo\n\nDespués\n", 0, "x1"), "> [!tip] T\n> cuerpo\n\n^x1\n\nDespués\n", "a quote takes its id on a line of its own");
    assert_eq!(apply("> cita\nsigue pegado\n", 0, "x1"), "> cita\n\n^x1\n\nsigue pegado\n", "a blank line is made when the next line touches the block");
    assert_eq!(apply("| a |\n|---|\n| 1 |", 0, "x1"), "| a |\n|---|\n| 1 |\n\n^x1");
}

#[test]
fn an_identifier_that_is_already_there_is_kept_and_not_shown_in_the_preview() {
    let found = blocks("Texto con id al final ^abc-1\n\n> cita ^otro\n");
    assert_eq!((found[0].preview.as_str(), found[0].id.as_deref()), ("Texto con id al final", Some("abc-1")));
    assert_eq!((found[1].preview.as_str(), found[1].id.as_deref()), ("cita", Some("otro")));
    assert!(blocks("a ^no válido\n").iter().all(|b| b.id.is_none()), "^ followed by more words is not an identifier");
}

#[test]
fn tasks_lose_their_box_and_long_text_is_clipped() {
    assert_eq!(blocks("- [ ] comprar pan\n- [x] hecho\n").iter().map(|b| b.preview.as_str()).collect::<Vec<_>>(), ["comprar pan", "hecho"]);
    let long = "palabra ".repeat(30);
    let preview = &blocks(&long)[0].preview;
    assert!(preview.chars().count() <= 81 && preview.ends_with('…'), "{preview:?}");
}

#[test]
fn new_identifiers_are_six_lowercase_letters_and_digits_that_are_not_taken() {
    let taken = blocks("a ^abc123\n");
    for _ in 0..20 {
        let id = new_id(&taken);
        assert_eq!(id.len(), 6);
        assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()), "{id}");
        assert_ne!(id, "abc123");
    }
}
