//! The headings of a note, to suggest them while a link to one of its sections is typed.

use aquilum_core::search::headings::headings;

use super::Inner;
use super::note::locate;
use crate::ui::editor::resolve::Fetch;

/// Level (1 to 6) and title of every heading, in the order they appear.
pub(super) fn outline(source: &str) -> Vec<(u8, String)> {
    headings(source).into_iter().filter(|heading| !heading.title.is_empty()).map(|heading| (heading.level, heading.title)).collect()
}

pub(super) fn load(inner: &Inner, target: &str) -> Fetch<Vec<(u8, String)>> {
    let path = match locate(inner, target) {
        Ok(path) => path,
        Err(None) => return Fetch::Pending,
        Err(Some(reason)) => return Fetch::Missing(reason),
    };
    match crate::note::read(&path) {
        Ok(loaded) => Fetch::Ready(outline(&loaded.text.to_string())),
        Err(error) => Fetch::Missing(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::outline;

    #[test]
    fn every_heading_comes_with_its_level_in_document_order() {
        let source = "---\ntype: note\n---\n\n# Один\n\nтекст\n\n## Два\n\n### Три\n\n## Четыре #\n\n```\n# no es un título\n```\n";
        assert_eq!(outline(source), vec![(1, "Один".to_owned()), (2, "Два".to_owned()), (3, "Три".to_owned()), (2, "Четыре".to_owned())]);
        assert!(outline("solo texto\n").is_empty());
    }
}
