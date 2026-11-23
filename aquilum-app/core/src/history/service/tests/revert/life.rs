use super::Story;
use crate::files::document::hash_bytes;
use crate::history::delta;
use crate::history::milestone::Source;
use crate::history::revert::tests::Edited;
use crate::history::store::{self, remove_expired};
use std::fs;
use std::time::{Duration, SystemTime};

#[test]
fn a_revert_after_renaming_the_note_uses_the_moved_history() {
    let mut story = Story::new("Старое имя.md", "раз\nдва\n");
    story.agent("раз\nдва\n");
    story.edit(&[("два\n", "")], Source::Agent);
    story.edit(&[("раз", "РАЗ")], Source::Agent);
    let renamed = story.fixture.vault.join("Папка").join("Новое имя.md");
    fs::create_dir_all(renamed.parent().unwrap()).unwrap();
    fs::rename(&story.note, &renamed).unwrap();
    story.history.moved(&|| story.fixture.roots(), &story.note, &renamed);
    story.note = renamed;
    story.revert_nth(1);
    assert_eq!(story.text(), "РАЗ\nдва\n", "the history moves with the note, and the revert works at the new path");
}

#[test]
fn a_revert_works_after_retention_removed_the_base_of_a_version() {
    let story = Story::new("Срок.md", "раз\nДВА\nтри\nчетыре\n");
    let dir = story.fixture.history_of("Срок.md");
    fs::create_dir_all(&dir).unwrap();
    let long_ago = SystemTime::now() - Duration::from_secs(400 * 24 * 60 * 60);
    let device = story.history.device().to_owned();
    let first = store::write_version(&dir, &device, Source::Agent, long_ago, &"раз\nдва\nтри\nчетыре\n").unwrap();
    let first = first.file_name().unwrap().to_string_lossy().into_owned();
    let mut second = Edited::new("раз\nдва\nтри\nчетыре\n");
    second.replace("два", "ДВА");
    let path = store::version_path(&dir, &device, Source::Agent, long_ago + Duration::from_secs(1), delta::EXTENSION);
    delta::write(&path, Some(&first), &hash_bytes(second.text.as_bytes()), &second.regions, &second.text.as_str()).unwrap();
    story.edit(&[("четыре", "ЧЕТЫРЕ")], Source::Agent);
    story.edit(&[("раз", "РАЗ")], Source::Agent);
    story.history.name(&|| story.fixture.roots(), &story.note, Some(&first), "веха");
    remove_expired(&story.fixture.vault, 30);
    assert_eq!(story.versions().len(), 3, "the unnamed old version is deleted, the named one stays");
    story.revert_nth(1);
    assert_eq!(story.text(), "РАЗ\nдва\nтри\nчетыре\n", "a version rewritten without a base is reverted against the remaining previous one");
}

#[test]
fn a_full_legacy_version_between_deltas_is_reverted() {
    let story = Story::new("Старый формат.md", "раз\nдва\nтри\n");
    story.agent("раз\nдва\nтри\n");
    let dir = story.fixture.history_of("Старый формат.md");
    store::write_version(&dir, story.history.device(), Source::Agent, SystemTime::now(), &"раз\nДВА\nтри\n").unwrap();
    fs::write(&story.note, "раз\nДВА\nтри\n").unwrap();
    story.edit(&[("три", "три!")], Source::Agent);
    story.revert_nth(1);
    assert_eq!(story.text(), "раз\nдва\nтри!\n", "a full version amid edits is reverted, and the edit following it stays");
}

#[test]
fn an_emptied_note_gets_its_text_back() {
    let story = Story::new("Пустая.md", "весь текст\nзаметки\n");
    story.agent("весь текст\nзаметки\n");
    story.agent("");
    story.agent("новое начало\n");
    story.revert_nth(1);
    assert_eq!(story.text(), "весь текст\nзаметки\nновое начало\n", "clearing a note is reverted, what was written after it stays");
}

#[test]
fn a_note_deleted_outside_and_written_again_reverts_to_its_old_text() {
    let story = Story::new("Пересоздана.md", "старый текст\n");
    story.agent("старый текст\n");
    story.agent("старый текст\nи правка\n");
    fs::remove_file(&story.note).unwrap();
    story.agent("совсем новый\n");
    let last = story.versions().len() - 1;
    story.revert_nth(last);
    assert_eq!(story.text(), "старый текст\nи правка\n", "reverting a write over a deleted note returns its previous text");
}

#[test]
fn an_edit_on_disk_missing_from_history_survives_the_revert() {
    let story = Story::new("Снаружи.md", "раз\nдва\nтри\n");
    story.agent("раз\nдва\nтри\n");
    story.edit(&[("два", "ДВА")], Source::Agent);
    fs::write(&story.note, "раз\nДВА\nтри\nдописано снаружи\n").unwrap();
    story.revert_nth(1);
    assert_eq!(story.text(), "раз\nдва\nтри\nдописано снаружи\n", "text on disk that has not reached the history yet is preserved");
}

#[test]
fn a_huge_note_reverts_by_regions_and_records_a_small_version() {
    let body: String = (0..120_000).map(|index| format!("строка текста {index}\n")).collect();
    let story = Story::new("Огромная.md", &body);
    story.agent(&body);
    story.edit(&[("строка текста 0\n", "Начало\nстрока текста 0\n"), ("строка текста 119999\n", "строка текста 119999\nКонец\n")], Source::Agent);
    story.edit(&[("строка текста 60000\n", "строка текста 60000 правка\n")], Source::Agent);
    story.revert_nth(1);
    assert_eq!(story.text(), body.replace("строка текста 60000\n", "строка текста 60000 правка\n"), "edits at the ends of a huge note are reverted, the edit in the middle stayed");
    let dir = story.fixture.history_of("Огромная.md");
    let revert = story.versions().pop().unwrap();
    let size = fs::metadata(dir.join(&revert.id)).unwrap().len();
    assert!(size < 4096, "reverting at the ends of a huge note wrote a {size}-byte version, not a copy of the note");
}
