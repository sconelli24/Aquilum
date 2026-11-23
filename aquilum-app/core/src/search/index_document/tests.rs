use super::{classify, CreatedAt, CreatedSource};
use crate::search::metadata::FileState;

fn state(created: CreatedAt) -> FileState {
    FileState {
        fingerprint: (1, 1),
        analyzer_version: 0,
        created,
    }
}

fn from_file() -> CreatedAt {
    CreatedAt {
        nanos: 500,
        source: CreatedSource::FileBirthTime,
    }
}

fn declared_at(nanos: i64) -> CreatedAt {
    CreatedAt {
        nanos,
        source: CreatedSource::Frontmatter,
    }
}

#[test]
fn a_removed_declaration_hands_the_date_back_to_the_file() {
    let probe = classify(Some(state(declared_at(900))), (2, 2), from_file(), false)
        .expect("changed file is probed");

    assert_eq!(probe.created, from_file());
}

#[test]
fn a_big_note_is_indexed_by_pages_and_an_edit_rewrites_only_the_last_page() {
    use crate::search::index::SearchIndex;
    use crate::search::progress::IndexProgress;
    use crate::search::query::Request;
    use crate::search::sync::IndexSynchronizer;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let directory = tempfile::tempdir().unwrap();
    let vault = crate::search::paths::canonical_path(directory.path()).join("vault");
    std::fs::create_dir_all(&vault).unwrap();
    let note = vault.join("Большая.md");
    let mut text: String = (0..40_000).map(|i| format!("Абзац {i} обычного текста про разное.\n\n")).collect();
    text.push_str("Здесь редкоеслово в самом конце.\n");
    std::fs::write(&note, &text).unwrap();
    let index = Arc::new(SearchIndex::open(&directory.path().join("index")).unwrap());
    let database = directory.path().join("documents.sqlite3");
    let mut synchronizer = IndexSynchronizer::new(vault.clone(), Arc::clone(&index), &database, Arc::new(IndexProgress::new(0, 1))).unwrap();
    synchronizer.full_scan(&AtomicBool::new(false)).unwrap();
    let pages = index.document_count();
    assert!(pages > 2, "{pages}");
    let keys = |database: &std::path::Path| -> Vec<String> {
        let connection = rusqlite::Connection::open(database).unwrap();
        let mut statement = connection.prepare("SELECT key FROM note_pages ORDER BY ordinal").unwrap();
        statement.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect()
    };
    let before = keys(&database);

    let found = index.search(&Request::new("редкоеслово")).unwrap().results;
    assert_eq!(found.len(), 1, "pages of one note give one result");
    assert_eq!(found[0].match_offset, text[..text.find("редкоеслово").unwrap()].encode_utf16().count());
    assert!(found[0].snippet.contains("редкоеслово"));
    assert_eq!(index.search(&Request::new("абзац")).unwrap().results.len(), 1);

    text.push_str("И ещё одна строка.\n");
    std::fs::write(&note, &text).unwrap();
    synchronizer.apply_paths(std::collections::HashSet::from([note])).unwrap();
    let after = keys(&database);
    assert_eq!(index.document_count(), pages);
    assert_eq!(before.len(), after.len());
    assert_eq!(before[..before.len() - 1], after[..after.len() - 1], "previous pages are not rewritten");
    assert_ne!(before.last(), after.last());
}

#[test]
fn a_canvas_is_indexed_with_links_from_its_cards() {
    use crate::search::index::SearchIndex;
    use crate::search::progress::IndexProgress;
    use crate::search::sync::IndexSynchronizer;
    use std::collections::HashSet;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let directory = tempfile::tempdir().unwrap();
    let vault = crate::search::paths::canonical_path(directory.path()).join("vault");
    std::fs::create_dir_all(vault.join("Folder")).unwrap();
    std::fs::write(vault.join("Folder").join("Note.md"), "body").unwrap();
    let board = vault.join("Board.canvas");
    let mut canvas = crate::canvas::Canvas::default();
    canvas.nodes.push(crate::canvas::Node::file("Folder/Note.md", 0.0, 0.0, 400.0, 400.0));
    canvas.nodes.push(crate::canvas::Node::text("See [[Other]]", 0.0, 500.0, 250.0, 60.0));
    std::fs::write(&board, canvas.to_json()).unwrap();

    let index = Arc::new(SearchIndex::open(&directory.path().join("index")).unwrap());
    let database = directory.path().join("documents.sqlite3");
    let mut synchronizer = IndexSynchronizer::new(vault.clone(), Arc::clone(&index), &database, Arc::new(IndexProgress::new(0, 1))).unwrap();
    synchronizer.full_scan(&AtomicBool::new(false)).unwrap();

    let links = |database: &std::path::Path| -> Vec<(String, String)> {
        let connection = rusqlite::Connection::open(database).unwrap();
        let mut statement = connection.prepare("SELECT target_key, target_kind FROM wiki_links WHERE source_path=?1 ORDER BY byte_offset").unwrap();
        statement
            .query_map([board.to_string_lossy()], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(links(&database), [("folder/note".to_owned(), "path".to_owned()), ("other".to_owned(), "name".to_owned())]);
    let connection = rusqlite::Connection::open(&database).unwrap();
    let title: String = connection
        .query_row("SELECT title_key FROM wiki_documents WHERE path=?1", [board.to_string_lossy()], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "board.canvas");

    canvas.nodes.remove(1);
    std::fs::write(&board, canvas.to_json()).unwrap();
    synchronizer.apply_paths(HashSet::from([board.clone()])).unwrap();
    assert_eq!(links(&database), [("folder/note".to_owned(), "path".to_owned())]);
}
