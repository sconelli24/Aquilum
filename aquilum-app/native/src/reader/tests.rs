use std::path::PathBuf;

use serde_json::Value;

use super::{Book, Point};

pub(crate) fn sample_fb2() -> String {
    fb2("Съешь же ещё этих мягких французских булок да выпей чаю, повторяя это снова и снова.")
}

pub(crate) fn sample_book(extension: &str) -> Vec<u8> {
    match extension {
        "fb2" => sample_fb2().into_bytes(),
        "epub" => include_bytes!("../../tests/fixtures/reader/sample.epub").to_vec(),
        "pdf" => super::pdf::tests::sample_pdf(),
        _ => include_bytes!("../../tests/fixtures/reader/sample.mobi").to_vec(),
    }
}

pub(crate) fn settings(size: f32, scrolled: bool) -> super::page::Settings {
    super::page::Settings { family: "iA Writer Quattro".into(), size, weight: 400.0, line_height: 1.6, justify: true, hyphenate: true, max_width_ch: 65, margin: 48.0, scrolled }
}

pub(crate) fn book_file(content: String) -> (tempfile::TempDir, PathBuf) {
    let dir = crate::test_dir::tempdir();
    let file = dir.path().join("книга.fb2");
    std::fs::write(&file, content).unwrap();
    (dir, file)
}

pub(crate) fn fb2(sentence: &str) -> String {
    let paragraphs: String = (1..=400).map(|i| format!("<p>Абзац номер {i}. {sentence}</p>")).collect();
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\"><body><section><title><p>Глава первая</p></title>{paragraphs}</section><section><title><p>Глава вторая</p></title>{paragraphs}</section></body></FictionBook>")
}

fn after(book: &Book, index: usize, point: Point) -> String {
    let Point::At(node, offset) = point else { return String::new() };
    let Some(text) = book.document(index).and_then(|d| d.text_of(node)) else { return String::new() };
    let units: Vec<u16> = text.encode_utf16().collect();
    let from = (offset as usize).min(units.len());
    String::from_utf16_lossy(&units[from..(from + 24).min(units.len())])
}

fn prefix(text: &str, units: usize) -> String {
    let all: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&all[..units.min(all.len())])
}

fn text_point(point: Point) -> Option<(u32, u32)> {
    match point {
        Point::At(node, offset) => Some((node, offset)),
        _ => None,
    }
}

