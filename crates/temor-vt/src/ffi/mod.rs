//! Everything that touches libghostty-vt's C API. This module is the only `unsafe` in the crate;
//! every block carries a `SAFETY` comment. The safe types defined here are re-exported at the
//! crate root.

mod anchor;
mod callbacks;
mod cells;
mod clipboard_read;
mod colors;
mod convert;
mod format;
mod frame;
mod hold;
mod key;
mod key_code;
mod kitty;
mod mouse;
mod paste;
mod plain;
mod png;
mod render;
mod reports;
mod row;
mod semantic;
mod signals;
mod snapshot;
mod styled;
mod terminal;
mod terminal_read;
mod terminal_write;
mod wrap;

pub use cells::GridCell;
pub use format::Formatter;
pub use frame::Frame;
pub use hold::HoldHook;
pub use key::KeyEncoder;
pub use kitty::{KittyImage, KittyImages, KittyPixels, KittyPlacement};
pub use mouse::MouseEncoder;
pub use paste::paste_is_safe;
pub use png::install_png_decoder;
pub use render::{PendingFrame, RenderView};
pub use row::RowView;
pub use snapshot::Snapshot;
pub use terminal::Terminal;
