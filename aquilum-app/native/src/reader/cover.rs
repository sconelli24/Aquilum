//! The cover image a book file carries inside: EPUB (OPF metadata) and FB2 (`<coverpage>`).
//! It becomes the book's thumbnail when the file is loaded into a note.

use std::path::Path;

use base64::Engine;

use super::{epub, fb2, text};

/// Extension of an image by its first bytes: only formats the app can show are accepted.
pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        _ => None,
    }
}

/// The image bytes and their extension, if the book has a cover of its own.
pub fn of(path: &Path) -> Option<(Vec<u8>, &'static str)> {
    let data = std::fs::read(path).ok()?;
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let bytes = match extension.as_str() {
        "mobi" | "azw3" | "azw" | "prc" | "pdf" => return None,
        "fb2" => fb2::cover(&text::parse_xml(&text::decode(&data))?)?,
        _ => epub::Epub::open(data)?.cover()?,
    };
    let extension = image_extension(&bytes)?;
    Some((bytes, extension))
}

pub(super) fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD.decode(compact).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    #[test]
    fn images_are_told_apart_by_their_first_bytes() {
        assert_eq!(image_extension(&[0x89, b'P', b'N', b'G', 1]), Some("png"));
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_extension(b"<svg xmlns=\"\"/>"), None);
        assert_eq!(image_extension(b""), None);
    }

    #[test]
    fn an_fb2_cover_comes_from_its_coverpage_binary() {
        let book = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\" xmlns:l=\"http://www.w3.org/1999/xlink\"><description><title-info><coverpage><image l:href=\"#cover.png\"/></coverpage></title-info></description><body><section><p>text</p></section></body><binary id=\"cover.png\" content-type=\"image/png\">{PIXEL}</binary></FictionBook>"
        );
        let (_dir, file) = crate::reader::tests::book_file(book);
        let (bytes, extension) = of(&file).expect("the cover of the fb2");
        assert_eq!(extension, "png");
        assert_eq!(&bytes[1..4], b"PNG");
    }

    #[test]
    fn a_book_without_a_cover_or_with_an_unreadable_one_has_none() {
        let (_dir, file) = crate::reader::tests::book_file(crate::reader::tests::sample_fb2());
        assert_eq!(of(&file), None);
        let dir = crate::test_dir::tempdir();
        let broken = dir.path().join("rota.epub");
        std::fs::write(&broken, b"not a zip").unwrap();
        assert_eq!(of(&broken), None);
        assert_eq!(of(&dir.path().join("no existe.epub")), None);
    }

    #[test]
    fn an_epub_cover_is_found_through_the_opf_when_the_book_declares_one() {
        for name in ["sample.epub", "scroll-audit.epub"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reader").join(name);
            if let Some((bytes, extension)) = of(&path) {
                assert_eq!(image_extension(&bytes), Some(extension), "{name}");
            }
        }
    }
}
