use super::*;
use crate::ui::editor::TextEditor;

fn cards(h: &Harness) -> Vec<(f64, Vec<String>)> {
    let Some(editor) = h.app.editor_id() else { return Vec::new() };
    h.root.get_widget(editor).and_then(|w| w.downcast::<TextEditor>().map(|e| e.inner().cards())).unwrap_or_default()
}

#[test]
fn a_note_embedded_by_heading_shows_that_section_and_a_title_that_opens_it() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "---
type: note
---

# Uno

introducción

## Dos

texto de la sección dos

- primero
- segundo

## Tres

fuera de la sección
");
    let host = h.write_note("Principal.md", "Antes

![[Fuente#Dos]]

Después
");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 1), "the embedded section was not drawn");
    let (height, links) = cards(&h).remove(0);
    assert!(height > 60.0, "a heading, a paragraph and two items need room: {height}");
    assert_eq!(links, vec!["Fuente#Dos".to_owned()], "the title opens the note at that section");
}

#[test]
fn a_whole_note_and_a_block_can_be_embedded_and_a_missing_one_stays_text() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "Primer párrafo con id ^uno

Segundo párrafo
");
    let host = h.write_note("Principal.md", "![[Fuente]]

![[Fuente#^uno]]

![[No existe]]
");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 2), "the whole note and the block should be drawn");
    let links: Vec<Vec<String>> = cards(&h).into_iter().map(|(_, links)| links).collect();
    assert_eq!(links, vec![vec!["Fuente".to_owned()], vec!["Fuente#^uno".to_owned()]]);
    assert!(h.body_text().contains("![[No existe]]"), "a note that does not exist keeps its text");
}

fn titles(h: &Harness) -> Vec<String> {
    cards(h).into_iter().flat_map(|(_, links)| links).collect()
}

#[test]
fn a_note_that_embeds_itself_or_loops_through_another_does_not_repeat_forever() {
    let mut h = Harness::new();
    let a = h.write_note("A.md", "Texto de A

![[B]]
");
    h.write_note("B.md", "Texto de B

![[A]]
");
    h.open(a);
    assert!(h.wait_for(|h| !cards(h).is_empty()), "A shows B");
    h.settle();
    // A shows B (one card); inside B, the embed of A is a loop notice, not a second card of A's text.
    assert_eq!(titles(&h), vec!["B".to_owned()]);
    let selfish = h.write_note("Yo.md", "![[Yo]]
");
    h.open(selfish);
    assert!(h.wait_for(|h| !cards(h).is_empty()), "a note that embeds itself still gets a card");
    assert!(h.wait_for(|h| h.app.embeds.as_ref().is_some_and(|e| !e.busy())));
}

#[test]
fn embedded_text_follows_the_note_it_comes_from() {
    let mut h = Harness::new();
    let source = h.write_note("Fuente.md", "uno
");
    let host = h.write_note("Principal.md", "![[Fuente]]
");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 1));
    let short = cards(&h)[0].0;
    std::fs::write(&source, "uno

dos

tres

cuatro

cinco
").unwrap();
    h.app.embeds.as_ref().unwrap().forget_paths(std::slice::from_ref(&source));
    assert!(h.wait_for(|h| cards(h).first().is_some_and(|(height, _)| *height > short + 40.0)), "the card kept the old text");
}

#[test]
fn clicking_the_title_of_an_embedded_note_opens_it() {
    let mut h = Harness::new();
    let source = h.write_note("Fuente.md", "# Uno

texto
");
    let host = h.write_note("Principal.md", "![[Fuente]]
");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 1));
    h.settle();
    let editor = h.root.get_widget(h.app.editor_id().unwrap()).unwrap().ctx().bounding_box().origin();
    let editor_ref = h.root.get_widget(h.app.editor_id().unwrap()).unwrap();
    let place = editor_ref.downcast::<TextEditor>().unwrap().inner().card_places().remove(0).0;
    h.click(editor.x + place.x + 30.0, editor.y + place.y + 14.0);
    assert!(h.wait_for(|h| h.app.note.as_ref().is_some_and(|n| n.is(&source))), "the click on the title did not open the source note");
}

/// Run with `AQ_OUT=<folder>` to write a picture of a note with embeds, to look at it.
#[test]
fn snapshot_of_embedded_notes() {
    let Some(_) = std::env::var_os("AQ_OUT") else { return };
    let mut h = Harness::new();
    h.write_note("Interior.md", "Esto está dentro de otra incrustación.\n");
    h.write_note("Fuente.md", "# Plan\n\nIntroducción con **negrita** y un [[enlace]].\n\n## Tareas\n\n- [ ] primera tarea\n- [x] segunda tarea\n\n> [!tip] Un consejo\n> dentro de la nota incrustada\n\n![[Interior]]\n\n## Cierre\n\nfinal\n");
    let host = h.write_note("Principal.md", "# Mi nota\n\nTexto antes de la incrustación.\n\n![[Fuente#Tareas]]\n\nTexto en medio.\n\n![[Fuente#Plan]]\n\nTexto al final.\n");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 2));
    for _ in 0..25 {
        assert!(h.wait_for(|h| h.app.embeds.as_ref().is_some_and(|e| !e.busy())));
        h.settle();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    let (w, hgt) = (900u16, 1100u16);
    h.root.handle_window_event(masonry::core::WindowEvent::Resize(PhysicalSize::new(u32::from(w), u32::from(hgt))));
    h.settle();
    dump("note-embeds.png", &h.frame(w, hgt, 1.0));
}

#[test]
fn an_embed_inside_an_embedded_note_gets_its_text_once_it_has_loaded() {
    let mut h = Harness::new();
    h.write_note("Interior.md", "Texto del interior\n\n- uno\n- dos\n- tres\n- cuatro\n");
    h.write_note("Fuente.md", "Intro\n\n![[Interior]]\n");
    let host = h.write_note("Principal.md", "![[Fuente]]\n");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 1));
    let first = cards(&h)[0].0;
    assert!(h.wait_for(|h| cards(h).first().is_some_and(|(height, _)| *height > first + 60.0)), "the nested embed never got its text: still {first}");
}

#[test]
fn pdf_export_finishes_with_embedded_notes_even_when_they_loop() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "# Título\n\nTexto de la fuente que se incrusta.\n\n![[Principal]]\n");
    let body = "Antes\n\n![[Fuente#Título]]\n\n![[Principal]]\n\nDespués\n";
    let host = h.write_note("Principal.md", body);
    h.open(host.clone());
    // The index is ready once the embeds of the open note resolve.
    assert!(h.wait_for(|h| !cards(h).is_empty()));
    let embeds = crate::app::embeds::Embeds::new(std::sync::Arc::clone(&h.app.workspace.core), h.vault.path().to_path_buf(), host, h._data.path().join("remote"), std::sync::Arc::new(|| {}));
    let started = std::time::Instant::now();
    let pdf = crate::app::note_page::export::render("Principal", crop::Rope::from(body), crate::ui::editor::Smart::default(), Some(embeds)).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "not a PDF");
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "the export dragged on: {:?}", started.elapsed());
}

#[test]
fn a_heading_with_characters_of_link_syntax_can_be_embedded_by_the_name_a_link_gives_it() {
    let mut h = Harness::new();
    h.write_note("Fuente.md", "# Tareas: fase #1\n\ncontenido de la fase uno\n\n## Otra\n\nfuera\n");
    let host = h.write_note("Principal.md", "![[Fuente#Tareas fase 1]]\n");
    h.open(host);
    assert!(h.wait_for(|h| cards(h).len() == 1), "the embed by the name a link gives the heading was not drawn");
    assert_eq!(titles(&h), vec!["Fuente#Tareas fase 1".to_owned()]);
}
