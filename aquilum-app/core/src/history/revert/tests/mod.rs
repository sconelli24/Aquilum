use super::{apply, as_map, between, windows, Map};
use crate::history::delta::{compose, Region};
use crate::text::content::{collect, Splice};

mod random;

pub(crate) struct Edited {
    pub(crate) text: String,
    pub(crate) regions: Vec<Region>,
}

impl Edited {
    pub(crate) fn new(text: &str) -> Self {
        Self { text: text.to_owned(), regions: Vec::new() }
    }

    fn splice(&mut self, at: usize, removed: usize, insert: &str) {
        self.text.replace_range(at..at + removed, insert);
        self.regions = compose(&self.regions, Splice { at, removed, inserted: insert.len() });
    }

    pub(crate) fn replace(&mut self, old: &str, new: &str) {
        let at = self.text.find(old).unwrap_or_else(|| panic!("the text has no {old:?}"));
        let same = |old: &mut dyn Iterator<Item = char>, new: &mut dyn Iterator<Item = char>| old.zip(new).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum::<usize>();
        let head = same(&mut old.chars(), &mut new.chars());
        let tail = same(&mut old[head..].chars().rev(), &mut new[head..].chars().rev());
        self.splice(at + head, old.len() - head - tail, &new[head..new.len() - tail]);
    }

    fn done(self) -> (String, Map) {
        (self.text, as_map(&self.regions))
    }
}

fn edited(base: &str, edits: &[(&str, &str)]) -> (String, Map) {
    let mut edited = Edited::new(base);
    for (old, new) in edits {
        edited.replace(old, new);
    }
    edited.done()
}

fn undo(previous: &str, own: &Map, version: &str, later: &Map, current: &str) -> (String, Vec<String>) {
    let found = windows(&version, own, later).unwrap();
    let reverted = apply(&found, &previous, &version, Box::new(current.to_owned())).unwrap();
    (collect(&reverted.text), reverted.displaced)
}

fn revert(previous: &str, version_edits: &[(&str, &str)], later_edits: &[(&str, &str)]) -> (String, Vec<String>) {
    let (version, own) = edited(previous, version_edits);
    let (current, later) = edited(&version, later_edits);
    undo(previous, &own, &version, &later, &current)
}

#[test]
fn a_deleted_whole_line_comes_back() {
    let (text, _) = revert("раз\nдва\nтри\n", &[("два\n", "")], &[]);
    assert_eq!(text, "раз\nдва\nтри\n", "reverting the deletion of a whole line returns the line");
}

#[test]
fn a_deleted_line_start_comes_back() {
    let (text, _) = revert("раз\nПривет, мир\nтри\n", &[("Привет, ", "")], &[]);
    assert_eq!(text, "раз\nПривет, мир\nтри\n", "reverting the deletion of the start of a line returns the start");
}

#[test]
fn a_deleted_first_line_comes_back() {
    let (text, _) = revert("первая\nвторая\n", &[("первая\n", "")], &[]);
    assert_eq!(text, "первая\nвторая\n", "reverting the deletion of the first line returns it");
}

#[test]
fn a_deleted_last_line_comes_back() {
    let (text, _) = revert("раз\nдва\nконец", &[("\nконец", "")], &[]);
    assert_eq!(text, "раз\nдва\nконец", "reverting the deletion of a tail without a line break");
    let (text, _) = revert("раз\nдва\nконец\n", &[("конец\n", "")], &[]);
    assert_eq!(text, "раз\nдва\nконец\n", "reverting the deletion of the last line");
}

#[test]
fn an_emptied_line_gets_its_text_back() {
    let (text, _) = revert("раз\nдва\nтри\n", &[("два", "")], &[]);
    assert_eq!(text, "раз\nдва\nтри\n", "reverting the erasure of a line text leaves the line with its text");
}

#[test]
fn an_inserted_line_and_a_changed_word_go_away() {
    let (text, _) = revert("раз\nтри\n", &[("раз\n", "раз\nдва\n")], &[]);
    assert_eq!(text, "раз\nтри\n", "reverting the insertion of a line");
    let (text, _) = revert("кошка спит на окне\n", &[("спит", "сидит")], &[]);
    assert_eq!(text, "кошка спит на окне\n", "reverting an edit of a word in the middle of a line");
}

#[test]
fn every_region_of_one_version_is_undone_once() {
    let previous = "а\nб\nв\nг\nд\nе\n";
    let edits = [("а\n", "А\n"), ("б\n", ""), ("в\n", "В\n"), ("д\n", "д\nвставка\n"), ("е\n", "")];
    let (text, displaced) = revert(previous, &edits, &[]);
    assert_eq!(text, previous, "several regions of one version, including adjacent ones, are reverted exactly once");
    assert!(displaced.is_empty());
}

