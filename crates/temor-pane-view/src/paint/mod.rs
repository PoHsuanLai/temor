//! Painting: the terminal's frame through the CPU renderer, handed to the pane's texture
//! (PLAN §5).

mod painter;

pub use painter::{GridFont, Inset, PaintView, Painter};
