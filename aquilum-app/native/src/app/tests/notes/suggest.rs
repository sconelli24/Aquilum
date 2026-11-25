use super::*;

fn suggesting(name: &str, text: &str, moves: &[NamedKey], typed: &str, what: &str) -> Harness {
    let mut h = Harness::new();
    let note = h.write_note(name, text);
    h.app.interface.set_metadata_expanded(&note, true);
    h.open(note);
    let _ = h.root.redraw();
    h.focus_editor();
    for key in moves {
        h.key(Key::Named(key.clone()), Modifiers::empty());
    }
    h.type_text(typed);
    h.app.on_core_events(&mut h.root);
    let _ = h.root.redraw();
    assert!(h.app.suggest_layer.is_some(), "{what}: {:?}", h.body_text());
    h
}

fn complete(name: &str, text: &str, moves: &[NamedKey], typed: &str, what: &str) -> String {
    let mut h = suggesting(name, text, moves, typed, what);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let _ = h.root.redraw();
    h.app.on_core_events(&mut h.root);
    h.body_text()
}

#[test]
fn page_cover_field_suggests_pattern_names() {
    let text = complete("Обложка.md", "---\nPage_cover:\n---\nтекст", &[NamedKey::ArrowDown, NamedKey::End], " tun", "cover suggestions did not appear");
    assert_eq!(text, "---\nPage_cover: tunnel\n---\nтекст");
    assert_eq!(crate::cover::pattern_id(Some("tunnel")), Some("tunnel"));
    assert_eq!(crate::cover::pattern_id(Some("pattern:Tunnel")), Some("tunnel"));
}

#[test]
fn frontmatter_keys_are_suggested_whatever_the_case() {
    let text = complete("Свойства.md", "---\n\n---\nтекст", &[NamedKey::ArrowDown], "PAGE_c", "key suggestions did not appear");
    assert_eq!(text, "---\nPage_cover: \n---\nтекст");
}

#[test]
fn escape_closes_suggestions_without_editing() {
    let mut h = suggesting("Свойства.md", "---\n\n---\nтекст", &[NamedKey::ArrowDown], "PAGE_c", "key suggestions did not appear");
    h.escape();
    let _ = h.root.redraw();
    h.app.on_core_events(&mut h.root);
    assert!(h.app.suggest_layer.is_none(), "Esc did not close the suggestions");
    assert_eq!(h.body_text(), "---\nPAGE_c\n---\nтекст");
}

