//! A `.canvas` file open in a tab: the board model, saving and undo, plus pure geometry that the
//! board widget draws with.

mod doc;
pub mod geometry;
#[cfg(test)]
mod tests;

pub use doc::CanvasDoc;
