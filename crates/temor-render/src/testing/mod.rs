//! Fixtures for this crate's tests, unit and integration alike (feature `testing`, as in
//! `temor-vt`): the bundled fonts, a frame of box-drawing characters, a view of it, and, with
//! the `gpu` feature, a headless wgpu device and a way to read its texture back.

mod fonts;
mod frames;
#[cfg(feature = "gpu")]
mod gpu;

pub use fonts::{bundled, fonts};
pub use frames::{GLYPH_COLS, GLYPH_PAD, GLYPH_ROWS, glyph_frame, size_for, view};
#[cfg(feature = "gpu")]
pub use gpu::{device, read_back};
