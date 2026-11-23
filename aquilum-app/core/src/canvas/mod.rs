//! JSON Canvas (`.canvas`) documents, the open format Obsidian uses for its boards.
//!
//! The model keeps every field it does not understand in `extra`, so a file written by another
//! app comes back out with those fields untouched.

mod model;
#[cfg(test)]
mod tests;

pub use model::{Canvas, Edge, End, Node, NodeKind, Side};

/// Extension of canvas files, without the dot.
pub const EXTENSION: &str = "canvas";

/// Content of a freshly created canvas.
pub const EMPTY: &str = "{\n\t\"nodes\":[],\n\t\"edges\":[]\n}";

/// Markdown stand-in for a canvas, used by search: text cards as they are, file cards as
/// `[[path]]` links so they count as backlinks, link cards as their URL and group labels as text.
pub fn index_text(canvas: &Canvas) -> String {
    let mut out = String::new();
    for node in &canvas.nodes {
        let part = match node.kind() {
            NodeKind::Text => node.text.clone(),
            NodeKind::File => node.file.as_deref().map(|file| {
                let file = file.trim_start_matches('/');
                match node.subpath.as_deref().filter(|s| s.starts_with('#')) {
                    Some(subpath) => format!("[[{file}{subpath}]]"),
                    None => format!("[[{file}]]"),
                }
            }),
            NodeKind::Link => node.url.clone(),
            NodeKind::Group => node.label.clone(),
            NodeKind::Other => None,
        };
        if let Some(part) = part.filter(|p| !p.trim().is_empty()) {
            out.push_str(part.trim_end());
            out.push_str("\n\n");
        }
    }
    out
}

/// A random 16-digit hex id, the shape Obsidian gives nodes and edges.
pub fn new_id() -> String {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        bytes = (nanos as u64).to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