#[test]
fn typing_suggests_notes_and_enter_links_them() {
    let mut h = Harness::new();
    let vault = h.vault.path().to_path_buf();
    std::fs::write(vault.join("Проектирование.md"), "x").unwrap();
    let note = vault.join("Черновик.md");
    std::fs::write(&note, "").unwrap();
    h.app.tree.paths_changed(&mut h.root, &[vault.join("Проектирование.md"), note.clone()]);
    h.open(note);
    let _ = h.root.redraw();
    let editor = h.app.editor_id().unwrap();
    h.root.handle_text_event(TextEvent::WindowFocusChange(true));
    h.root.focus_on(Some(editor));
    let text = |h: &Harness| h.root.get_widget(editor).and_then(|w| w.downcast::<crate::ui::editor::TextEditor>()).unwrap().inner().text();
    for round in ["", "[["] {
        for ch in round.chars().chain("Прое".chars()) {
            h.key(Key::Character(ch.to_string().into()), Modifiers::empty());
        }
        let mut shown = false;
        for _ in 0..200 {
            h.app.on_core_events(&mut h.root);
            let _ = h.root.redraw();
            if h.app.suggest_layer.is_some() {
                shown = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(shown, "suggestion list did not appear ({round:?}): {:?}", text(&h));
        if let Ok(out) = std::env::var("AQ_OUT") {
            let (layers, _) = h.root.redraw();
            let mut r = crate::render::Renderer::new(2);
            r.render(&layers, W as u16, H as u16, 1.0, theme::background()).unwrap();
            std::fs::write(std::path::Path::new(&out).join(format!("suggest-{}.png", round.len())), r.png()).unwrap();
        }
        h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
        let _ = h.root.redraw();
        h.app.on_core_events(&mut h.root);
        assert!(text(&h).ends_with("[[Проектирование]]"), "{:?}", text(&h));
        assert!(h.app.suggest_layer.is_none(), "list did not close");
        h.key(Key::Character(" ".into()), Modifiers::empty());
    }
    assert_eq!(text(&h), "[[Проектирование]] [[Проектирование]] ");
}

fn shown(h: &Harness) -> Option<(Vec<String>, Vec<String>)> {
    let editor = h.app.editor_id()?;
    h.root.get_widget(editor)?.downcast::<crate::ui::editor::TextEditor>()?.inner().suggestions()
}

fn wait_for_suggestions(h: &mut Harness, wanted: impl Fn(&(Vec<String>, Vec<String>)) -> bool) -> (Vec<String>, Vec<String>) {
    if !h.wait_for(|h| shown(h).is_some_and(|found| wanted(&found))) {
        panic!("the suggestions never matched: {:?} in {:?}", shown(h), h.body_text());
    }
    shown(h).unwrap()
}

fn open_empty_note(h: &mut Harness) {
    let note = h.write_note("Principal.md", "");
    h.open(note);
    let _ = h.root.redraw();
    h.focus_editor();
}

#[test]
fn typing_a_hash_in_a_link_lists_the_headings_of_that_note_with_their_level() {
    for opening in ["[[", "![["] {
        let mut h = Harness::new();
        h.write_note("Fuente.md", "# Uno

texto

## Dos

### Tres

## Cuatro: final
");
        open_empty_note(&mut h);
        h.type_text(&format!("{opening}Fuente#"));
        let (items, hints) = wait_for_suggestions(&mut h, |(items, _)| items.len() == 4);
        assert_eq!(items, ["Uno", "Dos", "Tres", "Cuatro: final"], "{opening}: every heading shows up as soon as the # is typed");
        assert_eq!(hints, ["H1", "H2", "H3", "H2"], "{opening}: each one says which level it is");
        h.type_text("tre");
        let (items, hints) = wait_for_suggestions(&mut h, |(items, _)| items.len() == 1);
        assert_eq!((items[0].as_str(), hints[0].as_str()), ("Tres", "H3"), "{opening}: typing narrows the list");
        h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
        let _ = h.root.redraw();
        assert_eq!(h.body_text(), format!("{opening}Fuente#Tres]]"), "{opening}: Enter writes the heading and closes the link");
        assert!(shown(&h).is_none(), "the list closes after choosing");
    }
}

#[test]
fn a_heading_with_characters_of_link_syntax_is_written_without_them() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "## Cuatro: final #2
");
    open_empty_note(&mut h);
    h.type_text("[[Fuente#");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 1);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let _ = h.root.redraw();
    assert_eq!(h.body_text(), "[[Fuente#Cuatro final 2]]");
}

#[test]
fn a_hash_right_after_the_brackets_lists_the_headings_of_the_note_being_written() {
    let mut h = Harness::new();
    let note = h.write_note("Principal.md", "# Introducción

## Detalles

");
    h.open(note);
    let _ = h.root.redraw();
    h.focus_editor();
    h.key(Key::Named(NamedKey::End), Modifiers::CONTROL);
    h.type_text("[[#");
    let (items, hints) = wait_for_suggestions(&mut h, |(items, _)| !items.is_empty());
    assert_eq!((items, hints), (vec!["Introducción".to_owned(), "Detalles".to_owned()], vec!["H1".to_owned(), "H2".to_owned()]));
}

#[test]
fn a_note_that_does_not_exist_lists_no_headings() {
    let mut h = Harness::new();
    open_empty_note(&mut h);
    h.type_text("[[No existe#");
    for _ in 0..30 {
        h.app.on_core_events(&mut h.root);
        let _ = h.root.redraw();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(shown(&h).is_none(), "{:?}", shown(&h));
}

/// Run with `AQ_OUT=<folder>` to write a picture of the list of headings, to look at it.
#[test]
fn snapshot_of_the_list_of_headings() {
    if std::env::var_os("AQ_OUT").is_none() {
        return;
    }
    let mut h = Harness::new();
    h.write_note("Fuente.md", "# Plan general\n\n## Objetivos\n\n### Objetivo uno\n\n### Objetivo dos\n\n## Tareas pendientes\n\n#### Detalle fino\n");
    open_empty_note(&mut h);
    h.type_text("Mira ![[Fuente#");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 6);
    h.app.on_core_events(&mut h.root);
    h.settle();
    assert!(h.app.suggest_layer.is_some(), "the list is not on screen");
    dump("heading-suggestions.png", &h.frame(900, 700, 1.0));
}

fn new_id_in(text: &str) -> String {
    let at = text.find('^').expect("a link with an identifier");
    text[at + 1..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect()
}

fn wait_for_file(h: &mut Harness, path: &std::path::Path, wanted: impl Fn(&str) -> bool) -> String {
    let path = path.to_path_buf();
    if !h.wait_for(|_| std::fs::read_to_string(&path).is_ok_and(|text| wanted(&text))) {
        panic!("the note on disk never became what it should: {:?}", std::fs::read_to_string(&path));
    }
    std::fs::read_to_string(&path).unwrap()
}

#[test]
fn typing_caret_after_the_hash_lists_the_blocks_of_the_note_and_shows_the_identifiers_they_have() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "---\ntipo: nota\n---\n\n# Título\n\nPrimer párrafo.\n\nSegundo con id ^seg\n\n- punto uno\n\n```\ncódigo\n```\n");
    open_empty_note(&mut h);
    h.type_text("[[Fuente#^");
    let (items, hints) = wait_for_suggestions(&mut h, |(items, _)| items.len() == 3);
    assert_eq!(items, ["Primer párrafo.", "Segundo con id", "punto uno"], "headings, properties and code are not blocks");
    assert_eq!(hints, ["", "^seg", ""], "a block that has an identifier shows it");
    h.type_text("seg");
    let (items, _) = wait_for_suggestions(&mut h, |(items, _)| items.len() == 1);
    assert_eq!(items, ["Segundo con id"], "the list narrows by the text, or by the identifier");
}

#[test]
fn choosing_a_block_without_identifier_gives_it_one_in_the_note_it_is_in() {
    let mut h = Harness::new();
    let source = h.write_note("Fuente.md", "Primer párrafo.\n\nSegundo con id ^seg\n");
    open_empty_note(&mut h);
    h.type_text("![[Fuente#^");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 2);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let _ = h.root.redraw();
    let body = h.body_text();
    let id = new_id_in(&body);
    assert_eq!(id.len(), 6, "{body:?}");
    assert_eq!(body, format!("![[Fuente#^{id}]]"), "the link is written and the brackets closed");
    let written = wait_for_file(&mut h, &source, |text| text.contains('^') && text.contains("Primer"));
    assert_eq!(written, format!("Primer párrafo. ^{id}\n\nSegundo con id ^seg\n"), "the identifier went to the end of that paragraph");
}

