//! Rows to quads: colours, glyph runs, decorations and the cursor, shared by both renderers.

mod build;
mod colors;
mod cursor;
mod model;
#[cfg(test)]
mod tests;

#[cfg(feature = "gpu")]
pub(crate) use build::build_row;
pub(crate) use build::{SceneInput, build_row_settled};
pub(crate) use cursor::{CursorDraw, resolve_cursor, spans};
pub(crate) use model::{Premul, Quad, QuadPaint, RowScene};
