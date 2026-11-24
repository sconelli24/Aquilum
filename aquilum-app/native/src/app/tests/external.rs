use super::*;

fn appears(h: &mut Harness, name: &str) -> bool {
    wait_until(300, 20, || {
        h.app.on_core_events(&mut h.root);
        h.app.tree.names().iter().any(|n| n == name)
    })
}

#[test]
fn files_and_folders_put_into_the_vault_from_outside_show_up_in_the_tree() {
    let mut h = Harness::new();
    h.watch_vault();
    let vault = h.vault.path().to_path_buf();
    std::fs::write(vault.join("внешняя.md"), "текст").unwrap();
    assert!(appears(&mut h, "внешняя"), "new note did not appear");
    std::fs::create_dir(vault.join("Пустая")).unwrap();
    assert!(appears(&mut h, "Пустая"), "new empty folder did not appear");
    std::fs::create_dir(vault.join("С заметкой")).unwrap();
    std::fs::write(vault.join("С заметкой/внутри.md"), "текст").unwrap();
    assert!(appears(&mut h, "С заметкой"), "new folder with a note did not appear");
    std::fs::write(vault.join("картинка.png"), "png").unwrap();
    assert!(appears(&mut h, "картинка.png"), "new image did not appear");
    std::fs::remove_file(vault.join("внешняя.md")).unwrap();
    assert!(wait_until(300, 20, || {
        h.app.on_core_events(&mut h.root);
        !h.app.tree.names().iter().any(|n| n == "внешняя")
    }), "deleted note remained");
}

#[test]
fn open_note_follows_an_outside_edit_without_waiting_for_the_index() {
    let mut h = Harness::new();
    h.watch_vault();
    let path = h.write_note("внешняя.md", "старый текст");
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
    assert!(h.open(path.clone()));
    std::fs::write(&path, "новый текст снаружи").unwrap();
    assert!(
        wait_until(250, 20, || {
            h.app.on_core_events(&mut h.root);
            h.body_text() == "новый текст снаружи"
        }),
        "external edit of the note was not picked up by the editor"
    );
}

#[test]
fn a_lost_batch_of_events_still_leaves_the_tree_and_the_open_note_up_to_date() {
    let mut h = Harness::new();
    let path = h.write_note("внешняя.md", "старый текст");
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
    assert!(h.open(path.clone()));
    std::fs::write(&path, "новый текст").unwrap();
    std::fs::write(h.vault.path().join("новая.md"), "").unwrap();
    h.app.workspace.core.emit(aquilum_core::CoreEvent::WorkspaceRescanned);
    h.app.on_core_events(&mut h.root);
    assert_eq!(h.body_text(), "новый текст");
    assert!(h.app.tree.names().iter().any(|name| name == "новая"), "tree not rescanned after lost events");
}

#[test]
fn a_note_or_text_file_removed_outside_closes_its_tab() {
    let mut h = Harness::new();
    h.watch_vault();
    for name in ["удаляемая.md", "удаляемая.txt"] {
        let path = h.write_note(name, "текст");
        h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
        assert!(h.open(path.clone()));
        assert!(h.app.shows(&path));
        std::fs::remove_file(&path).unwrap();
        assert!(
            wait_until(250, 20, || {
                h.app.on_core_events(&mut h.root);
                h.app.note.as_ref().is_none_or(|note| !note.is(&path))
            }),
            "{name}: deleted file stayed open"
        );
    }
}

#[test]
fn deleting_a_text_file_from_the_tree_closes_its_tab() {
    let mut h = Harness::new();
    let path = h.write_note("черновик.txt", "текст");
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&path));
    assert!(h.open(path.clone()));
    crate::file_ops::trash_all(&h.app.workspace.core, h.vault.path().to_path_buf(), vec![path.clone()]);
    assert!(
        wait_until(250, 20, || {
            h.app.on_core_events(&mut h.root);
            h.app.note.as_ref().is_none_or(|note| !note.is(&path))
        }),
        ".txt deleted from the tree stayed open"
    );
    assert!(!path.exists());
}

#[test]
fn images_and_books_can_be_deleted_from_the_tree() {
    let mut h = Harness::new();
    let dir = h.vault.path().join("Files");
    std::fs::create_dir_all(&dir).unwrap();
    let files: Vec<std::path::PathBuf> = [("foto.png", &b"\x89PNG\r\n\x1a\n"[..]), ("libro.epub", &b"PK\x03\x04"[..]), ("doc.pdf", &b"%PDF-1.4"[..])]
        .into_iter()
        .map(|(name, bytes)| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        })
        .collect();
    h.app.tree.paths_changed(&mut h.root, &files);
    crate::file_ops::trash_all(&h.app.workspace.core, h.vault.path().to_path_buf(), files.clone());
    crate::disk::wait();
    for file in &files {
        assert!(!file.exists(), "{} was not deleted", file.display());
    }
}