#[test]
fn choosing_a_block_that_already_has_an_identifier_uses_it_and_leaves_the_note_alone() {
    let mut h = Harness::new();
    let source = h.write_note("Fuente.md", "Primer párrafo.\n\nSegundo con id ^seg\n");
    open_empty_note(&mut h);
    h.type_text("[[Fuente#^");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 2);
    h.type_text("segundo");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 1);
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let _ = h.root.redraw();
    assert_eq!(h.body_text(), "[[Fuente#^seg]]");
    for _ in 0..20 {
        h.app.on_core_events(&mut h.root);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "Primer párrafo.\n\nSegundo con id ^seg\n");
}

#[test]
fn a_quote_or_callout_gets_its_identifier_on_a_line_of_its_own() {
    let mut h = Harness::new();
    let source = h.write_note("Fuente.md", "> [!tip] Un consejo\n> con cuerpo\n\nfin\n");
    open_empty_note(&mut h);
    h.type_text("[[Fuente#^");
    let (items, _) = wait_for_suggestions(&mut h, |(items, _)| items.len() == 2);
    assert_eq!(items[0], "Un consejo", "a callout shows its title");
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let id = new_id_in(&h.body_text());
    let written = wait_for_file(&mut h, &source, |text| text.contains('^'));
    assert_eq!(written, format!("> [!tip] Un consejo\n> con cuerpo\n\n^{id}\n\nfin\n"));
}

#[test]
fn a_block_of_the_note_being_written_gets_its_identifier_in_the_same_edit() {
    let mut h = Harness::new();
    let note = h.write_note("Principal.md", "Texto uno\n\nTexto dos\n\n");
    h.open(note);
    let _ = h.root.redraw();
    h.focus_editor();
    h.key(Key::Named(NamedKey::End), Modifiers::CONTROL);
    h.type_text("[[#^");
    wait_for_suggestions(&mut h, |(items, _)| items.len() >= 2);
    h.key(Key::Named(NamedKey::ArrowDown), Modifiers::empty());
    h.key(Key::Named(NamedKey::Enter), Modifiers::empty());
    let _ = h.root.redraw();
    let body = h.body_text();
    let id = new_id_in(&body);
    assert_eq!(id.len(), 6, "{body:?}");
    assert_eq!(body, format!("Texto uno\n\nTexto dos ^{id}\n\n[[#^{id}]]"), "one edit: the identifier on the block, the link where it was typed");
    h.key(Key::Character("z".into()), crate::test_dir::command());
    assert_eq!(h.body_text(), "Texto uno

Texto dos

[[#^]]", "a single undo takes both away: the identifier and the link");
}

/// Run with `AQ_OUT=<folder>` to write a picture of the list of blocks, to look at it.
#[test]
fn snapshot_of_the_list_of_blocks() {
    if std::env::var_os("AQ_OUT").is_none() {
        return;
    }
    let mut h = Harness::new();
    h.write_note("Reunión.md", "# Reunión\n\nAcordamos entregar el informe el viernes por la mañana a primera hora. ^entrega\n\nRevisar el presupuesto con contabilidad.\n\n- [ ] enviar el acta a todos\n- [x] reservar la sala ^sala\n\n> [!warning] Riesgo de retraso\n> depende del proveedor\n");
    open_empty_note(&mut h);
    h.type_text("Ver ![[Reunión#^");
    wait_for_suggestions(&mut h, |(items, _)| items.len() == 5);
    h.app.on_core_events(&mut h.root);
    h.settle();
    assert!(h.app.suggest_layer.is_some(), "the list is not on screen");
    dump("block-suggestions.png", &h.frame(900, 700, 1.0));
}
