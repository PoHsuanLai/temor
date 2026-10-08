//! The plain values the terminal API speaks in: sizes, points, modes, colours and the config.

mod cell;
mod colour;
mod config;
mod media;
mod mode;
mod size;

pub use cell::{
    CellStyle, CellView, CellWidth, CursorBlink, CursorCell, CursorShape, CursorView, FrameColors,
    FrameDirty, Hyperlink, RowDirty, RowFilter, RowId, Selected, SemanticMark, StyleFlags,
    Underline,
};
pub use colour::{ColorScheme, Rgb};
pub use config::TerminalConfig;
pub use media::{
    Generation, ImageId, KittyLayer, KittyPixelFormat, PlacementId, PlacementKind, PlacementRender,
    TextFormat, TextSpan, Visibility,
};
#[cfg(feature = "ghostty")]
pub(crate) use mode::ModeFamily;
pub use mode::{ModeState, TerminalMode};
pub use size::{
    ByteCount, CellPx, Col, Cols, GridPoint, GridSize, LineCount, PointSpace, Px, RowCount,
    RowDelta, Rows, Scrollbar, ViewportScroll,
};