#[test]
fn later_edits_elsewhere_survive() {
    let previous = "раз\nдва\nтри\nчетыре\nпять\n";
    let (text, displaced) = revert(previous, &[("два\n", "")], &[("раз", "РАЗ"), ("пять", "ПЯТЬ"), ("три\n", "три\nтри с половиной\n")]);
    assert_eq!(text, "РАЗ\nдва\nтри\nтри с половиной\nчетыре\nПЯТЬ\n", "later edits of other lines stay");
    assert!(displaced.is_empty());
}

#[test]
fn a_later_insertion_where_the_line_was_deleted_stays() {
    let (text, displaced) = revert("раз\nдва\nтри\n", &[("два\n", "")], &[("раз\n", "раз\nновая\n")]);
    assert_eq!(text, "раз\nдва\nновая\nтри\n", "a later insertion in place of the deleted line stays, the line comes back");
    assert!(displaced.is_empty());
}

#[test]
fn a_line_typed_again_where_it_was_deleted_is_not_doubled() {
    let (text, displaced) = revert("раз\nдва\nтри\n", &[("два\n", "")], &[("раз\n", "раз\nдва\n")]);
    assert_eq!(text, "раз\nдва\nтри\n", "a line already retyped in its place is not duplicated");
    assert!(displaced.is_empty());
    let (text, _) = revert("а\nб\nв\n", &[("а", "А"), ("б\n", "")], &[("А\n", "А\nб\n")]);
    assert_eq!(text, "а\nб\nв\n", "the neighbouring edit of the version is reverted, and the retyped line is not duplicated");
}

#[test]
fn later_lines_right_next_to_the_reverted_one_stay_in_place() {
    let (text, displaced) = revert("раз\nдва\nтри\n", &[("два", "ДВА")], &[("раз\n", "раз\nперед\n"), ("ДВА\n", "ДВА\nпосле\n")]);
    assert_eq!(text, "раз\nперед\nдва\nпосле\nтри\n", "lines inserted later right next to the reverted one stay in place");
    assert!(displaced.is_empty());
}

#[test]
fn a_later_edit_of_the_same_line_loses_to_the_revert_but_is_kept_aside() {
    let (text, displaced) = revert("раз\nдва\nтри\n", &[("два", "ДВА")], &[("ДВА", "ДВА и ещё")]);
    assert_eq!(text, "раз\nдва\nтри\n", "the revert wins over a later edit of the same line");
    assert_eq!(displaced, ["ДВА и ещё"], "the later text goes into the copy");
}

#[test]
fn a_line_deleted_later_comes_back_as_it_was_before_the_version() {
    let (text, displaced) = revert("раз\nдва\nтри\n", &[("два", "ДВА")], &[("ДВА\n", "")]);
    assert_eq!(text, "раз\nдва\nтри\n", "a line deleted later comes back as it was");
    assert!(displaced.is_empty());
}

#[test]
fn multibyte_characters_are_never_split() {
    let previous = "жук 🐞 и ёж\nзмея 🐍\n";
    let version = "жук 🐝 и ёж\nзмея 🐍\n";
    let own = between(&previous, &version).unwrap();
    let current = "жук 🐝 и ёж\nзмея 🐍 и 🦎\n";
    let later = between(&version, &current).unwrap();
    let (text, _) = undo(previous, &own, version, &later, current);
    assert_eq!(text, "жук 🐞 и ёж\nзмея 🐍 и 🦎\n", "reverting an emoji edit with byte-level regions does not break characters");
    assert!(!text.contains('\u{FFFD}'));
}

#[test]
fn crlf_lines_are_restored_with_their_endings() {
    let (text, _) = revert("раз\r\nдва\r\nтри\r\n", &[("два\r\n", "")], &[("три", "ТРИ")]);
    assert_eq!(text, "раз\r\nдва\r\nТРИ\r\n", "CRLF lines come back with the same line ending");
}

#[test]
fn an_emptied_and_a_filled_note_are_reverted() {
    let (text, _) = revert("весь текст\nзаметки\n", &[("весь текст\nзаметки\n", "")], &[]);
    assert_eq!(text, "весь текст\nзаметки\n", "reverting the clearing of a note");
    let (text, _) = revert("", &[("", "новый текст\n")], &[]);
    assert_eq!(text, "", "reverting the first filling of an empty note");
}
