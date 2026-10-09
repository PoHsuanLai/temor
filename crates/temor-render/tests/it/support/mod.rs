//! A terminal wired to a renderer with the bundled fonts, and golden-image comparison.

mod golden;
mod pane;

pub use golden::assert_golden;
#[cfg(feature = "gpu")]
pub use pane::fonts;
pub use pane::{PADDING, Pane, Shot, fixture};
