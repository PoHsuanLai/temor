//! Everything that touches libghostty-vt's C API. This module is the only `unsafe` in the crate;
//! every block carries a `SAFETY` comment. The safe types defined here are re-exported at the
//! crate root.

mod anchor;
mod callbacks;
mod cells;
mod colors;
mod convert;
mod format;
mod frame;
mod key;
mod key_code;
mod kitty;
mod mouse;
mod paste;
mod png;
mod render;
mod reports;
mod row;
mod signals;
mod snapshot;
mod terminal;
mod terminal_read;
mod vt_impl;
mod wrap;

pub use cells::GridCell;
pub use format::Formatter;
pub use frame::Frame;
pub use key::KeyEncoder;
pub use kitty::{KittyImage, KittyImages, KittyPixels, KittyPlacement};
pub use mouse::MouseEncoder;
pub use paste::paste_is_safe;
pub use png::install_png_decoder;
pub use render::{PendingFrame, RenderView};
pub use row::RowView;
pub use snapshot::Snapshot;
pub use terminal::Terminal;