#[test]
#[ignore]
fn matches_foliate_golden() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(std::env::var("AQ_READER_GOLDEN").unwrap()).unwrap()).unwrap();
    let vault = PathBuf::from(std::env::var("AQ_VAULT").unwrap());
    let mut failures = Vec::new();
    let mut checked = 0;
    for entry in golden.as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();
        let started = std::time::Instant::now();
        let Some(book) = Book::open(&vault.join(file)) else {
            failures.push(format!("{file}: failed to open"));
            continue;
        };
        let sections = entry["sections"].as_array().unwrap();
        if sections.len() != book.sections.len() {
            failures.push(format!("{file}: chapters {} instead of {}", book.sections.len(), sections.len()));
            continue;
        }
        for (index, section) in sections.iter().enumerate() {
            if book.sections[index].base != section["base"].as_str().unwrap() {
                failures.push(format!("{file} #{index}: base {} instead of {}", book.sections[index].base, section["base"]));
            }
            if book.document(index).is_none() {
                failures.push(format!("{file} #{index}: chapter was not parsed"));
                continue;
            }
            for point in section["points"].as_array().unwrap() {
                checked += 1;
                let cfi = point["cfi"].as_str().unwrap();
                let Some((i, start, _)) = book.resolve(cfi) else {
                    failures.push(format!("{file}: {cfi} did not resolve"));
                    continue;
                };
                let got = after(&book, i, start);
                if got != point["after"].as_str().unwrap() {
                    failures.push(format!("{file}: {cfi} → {got:?} instead of {:?}", point["after"]));
                    continue;
                }
                let back = text_point(start).and_then(|p| book.cfi(i, p, p));
                if back.as_deref() != Some(cfi) {
                    failures.push(format!("{file}: a location produced {back:?} instead of {cfi}"));
                }
            }
            for range in section["ranges"].as_array().unwrap() {
                checked += 1;
                let cfi = range["cfi"].as_str().unwrap();
                let Some((i, start, end)) = book.resolve(cfi) else {
                    failures.push(format!("{file}: {cfi} did not resolve"));
                    continue;
                };
                let text = book.text(i, start, end).unwrap_or_default();
                let length = text.encode_utf16().count() as u64;
                if length != range["length"].as_u64().unwrap() || prefix(&text, 48) != range["text"].as_str().unwrap() {
                    failures.push(format!("{file}: {cfi} → {:?} ({length}) instead of {:?} ({})", prefix(&text, 48), range["text"], range["length"]));
                    continue;
                }
                let back = text_point(start).zip(text_point(end)).and_then(|(a, b)| book.cfi(i, a, b));
                if back.as_deref() != Some(cfi) {
                    failures.push(format!("{file}: a selection produced {back:?} instead of {cfi}"));
                }
            }
        }
        for saved in entry["saved"].as_array().unwrap() {
            checked += 1;
            let cfi = saved["cfi"].as_str().unwrap();
            let Some((i, start, end)) = book.resolve(cfi) else {
                failures.push(format!("{file}: saved {cfi} did not resolve"));
                continue;
            };
            let text = book.text(i, start, end).unwrap_or_default();
            if i as u64 != saved["index"].as_u64().unwrap() || prefix(&text, 60) != saved["text"].as_str().unwrap() || text.encode_utf16().count() as u64 != saved["length"].as_u64().unwrap() {
                failures.push(format!("{file}: saved {cfi} → #{i} {:?} instead of #{} {:?}", prefix(&text, 60), saved["index"], saved["text"]));
            }
        }
        eprintln!("{file}: chapters {}, {:?}", book.sections.len(), started.elapsed());
    }
    for failure in failures.iter().take(40) {
        eprintln!("{failure}");
    }
    assert!(failures.is_empty(), "mismatches {} of {checked}", failures.len());
    eprintln!("checked {checked}, everything matched");
}

#[test]
#[ignore]
fn quote_links_point_at_their_text() {
    let note = PathBuf::from(std::env::var("AQ_NOTE").unwrap());
    let vault = PathBuf::from(std::env::var("AQ_VAULT").unwrap());
    let doc = std::fs::read_to_string(&note).unwrap();
    let own = crate::frontmatter::parse(&doc).and_then(|f| f.text(crate::frontmatter::BOOK_FILE).map(str::to_owned));
    let normalize = |s: &str| s.replace(['\\', '«', '»', '“', '”'], "").split_whitespace().collect::<Vec<_>>().join(" ");
    let quotes = super::quotes::find(&doc);
    assert!(!quotes.is_empty(), "no quotes");
    let mut failures = Vec::new();
    for quote in quotes {
        let file = quote.book.clone().or_else(|| own.clone()).expect("book file");
        let book = Book::open(&vault.join(&file)).expect("book opened");
        let Some((index, start, end)) = book.resolve(&quote.cfi) else {
            failures.push(format!("[{}] {} did not resolve", quote.label, quote.cfi));
            continue;
        };
        let got = normalize(&book.text(index, start, end).unwrap_or_default());
        let want = normalize(&doc[quote.text.clone()]);
        let head: String = want.chars().take(40).collect();
        eprintln!("[{}] {} -> {:?}", quote.label, quote.cfi, got.chars().take(60).collect::<String>());
        if !got.contains(&head) {
            failures.push(format!("[{}] {}: expected {head:?}, got {:?}", quote.label, quote.cfi, got.chars().take(80).collect::<String>()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
