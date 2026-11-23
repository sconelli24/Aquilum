use super::Story;
use crate::history::milestone::Source;

#[test]
fn reverting_twice_changes_nothing_the_second_time() {
    let story = Story::new("Дважды.md", "раз\nдва\nтри\n");
    story.agent("раз\nдва\nтри\n");
    story.edit(&[("два\n", "")], Source::Agent);
    story.edit(&[("три", "три и ещё")], Source::Agent);
    story.revert_nth(1);
    let count = story.versions().len();
    story.revert_nth(1);
    assert_eq!(story.text(), "раз\nдва\nтри и ещё\n", "reverting the same version again changes nothing");
    assert_eq!(story.versions().len(), count, "a repeated revert does not create an empty version");
}

#[test]
fn reverting_the_revert_brings_the_changes_back() {
    let story = Story::new("Туда-обратно.md", "раз\nдва\nтри\n");
    story.agent("раз\nдва\nтри\n");
    story.edit(&[("раз\n", ""), ("три", "три\nчетыре")], Source::Agent);
    story.edit(&[("два", "ДВА")], Source::Agent);
    story.revert_nth(1);
    assert_eq!(story.text(), "раз\nДВА\nтри\n");
    let last = story.versions().len() - 1;
    story.revert_nth(last);
    assert_eq!(story.text(), "ДВА\nтри\nчетыре\n", "reverting a revert brings the version changes back");
}

#[test]
fn a_revert_is_a_new_version_and_keeps_the_history() {
    let story = Story::new("Новая.md", "раз\nдва\n");
    story.agent("раз\nдва\n");
    story.edit(&[("два", "ДВА")], Source::Agent);
    let before = story.versions();
    story.revert_nth(1);
    let after = story.versions();
    assert_eq!(&after[..before.len()], before.as_slice(), "a revert does not touch earlier versions");
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after[before.len()].source, Source::Revert(before[1].at_ms), "the new version remembers which one was reverted");
    assert_eq!(story.version_text(&after[before.len()].id), "раз\nдва\n");
}

#[test]
fn a_restore_can_be_reverted() {
    let story = Story::new("Восстановление.md", "первая\n");
    story.agent("первая\n");
    story.agent("первая\nвторая\n");
    story.agent("первая\nвторая\nтретья\n");
    let first = story.versions().remove(0);
    story.save("первая\n", Source::Restore(first.at_ms));
    let last = story.versions().len() - 1;
    story.revert_nth(last);
    assert_eq!(story.text(), "первая\nвторая\nтретья\n", "reverting a restore returns the text from before it");
}

#[test]
fn the_oldest_version_and_an_unknown_one_cannot_be_reverted() {
    let story = Story::new("Старая.md", "раз\n");
    story.agent("раз\n");
    story.agent("раз\nдва\n");
    let oldest = story.versions().remove(0);
    assert!(story.revert(&oldest).is_err(), "the oldest version has no previous one");
    let mut unknown = oldest.clone();
    unknown.id = "1_00000000_me.delta".to_owned();
    assert!(story.revert(&unknown).is_err(), "an unknown version gives an error");
    assert_eq!(story.text(), "раз\nдва\n", "the note did not change");
}

#[test]
fn emoji_and_crlf_survive_a_revert_without_hints() {
    let story = Story::new("Эмодзи.md", "жук 🐞 и ёж\r\nзмея 🐍\r\nкит 🐋\r\n");
    story.agent("жук 🐞 и ёж\r\nзмея 🐍\r\nкит 🐋\r\n");
    story.agent("жук 🐝 и ёж\r\nкит 🐋\r\n");
    story.agent("жук 🐝 и ёж\r\nкит 🐋 и 🦎\r\n");
    story.revert_nth(1);
    assert_eq!(story.text(), "жук 🐞 и ёж\r\nзмея 🐍\r\nкит 🐋 и 🦎\r\n", "byte-level regions without a hint do not cut characters or line endings");
}