#[test]
fn the_tree_menu_deletes_an_image_after_confirmation() {
    use crate::ui::menu::MenuChoice;
    let mut h = Harness::new();
    let image = h.vault.path().join("Files").join("foto.png");
    std::fs::create_dir_all(image.parent().unwrap()).unwrap();
    std::fs::write(&image, b"\x89PNG\r\n\x1a\n").unwrap();
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&image));
    h.app.open_file_menu(&mut h.root, vec![image.clone()], image.clone(), false, masonry::kurbo::Point::new(300.0, 200.0), None);
    let Some(Popup::FileMenu { layer, .. }) = &h.app.popup else { panic!("the menu did not open for an image") };
    let layer = *layer;
    h.act(layer, MenuChoice::Item(2));
    let Some(Popup::Confirm(dialog, _)) = &h.app.popup else { panic!("deleting an image did not ask for confirmation") };
    let dialog = dialog.layer;
    h.act(dialog, crate::ui::dialog::DialogAnswer::Confirm);
    crate::disk::wait();
    h.settle();
    assert!(!image.exists(), "the image is still in the vault after confirming");
}

#[test]
fn a_right_click_on_an_image_or_a_book_opens_the_tree_menu() {
    use crate::test_dir::{MOUSE, pointer_at};
    use masonry::core::PointerButton;
    let mut h = Harness::new();
    let files: Vec<std::path::PathBuf> = ["foto.png", "libro.epub", "doc.pdf"].iter().map(|name| h.vault.path().join(name)).collect();
    for file in &files {
        std::fs::write(file, b"x").unwrap();
    }
    h.app.tree.paths_changed(&mut h.root, &files);
    h.settle();
    for name in ["foto.png", "libro.epub", "doc.pdf"] {
        let row = h.app.tree.row_id(name).unwrap_or_else(|| panic!("{name} is not a row of the tree"));
        let center = h.root.get_widget(row).unwrap().ctx().bounding_box().center();
        h.press(pointer_at(center.x, center.y, 1.0), PointerButton::Secondary, MOUSE);
        assert!(matches!(h.app.popup, Some(Popup::FileMenu { .. })), "right click on {name} did not open the menu");
        h.escape();
        assert!(h.app.popup.is_none(), "the menu stayed open after Escape");
    }
}

#[test]
fn supr_deletes_the_selected_files_only_when_nothing_has_the_focus() {
    use crate::ui::keymap::Scope;
    use masonry::core::keyboard::{Key, Modifiers, NamedKey};
    assert_eq!(crate::ui::keymap::command(&[Scope::Global], &Key::Named(NamedKey::Delete), Modifiers::empty()), None, "Supr is not a global shortcut: text fields keep it");
    let mut h = Harness::new();
    let image = h.vault.path().join("foto.png");
    std::fs::write(&image, b"x").unwrap();
    h.app.tree.paths_changed(&mut h.root, std::slice::from_ref(&image));
    h.settle();
    assert!(h.app.delete_selected(&mut h.root).is_none(), "nothing is selected yet");
    assert!(h.app.popup.is_none());

    let center = h.root.get_widget(h.app.tree.row_id("foto.png").unwrap()).unwrap().ctx().bounding_box().center();
    h.click(center.x, center.y);
    assert_eq!(h.app.tree.selected_paths(), vec![image.clone()], "the row clicked last is the one Supr acts on");

    let field = h.app.tree.filter_field().expect("the filter field");
    h.root.focus_on(Some(field));
    assert!(h.app.delete_selected(&mut h.root).is_none(), "Supr belongs to the text field that has the focus");
    assert!(h.app.popup.is_none() && image.exists());

    h.root.focus_on(None);
    assert!(h.app.delete_selected(&mut h.root).is_some());
    let Some(Popup::Confirm(dialog, _)) = &h.app.popup else { panic!("Supr did not ask for confirmation") };
    assert!(image.exists(), "nothing is deleted before the answer");
    let dialog = dialog.layer;
    h.act(dialog, crate::ui::dialog::DialogAnswer::Confirm);
    crate::disk::wait();
    h.settle();
    assert!(!image.exists(), "the image is still there after confirming");
}
