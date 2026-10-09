//! Box drawing, block elements and Powerline arrows, drawn to fill the cell exactly (as Ghostty
//! and kitty do) instead of taken from the font, so they join across cells and rows.

mod canvas;
mod draw;
mod lines;
mod shapes;
#[cfg(test)]
mod tests;

pub(crate) use canvas::Canvas;
pub(crate) use draw::{draw, is_procedural};
